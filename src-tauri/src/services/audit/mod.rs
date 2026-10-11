//! Audit service: one place that times an operation, builds the
//! `SqlLogEntry`, appends it durably, and mirrors it into the bounded
//! in-memory console the frontend reads.
//!
//! Rules:
//! - Persistence failure flips `degraded` — it must NOT turn a successful
//!   query into an error (that would tempt the UI to retry a mutation).
//! - Labels never contain secrets: callers pass the SQL text being
//!   executed or a short operation description, never bind values,
//!   credentials, or approval tokens.

use super::connections::LiveConnection;
use crate::errors::AppResult;
use crate::models::{detect_statement_type, SqlLogEntry};
use crate::vault;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Console keeps at most this many entries (oldest dropped first).
const CONSOLE_CAP: usize = 500;

pub struct AuditService {
    console: Arc<RwLock<VecDeque<SqlLogEntry>>>,
    degraded: Arc<AtomicBool>,
}

impl AuditService {
    pub fn new() -> Self {
        Self {
            console: Arc::new(RwLock::new(VecDeque::new())),
            degraded: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Time `fut`, log its outcome, return the result unchanged.
    /// `affected` extracts the row count from a successful result.
    pub async fn run<T, Fut, A>(
        &self,
        conn: &LiveConnection,
        label: String,
        fut: Fut,
        affected: A,
    ) -> AppResult<T>
    where
        Fut: std::future::Future<Output = AppResult<T>>,
        A: FnOnce(&T) -> Option<u64>,
    {
        let start = Instant::now();
        let result = fut.await;
        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        let (is_error, error_message, affected_rows) = match &result {
            Ok(v) => (false, None, affected(v)),
            Err(e) => (true, Some(e.message()), None),
        };
        self.record(
            Some(conn.id.clone()),
            Some(conn.connection_name.clone()),
            Some(conn.environment.as_str().to_string()),
            label,
            duration_ms,
            is_error,
            error_message,
            affected_rows,
        )
        .await;
        result
    }

    /// Direct entry write — for lifecycle events (connect/disconnect) that
    /// don't fit the run() shape.
    #[allow(clippy::too_many_arguments)]
    pub async fn record(
        &self,
        connection_id: Option<String>,
        connection_name: Option<String>,
        environment: Option<String>,
        query: String,
        duration_ms: f64,
        is_error: bool,
        error_message: Option<String>,
        affected_rows: Option<u64>,
    ) {
        let entry = SqlLogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            connection_id,
            connection_name,
            environment,
            statement_type: Some(detect_statement_type(&query)),
            query,
            duration_ms,
            is_error,
            error_message,
            affected_rows,
        };

        if vault::append_audit(&entry).is_err() {
            self.degraded.store(true, Ordering::Relaxed);
        }

        let mut console = self.console.write().await;
        console.push_back(entry);
        while console.len() > CONSOLE_CAP {
            console.pop_front();
        }
    }

    pub async fn console_entries(&self) -> Vec<SqlLogEntry> {
        self.console.read().await.iter().cloned().collect()
    }

    pub async fn clear_console(&self) {
        self.console.write().await.clear();
    }

    /// True when the durable audit write has failed at least once —
    /// surfaced to the UI so silent audit loss is visible.
    pub fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::Relaxed)
    }
}

impl Default for AuditService {
    fn default() -> Self {
        Self::new()
    }
}
