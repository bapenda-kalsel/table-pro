//! Application error taxonomy.
//!
//! `AppError` is both the internal error type and the IPC payload: Tauri
//! serializes command errors with serde, so the tagged enum shape is the
//! machine-readable contract the frontend consumes via `code`.
//!
//! `message` is for humans — never embed secrets, raw bind values, or
//! approval tokens in it.

use serde::Serialize;

/// One-shot approval challenge carried by `ConfirmationRequired`.
/// The frontend shows `summary`, then replays the same request with
/// `token` to prove the user approved this exact operation.
#[derive(Debug, Clone, Serialize)]
pub struct ConfirmationChallenge {
    pub token: String,
    pub summary: String,
    pub connection_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AppError {
    Validation {
        message: String,
    },
    NotConnected {
        connection_id: String,
    },
    NotFound {
        message: String,
    },
    /// The OS credential store refused or errored (locked, access denied, backend missing).
    CredentialUnavailable {
        message: String,
    },
    /// TLS configuration is missing, invalid, or verification failed.
    TlsConfig {
        message: String,
    },
    Connection {
        message: String,
    },
    Timeout {
        message: String,
    },
    /// The database engine returned an error (syntax, constraint, permission, ...).
    Database {
        message: String,
    },
    /// A local file could not be read or written.
    Storage {
        message: String,
    },
    /// A local file exists but does not parse as the expected format.
    StorageCorrupt {
        message: String,
        path: String,
    },
    /// A conflicting operation is already running on this connection.
    Busy {
        message: String,
    },
    /// The operation referenced a session that was closed or replaced.
    StaleOperation {
        message: String,
    },
    /// A production connection requires an explicit approval for this exact
    /// request; the challenge carries a one-shot token to replay it with.
    ConfirmationRequired {
        challenge: Box<ConfirmationChallenge>,
    },
    /// The supplied approval token is unknown, used, expired, or bound to a
    /// different request/session.
    ConfirmationInvalid {
        message: String,
    },
    /// The requested feature is not supported for this driver/object.
    Unsupported {
        message: String,
    },
    Internal {
        message: String,
    },
}

impl AppError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
        }
    }
    pub fn credential_unavailable(message: impl Into<String>) -> Self {
        Self::CredentialUnavailable {
            message: message.into(),
        }
    }
    pub fn tls_config(message: impl Into<String>) -> Self {
        Self::TlsConfig {
            message: message.into(),
        }
    }
    pub fn connection(message: impl Into<String>) -> Self {
        Self::Connection {
            message: message.into(),
        }
    }
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout {
            message: message.into(),
        }
    }
    pub fn database(message: impl Into<String>) -> Self {
        Self::Database {
            message: message.into(),
        }
    }
    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage {
            message: message.into(),
        }
    }
    pub fn storage_at(path: &std::path::Path, message: impl Into<String>) -> Self {
        Self::Storage {
            message: format!("{}: {}", path.display(), message.into()),
        }
    }
    pub fn storage_corrupt(path: &std::path::Path, message: impl Into<String>) -> Self {
        Self::StorageCorrupt {
            message: message.into(),
            path: path.display().to_string(),
        }
    }
    pub fn busy(message: impl Into<String>) -> Self {
        Self::Busy {
            message: message.into(),
        }
    }
    pub fn stale(message: impl Into<String>) -> Self {
        Self::StaleOperation {
            message: message.into(),
        }
    }
    pub fn confirmation_invalid(message: impl Into<String>) -> Self {
        Self::ConfirmationInvalid {
            message: message.into(),
        }
    }
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported {
            message: message.into(),
        }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Human-readable message for logging / display.
    pub fn message(&self) -> String {
        match self {
            Self::Validation { message }
            | Self::NotFound { message }
            | Self::CredentialUnavailable { message }
            | Self::TlsConfig { message }
            | Self::Connection { message }
            | Self::Timeout { message }
            | Self::Database { message }
            | Self::Storage { message }
            | Self::StorageCorrupt { message, .. }
            | Self::Busy { message }
            | Self::StaleOperation { message }
            | Self::Unsupported { message }
            | Self::Internal { message } => message.clone(),
            Self::NotConnected { connection_id } => {
                format!("Not connected to database '{}'", connection_id)
            }
            Self::ConfirmationInvalid { message } => message.clone(),
            Self::ConfirmationRequired { challenge } => challenge.summary.clone(),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::Io(io) => AppError::connection(io.to_string()),
            sqlx::Error::Tls(tls) => AppError::tls_config(tls.to_string()),
            sqlx::Error::PoolTimedOut => AppError::timeout("Connection pool timed out"),
            sqlx::Error::PoolClosed => AppError::connection("Connection pool is closed"),
            sqlx::Error::Database(db) => AppError::database(db.to_string()),
            other => AppError::database(other.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::storage(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::internal(format!("JSON (de)serialization failed: {}", e))
    }
}

impl From<csv::Error> for AppError {
    fn from(e: csv::Error) -> Self {
        AppError::validation(format!("CSV error: {}", e))
    }
}

impl From<rust_xlsxwriter::XlsxError> for AppError {
    fn from(e: rust_xlsxwriter::XlsxError) -> Self {
        AppError::internal(format!("XLSX export failed: {}", e))
    }
}

impl From<csv::IntoInnerError<csv::Writer<Vec<u8>>>> for AppError {
    fn from(e: csv::IntoInnerError<csv::Writer<Vec<u8>>>) -> Self {
        AppError::internal(format!("CSV serialization failed: {}", e))
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;

/// Ergonomic error-mapping helpers so call sites read `ctx_db("doing X")?`
/// instead of `.map_err(|e| AppError::database(format!("X: {}", e)))`.
pub trait ResultExt<T, E> {
    /// Wrap the error in `AppError::database` with a context prefix.
    fn ctx_db(self, msg: &str) -> AppResult<T>;
    /// Wrap the error in `AppError::database` with a computed message.
    fn db_with(self, f: impl FnOnce(E) -> String) -> AppResult<T>;
}

impl<T, E: std::fmt::Display> ResultExt<T, E> for Result<T, E> {
    fn ctx_db(self, msg: &str) -> AppResult<T> {
        self.map_err(|e| AppError::database(format!("{}: {}", msg, e)))
    }
    fn db_with(self, f: impl FnOnce(E) -> String) -> AppResult<T> {
        self.map_err(|e| AppError::database(f(e)))
    }
}
