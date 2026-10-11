//! Connection lifecycle service: profile validation, credential resolution,
//! driver construction, and session registry management.
//!
//! Commands stay transport-thin; all decisions (which password, which TLS
//! mode, what happens on partial failure) live here.

pub mod registry;
pub mod tunnel;

pub use registry::{LiveConnection, OpGuard, SessionRegistry};
use tunnel::SshTunnel;

use crate::drivers::factory;
use crate::errors::{AppError, AppResult};
use crate::models::{ConnectionProfile, DatabaseDriverType};
use crate::vault;
use crate::vault::credentials::{CredentialStore, KeychainStore, SecretIntent};
use std::path::PathBuf;
use std::sync::Arc;

pub struct ConnectionService {
    registry: SessionRegistry,
    credentials: Arc<dyn CredentialStore>,
    profiles_file: PathBuf,
    /// Serializes `connect` calls — two concurrent connects for the same
    /// profile id would otherwise race `insert` and leak one pool.
    connect_lock: tokio::sync::Mutex<()>,
}

impl ConnectionService {
    pub fn new() -> Self {
        Self {
            registry: SessionRegistry::new(),
            credentials: Arc::new(KeychainStore::new()),
            profiles_file: Self::profiles_file(),
            connect_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Test seam: isolated profiles file + fake credential store.
    #[cfg(test)]
    pub fn for_test(profiles_file: PathBuf, credentials: Arc<dyn CredentialStore>) -> Self {
        Self {
            registry: SessionRegistry::new(),
            credentials,
            profiles_file,
            connect_lock: tokio::sync::Mutex::new(()),
        }
    }

    fn profiles_file() -> PathBuf {
        vault::file_in_config_dir("connections.json")
    }

    // ---- Profile CRUD (file + credentials, honestly partial) ----

    pub fn profiles(&self) -> AppResult<Vec<ConnectionProfile>> {
        vault::profiles::load_profiles_from(&self.profiles_file)
    }

    /// Validate, then persist the profile, then apply the credential intent.
    /// If the credential step fails the profile stays saved and the error
    /// message says so — no fake success, no silent credential loss.
    pub async fn save_profile(
        &self,
        profile: ConnectionProfile,
        secret: SecretIntent,
    ) -> AppResult<()> {
        factory::validate(&profile)?;
        let profile_id = profile.id.clone();
        let profiles_file = self.profiles_file.clone();
        let credentials = self.credentials.clone();

        tokio::task::spawn_blocking(move || {
            vault::profiles::save_profile_to(&profiles_file, profile)
        })
        .await
        .map_err(|e| AppError::internal(format!("storage task failed: {}", e)))??;

        match secret {
            SecretIntent::Keep => Ok(()),
            SecretIntent::Set(pw) => {
                let id = profile_id.clone();
                tokio::task::spawn_blocking(move || credentials.set(&id, &pw))
                    .await
                    .map_err(|e| AppError::internal(format!("credential task failed: {}", e)))?
                    .map_err(|e| {
                        AppError::credential_unavailable(format!(
                            "Profile saved, but the password could not be stored: {}. \
                             Open the connection again and retry entering the password.",
                            e.message()
                        ))
                    })
            }
        }
    }

    /// SSH secrets live under dedicated keychain accounts so the
    /// connection password and SSH credentials rotate independently.
    /// Some("") clears, Some(v) replaces, None leaves the stored value.
    pub async fn save_ssh_secrets(
        &self,
        profile_id: &str,
        ssh_password: Option<String>,
        ssh_passphrase: Option<String>,
    ) -> AppResult<()> {
        for (account, value) in [
            (format!("ssh:{}", profile_id), ssh_password),
            (format!("sshpass:{}", profile_id), ssh_passphrase),
        ] {
            let credentials = self.credentials.clone();
            let result = tokio::task::spawn_blocking(move || match value {
                None => Ok(()),
                Some(v) if v.is_empty() => credentials.delete(&account),
                Some(v) => credentials.set(&account, &v),
            })
            .await
            .map_err(|e| AppError::internal(format!("credential task failed: {}", e)))?;
            result.map_err(|e| {
                AppError::credential_unavailable(format!(
                    "SSH secret could not be stored: {}",
                    e.message()
                ))
            })?;
        }
        Ok(())
    }

    /// Delete a profile. Refuses while the connection is live so a running
    /// session is never orphaned. Credential delete failures are reported
    /// (the profile file entry is already gone — said so in the message).
    pub async fn delete_profile(&self, id: &str) -> AppResult<()> {
        if self.registry.get(id).await.is_some() {
            return Err(AppError::busy(
                "This connection is active. Disconnect before deleting it.",
            ));
        }
        let profiles_file = self.profiles_file.clone();
        let id_for_file = id.to_string();
        tokio::task::spawn_blocking(move || {
            vault::profiles::delete_profile_from(&profiles_file, &id_for_file)
        })
        .await
        .map_err(|e| AppError::internal(format!("storage task failed: {}", e)))??;

        let credentials = self.credentials.clone();
        let accounts = [
            id.to_string(),
            format!("ssh:{}", id),
            format!("sshpass:{}", id),
        ];
        for account in accounts {
            let credentials = credentials.clone();
            let account_for_msg = account.clone();
            tokio::task::spawn_blocking(move || credentials.delete(&account))
                .await
                .map_err(|e| AppError::internal(format!("credential task failed: {}", e)))?
                .map_err(|e| {
                    AppError::credential_unavailable(format!(
                        "Profile was removed, but stored secret '{}' could not be deleted: {}",
                        account_for_msg,
                        e.message()
                    ))
                })?;
        }
        Ok(())
    }

    /// Resolve the effective password: an inline value wins (form field),
    /// otherwise the stored credential. `Ok(None)` means "passwordless".
    async fn resolve_password(
        &self,
        profile_id: &str,
        inline: Option<String>,
    ) -> AppResult<Option<String>> {
        if let Some(pw) = inline {
            return Ok(Some(pw));
        }
        let credentials = self.credentials.clone();
        let id = profile_id.to_string();
        tokio::task::spawn_blocking(move || credentials.get(&id))
            .await
            .map_err(|e| AppError::internal(format!("credential task failed: {}", e)))?
    }

    // ---- Session lifecycle ----

    /// Connect a stored profile. Replaces an existing session for the same
    /// id after draining it. Returns server ping latency in ms.
    pub async fn connect(&self, profile_id: &str, password: Option<String>) -> AppResult<f64> {
        let _connect_guard = self.connect_lock.lock().await;
        let profile = self
            .profiles()?
            .into_iter()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| {
                AppError::not_found(format!("Connection profile '{}' not found", profile_id))
            })?;

        // If a session already exists, close it first so the pool isn't leaked.
        if let Some(old) = self.registry.begin_close(profile_id).await {
            old.wait_drained().await;
            old.driver.close().await;
        }

        let pw = self.resolve_password(profile_id, password).await?;
        let (effective, tunnel) = self.prepare_target(&profile, None, None).await?;
        let driver = factory::build(&effective, pw).await?;
        let ms = driver.ping().await?;

        let conn = LiveConnection::new(
            profile_id.to_string(),
            driver,
            profile.driver.clone(),
            profile.name.clone(),
            profile.environment.clone(),
            self.registry.bump_generation(),
            tunnel,
        );
        self.registry
            .insert(profile_id.to_string(), Arc::new(conn))
            .await;
        Ok(ms)
    }

    /// Disconnect a session: stop new operations, drain in-flight ones,
    /// then close the pool. Idempotent — disconnecting twice is a no-op.
    pub async fn disconnect(&self, id: &str) -> AppResult<()> {
        if let Some(conn) = self.registry.begin_close(id).await {
            conn.wait_drained().await;
            conn.driver.close().await;
        }
        Ok(())
    }

    /// Connectivity check for the connection form. Uses the same
    /// validation, credential and TLS rules as `connect`, then always
    /// closes the candidate pool.
    pub async fn test_connection(
        &self,
        profile: &ConnectionProfile,
        password: Option<String>,
        ssh_password: Option<String>,
        ssh_passphrase: Option<String>,
    ) -> AppResult<f64> {
        let pw = self.resolve_password(&profile.id, password).await?;
        let (effective, tunnel) = self
            .prepare_target(profile, ssh_password, ssh_passphrase)
            .await?;
        let driver = factory::build(&effective, pw).await?;
        let result = driver.ping().await;
        driver.close().await;
        drop(tunnel);
        result
    }

    /// Resolve SSH secrets and open a tunnel when the profile asks for
    /// one, returning the effective profile (host rewritten to the local
    /// forward endpoint) plus the tunnel handle to keep alive.
    async fn prepare_target(
        &self,
        profile: &ConnectionProfile,
        ssh_password: Option<String>,
        ssh_passphrase: Option<String>,
    ) -> AppResult<(ConnectionProfile, Option<SshTunnel>)> {
        let ssh = match profile.ssh.as_ref().filter(|s| s.enabled) {
            Some(s) => s,
            None => return Ok((profile.clone(), None)),
        };
        factory::validate(profile)?;
        let default_port = match profile.driver {
            DatabaseDriverType::MySQL => 3306,
            DatabaseDriverType::PostgreSQL => 5432,
            DatabaseDriverType::SQLite => {
                return Err(AppError::validation(
                    "SSH tunnels only apply to TCP drivers (MySQL/PostgreSQL)",
                ))
            }
        };
        let ssh_pw = self
            .resolve_password(&format!("ssh:{}", profile.id), ssh_password)
            .await?;
        let ssh_pp = self
            .resolve_password(&format!("sshpass:{}", profile.id), ssh_passphrase)
            .await?;
        let db_host = profile.host.clone().unwrap_or_else(|| "localhost".into());
        let db_port = profile.port.unwrap_or(default_port);

        let tunnel = SshTunnel::open(ssh, ssh_pw, ssh_pp, &db_host, db_port).await?;
        let mut effective = profile.clone();
        effective.host = Some("127.0.0.1".into());
        effective.port = Some(tunnel.local_port());
        Ok((effective, Some(tunnel)))
    }

    /// Lease for one database operation.
    pub async fn acquire(&self, connection_id: &str) -> AppResult<OpGuard> {
        self.registry.acquire(connection_id).await
    }

    /// (name, environment) for audit enrichment; `None` when disconnected.
    pub async fn session_meta(&self, connection_id: &str) -> Option<(String, String)> {
        self.registry.get(connection_id).await.map(|c| {
            (
                c.connection_name.clone(),
                c.environment.as_str().to_string(),
            )
        })
    }
}

impl Default for ConnectionService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
