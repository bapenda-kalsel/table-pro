//! Driver construction: validate a `ConnectionProfile`, resolve its TLS
//! options into engine-specific settings, and build the concrete driver.
//!
//! This is the only place where profile fields, defaults, and TLS rules
//! are interpreted — commands and services call `build`/`validate` and
//! never touch sqlx connect options themselves.

use crate::drivers::mysql::MysqlDriver;
use crate::drivers::postgres::PostgresDriver;
use crate::drivers::sqlite::SqliteDriver;
use crate::drivers::DatabaseDriver;
use crate::errors::{AppError, AppResult};
use crate::models::{ConnectionProfile, DatabaseDriverType};

/// Unified TLS mode for TCP drivers; each driver maps it onto its own
/// sqlx enum (the vocabularies differ slightly per engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMode {
    Disabled,
    Prefer,
    Required,
    VerifyCa,
    VerifyFull,
}

impl TlsMode {
    /// Parse a stored `ssl_mode` string. Accepts a few common aliases so
    /// hand-edited profiles don't break (`verify-full`, `verifyIdentity`, ...).
    pub fn parse(raw: &str) -> AppResult<Self> {
        match raw
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_'], "")
            .as_str()
        {
            "disable" | "disabled" => Ok(Self::Disabled),
            "prefer" | "preferred" => Ok(Self::Prefer),
            "require" | "required" => Ok(Self::Required),
            "verifyca" => Ok(Self::VerifyCa),
            "verifyfull" | "verifyidentity" => Ok(Self::VerifyFull),
            other => Err(AppError::tls_config(format!(
                "Unknown TLS mode '{}' — expected one of: disable, prefer, require, verify_ca, verify_full",
                other
            ))),
        }
    }
}

/// TLS options resolved from a profile.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub mode: TlsMode,
    /// Optional custom CA bundle (PEM) for private-CA servers.
    pub ca_cert_path: Option<String>,
}

/// Fully validated connection target for TCP drivers.
#[derive(Debug, Clone)]
pub struct ResolvedProfile {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: Option<String>,
    pub tls: TlsConfig,
}

/// Validate a profile for connection use. TCP drivers require an explicit
/// TLS mode: `ssl_mode: None` (legacy/unconfigured) is an error, not a
/// silent insecure default.
pub fn validate(profile: &ConnectionProfile) -> AppResult<()> {
    match profile.driver {
        DatabaseDriverType::SQLite => {
            if profile.file_path.as_deref().unwrap_or("").is_empty() {
                return Err(AppError::validation("SQLite profile requires a file path"));
            }
        }
        _ => {
            if profile
                .database
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            {
                return Err(AppError::validation("Connection requires a database name"));
            }
            if profile
                .username
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            {
                return Err(AppError::validation("Connection requires a username"));
            }
            match profile.ssl_mode.as_deref().map(str::trim) {
                None | Some("") => {
                    return Err(AppError::tls_config(
                        "TLS mode is not configured for this connection. Open the \
                         connection settings and choose a TLS mode (recommended: \
                         'verify_full'). The setting is required once for profiles \
                         created before TLS was enforced.",
                    ));
                }
                Some(raw) => {
                    TlsMode::parse(raw)?;
                }
            }
        }
    }
    if let Some(ssh) = profile.ssh.as_ref().filter(|s| s.enabled) {
        if matches!(profile.driver, DatabaseDriverType::SQLite) {
            return Err(AppError::validation(
                "SSH tunnels only apply to TCP drivers (MySQL/PostgreSQL)",
            ));
        }
        if ssh.host.trim().is_empty() || ssh.username.trim().is_empty() {
            return Err(AppError::validation(
                "SSH tunnel requires a host and a username",
            ));
        }
        match ssh.auth_method.as_str() {
            "password" => {}
            "key_file" => {
                if ssh.key_path.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(AppError::validation(
                        "SSH key-file auth requires a private key path",
                    ));
                }
            }
            other => {
                return Err(AppError::validation(format!(
                    "Unknown SSH auth method '{}' — expected 'password' or 'key_file'",
                    other
                )))
            }
        }
    }
    Ok(())
}

/// Resolve TCP connection fields + TLS config from a validated profile.
fn resolve_tcp(profile: &ConnectionProfile, default_port: u16) -> AppResult<ResolvedProfile> {
    let mode = TlsMode::parse(profile.ssl_mode.as_deref().unwrap_or(""))?;
    Ok(ResolvedProfile {
        host: profile
            .host
            .clone()
            .unwrap_or_else(|| "localhost".to_string()),
        port: profile.port.unwrap_or(default_port),
        database: profile.database.clone().unwrap_or_default(),
        username: profile.username.clone().unwrap_or_default(),
        password: None, // filled by caller
        tls: TlsConfig {
            mode,
            ca_cert_path: profile.ssl_ca_cert_path.clone(),
        },
    })
}

/// Build a driver for a profile. `password` is `None` when the profile is
/// passwordless or no credential is stored.
pub async fn build(
    profile: &ConnectionProfile,
    password: Option<String>,
) -> AppResult<Box<dyn DatabaseDriver>> {
    validate(profile)?;
    match profile.driver {
        DatabaseDriverType::SQLite => {
            let path = profile.file_path.clone().unwrap_or_default();
            Ok(Box::new(SqliteDriver::new(&path).await?))
        }
        DatabaseDriverType::MySQL => {
            let mut r = resolve_tcp(profile, 3306)?;
            r.password = password;
            Ok(Box::new(
                MysqlDriver::new(
                    &r.host,
                    r.port,
                    &r.database,
                    &r.username,
                    r.password.as_deref(),
                    &r.tls,
                )
                .await?,
            ))
        }
        DatabaseDriverType::PostgreSQL => {
            let mut r = resolve_tcp(profile, 5432)?;
            r.password = password;
            Ok(Box::new(
                PostgresDriver::new(
                    &r.host,
                    r.port,
                    &r.database,
                    &r.username,
                    r.password.as_deref(),
                    &r.tls,
                )
                .await?,
            ))
        }
    }
}

#[cfg(test)]
mod tests;
