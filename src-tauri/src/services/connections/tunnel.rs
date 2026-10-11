//! SSH tunnel for TCP database connections.
//!
//! A tunnel owns: one authenticated SSH session to the jump host, a local
//! `127.0.0.1` listener, and an accept loop that bridges each accepted
//! socket onto a `direct-tcpip` channel aimed at the database host:port
//! (as seen from the jump host).
//!
//! Host-key policy is strict-TOFU (`StrictHostKeyChecking=accept-new`):
//! a key present in `~/.ssh/known_hosts` must match — a changed key is a
//! hard failure (MITM signal); an unknown host is learned on first
//! connect and pinned afterwards.

use crate::errors::{AppError, AppResult};
use crate::models::SshConfig;
use russh::client::{self, AuthResult, Handle};
use russh::keys::{self, PrivateKeyWithHashAlg};
use russh::Channel;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinHandle;

struct HostKeyChecker {
    host: String,
    port: u16,
}

impl client::Handler for HostKeyChecker {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = match server_public_key {
            russh::keys::PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            // Host certificates aren't pinned by known_hosts; reject until
            // we implement cert verification rather than accepting blind.
            russh::keys::PublicKeyOrCertificate::Certificate(_) => return Ok(false),
        };
        match keys::check_known_hosts(&self.host, self.port, &key) {
            Ok(true) => Ok(true),
            // Host (or this key algorithm) not recorded yet — TOFU: learn
            // and pin; a *changed* key returns Err(KeyChanged) above.
            Ok(false) => {
                Ok(keys::known_hosts::learn_known_hosts(&self.host, self.port, &key).is_ok())
            }
            // Changed key or unreadable known_hosts — reject loudly.
            Err(_) => Ok(false),
        }
    }
}

/// A live local-forward tunnel. Drop aborts the accept loop; the SSH
/// session is owned by that task and closes with it.
pub struct SshTunnel {
    local_port: u16,
    shutdown: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl SshTunnel {
    /// Open SSH session to `cfg`, authenticate, bind a local listener and
    /// start forwarding to `db_host:db_port`.
    pub async fn open(
        cfg: &SshConfig,
        password: Option<String>,
        key_passphrase: Option<String>,
        db_host: &str,
        db_port: u16,
    ) -> AppResult<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| AppError::connection(format!("SSH tunnel listener failed: {}", e)))?;
        let local_port = listener
            .local_addr()
            .map_err(|e| AppError::connection(format!("SSH tunnel listener failed: {}", e)))?
            .port();

        let handler = HostKeyChecker {
            host: cfg.host.clone(),
            port: cfg.port.unwrap_or(22),
        };
        let ssh_cfg = Arc::new(client::Config::default());
        let mut session: Handle<HostKeyChecker> =
            client::connect(ssh_cfg, (cfg.host.as_str(), handler.port), handler)
                .await
                .map_err(|e| {
                    AppError::connection(format!("SSH connect to {} failed: {}", cfg.host, e))
                })?;

        let auth = match cfg.auth_method.as_str() {
            "key_file" => {
                let path = cfg
                    .key_path
                    .clone()
                    .ok_or_else(|| AppError::validation("SSH key-file auth requires a key path"))?;
                let key = keys::load_secret_key(&path, key_passphrase.as_deref()).map_err(|e| {
                    AppError::credential_unavailable(format!(
                        "Cannot load SSH key '{}': {}",
                        path, e
                    ))
                })?;
                let hash = session
                    .best_supported_rsa_hash()
                    .await
                    .ok()
                    .flatten()
                    .flatten();
                session
                    .authenticate_publickey(
                        cfg.username.clone(),
                        PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                    )
                    .await
            }
            _ => {
                session
                    .authenticate_password(cfg.username.clone(), password.unwrap_or_default())
                    .await
            }
        }
        .map_err(|e| AppError::connection(format!("SSH authentication failed: {}", e)))?;

        if !matches!(auth, AuthResult::Success) {
            return Err(AppError::credential_unavailable(
                "SSH authentication rejected — check the username and credentials",
            ));
        }

        let session = Arc::new(session);
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let db_host = db_host.to_string();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => break,
                    accepted = listener.accept() => {
                        match accepted {
                            Ok((socket, _)) => {
                                let session = Arc::clone(&session);
                                let host = db_host.clone();
                                tokio::spawn(bridge(session, socket, host, db_port));
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        Ok(Self {
            local_port,
            shutdown: shutdown_tx,
            task,
        })
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        self.task.abort();
    }
}

/// Bridge one accepted TCP socket to a direct-tcpip channel.
async fn bridge<H: client::Handler + Send + Sync + 'static>(
    session: Arc<Handle<H>>,
    tcp: TcpStream,
    db_host: String,
    db_port: u16,
) {
    let result: Result<(), ()> = async {
        let channel: Channel<_> = session
            .channel_open_direct_tcpip(db_host, db_port as u32, "127.0.0.1", 0)
            .await
            .map_err(|_| ())?;
        let mut stream = channel.into_stream();
        let mut tcp = tcp;
        tokio::io::copy_bidirectional(&mut tcp, &mut stream)
            .await
            .map_err(|_| ())?;
        Ok(())
    }
    .await;
    let _ = result; // individual channel failure must not kill the tunnel
}
