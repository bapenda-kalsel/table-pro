//! Live-connection registry: sessions keyed by profile id, with explicit
//! lifecycle state and per-session operation leases.
//!
//! Invariants:
//! - The registry lock is only held to look up / insert / remove the
//!   `Arc<LiveConnection>` — never across database I/O.
//! - An operation holds an `OpGuard`; `disconnect` marks the session
//!   closing, stops new leases, waits for in-flight ops to drain, then
//!   closes the pool. Dropping a future does NOT cancel server-side work,
//!   so draining is the honest behavior.
//! - `generation` distinguishes "same profile id, different session" so
//!   async results from a previous session can be discarded safely.

use crate::drivers::DatabaseDriver;
use crate::errors::{AppError, AppResult};
use crate::models::{DatabaseDriverType, EnvironmentTag};
use crate::services::connections::tunnel::SshTunnel;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleState {
    Open,
    Closing,
}

pub struct LiveConnection {
    pub id: String,
    pub driver: Arc<dyn DatabaseDriver>,
    pub driver_type: DatabaseDriverType,
    pub connection_name: String,
    pub environment: EnvironmentTag,
    /// Unique per session of this profile id; stale-callback guard.
    pub generation: u64,
    /// Keeps the SSH tunnel alive for the session's lifetime; dropping the
    /// connection drops the tunnel, which aborts the accept loop.
    pub _tunnel: Option<SshTunnel>,
    state: std::sync::RwLock<LifecycleState>,
    active_ops: AtomicUsize,
    drained: Notify,
}

impl LiveConnection {
    pub fn new(
        id: String,
        driver: Box<dyn DatabaseDriver>,
        driver_type: DatabaseDriverType,
        connection_name: String,
        environment: EnvironmentTag,
        generation: u64,
        tunnel: Option<SshTunnel>,
    ) -> Self {
        Self {
            id,
            driver: Arc::from(driver),
            driver_type,
            connection_name,
            environment,
            generation,
            _tunnel: tunnel,
            state: std::sync::RwLock::new(LifecycleState::Open),
            active_ops: AtomicUsize::new(0),
            drained: Notify::new(),
        }
    }

    fn is_closing(&self) -> bool {
        *self.state.read().unwrap() == LifecycleState::Closing
    }

    /// Wait until no operation leases remain. Callers must have already
    /// marked the session closing, so no new leases can appear.
    pub async fn wait_drained(&self) {
        loop {
            if self.active_ops.load(Ordering::SeqCst) == 0 {
                return;
            }
            self.drained.notified().await;
        }
    }
}

/// Lease on a session for one in-flight operation. Dropping it decrements
/// the counter and wakes a draining disconnect when it reaches zero.
pub struct OpGuard {
    pub conn: Arc<LiveConnection>,
}

impl std::fmt::Debug for OpGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpGuard")
            .field("connection_id", &self.conn.id)
            .field("generation", &self.conn.generation)
            .finish()
    }
}

impl Drop for OpGuard {
    fn drop(&mut self) {
        if self.conn.active_ops.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.conn.drained.notify_waiters();
        }
    }
}

#[derive(Clone)]
pub struct SessionRegistry {
    inner: Arc<RwLock<HashMap<String, Arc<LiveConnection>>>>,
    next_generation: Arc<AtomicU64>,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            next_generation: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn bump_generation(&self) -> u64 {
        self.next_generation.fetch_add(1, Ordering::SeqCst)
    }

    /// Snapshot of a live session (no lease). For read-only metadata.
    pub async fn get(&self, id: &str) -> Option<Arc<LiveConnection>> {
        self.inner.read().await.get(id).cloned()
    }

    /// Acquire an operation lease. Fails when the session is missing or
    /// already closing; the returned guard guarantees the session cannot
    /// be closed until the guard is dropped.
    pub async fn acquire(&self, id: &str) -> AppResult<OpGuard> {
        let conn = {
            let map = self.inner.read().await;
            map.get(id).cloned()
        }
        .ok_or_else(|| AppError::NotConnected {
            connection_id: id.to_string(),
        })?;

        conn.active_ops.fetch_add(1, Ordering::SeqCst);
        // Post-increment re-check closes the TOCTOU with `begin_close`:
        // either we saw Open and our increment keeps the drain waiting,
        // or Closing won and we must back out.
        if conn.is_closing() {
            drop(OpGuard { conn });
            return Err(AppError::stale(format!("Connection '{}' is closing", id)));
        }
        Ok(OpGuard { conn })
    }

    /// Insert a fresh session, replacing any previous one without draining
    /// (the caller owns replacement ordering).
    pub async fn insert(&self, id: String, conn: Arc<LiveConnection>) {
        self.inner.write().await.insert(id, conn);
    }

    /// Mark the session closing and remove it from the map. The returned
    /// session must still be drained and closed by the caller.
    pub async fn begin_close(&self, id: &str) -> Option<Arc<LiveConnection>> {
        let conn = { self.inner.write().await.remove(id) }?;
        *conn.state.write().unwrap() = LifecycleState::Closing;
        Some(conn)
    }
}
