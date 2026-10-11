//! OS credential store behind a narrow, fakeable interface.
//!
//! Distinctions that matter:
//! - `get` returning `Ok(None)` means "no credential stored" — a legitimate
//!   state (passwordless connection) decided by the caller.
//! - `Err(_)` means the store itself failed (locked, access denied, missing
//!   backend) and must surface to the user — never degraded into "no
//!   password", which would produce a confusing auth failure downstream.
//!
//! There is no delete-then-set fallback: a failed `set` leaves the existing
//! credential intact so the user never silently loses a working password.

use crate::errors::{AppError, AppResult};

/// What to do with the stored secret while saving a profile.
#[derive(Debug, Clone)]
pub enum SecretIntent {
    /// Leave the stored credential untouched.
    Keep,
    /// Replace the stored credential with this value.
    Set(String),
}

pub trait CredentialStore: Send + Sync {
    fn get(&self, account: &str) -> AppResult<Option<String>>;
    fn set(&self, account: &str, secret: &str) -> AppResult<()>;
    fn delete(&self, account: &str) -> AppResult<()>;
}

/// Production store backed by the OS keychain/secret service.
pub struct KeychainStore {
    service: &'static str,
}

impl KeychainStore {
    pub fn new() -> Self {
        Self {
            service: super::SERVICE_NAME,
        }
    }
}

impl Default for KeychainStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for KeychainStore {
    fn get(&self, account: &str) -> AppResult<Option<String>> {
        let entry = keyring::Entry::new(self.service, account)
            .map_err(|e| AppError::credential_unavailable(e.to_string()))?;
        match entry.get_password() {
            Ok(pw) => Ok(Some(pw)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::credential_unavailable(e.to_string())),
        }
    }

    fn set(&self, account: &str, secret: &str) -> AppResult<()> {
        let entry = keyring::Entry::new(self.service, account)
            .map_err(|e| AppError::credential_unavailable(e.to_string()))?;
        // No delete-then-set fallback: on failure the existing credential
        // stays valid and the error is reported to the user.
        entry
            .set_password(secret)
            .map_err(|e| AppError::credential_unavailable(format!("Keychain save failed: {}", e)))
    }

    fn delete(&self, account: &str) -> AppResult<()> {
        let entry = keyring::Entry::new(self.service, account)
            .map_err(|e| AppError::credential_unavailable(e.to_string()))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::credential_unavailable(e.to_string())),
        }
    }
}

/// In-memory store for tests — never touches the real keychain.
#[cfg(test)]
pub struct MemoryStore {
    inner: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

#[cfg(test)]
impl MemoryStore {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[cfg(test)]
impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl CredentialStore for MemoryStore {
    fn get(&self, account: &str) -> AppResult<Option<String>> {
        Ok(self.inner.lock().unwrap().get(account).cloned())
    }
    fn set(&self, account: &str, secret: &str) -> AppResult<()> {
        self.inner
            .lock()
            .unwrap()
            .insert(account.to_string(), secret.to_string());
        Ok(())
    }
    fn delete(&self, account: &str) -> AppResult<()> {
        self.inner.lock().unwrap().remove(account);
        Ok(())
    }
}
