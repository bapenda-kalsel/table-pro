//! Atomic JSON-list persistence with a cross-process advisory lock.
//!
//! Rules enforced here:
//! - A missing file is an empty collection (first run).
//! - A file that exists but cannot be read or parsed is a hard error —
//!   never silently treated as empty, otherwise the next save would
//!   destroy the user's real data.
//! - Writes go to a sibling temp file and are renamed over the target,
//!   so readers never observe a torn write.
//! - Read-modify-write cycles hold an OS-level lock on a `.lock` sibling,
//!   so two TablePro processes cannot lose each other's updates.

use crate::errors::{AppError, AppResult};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};

fn lock_path_for(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_os_string();
    p.push(".lock");
    PathBuf::from(p)
}

/// Load a JSON list file.
/// Missing => `Ok(vec![])`; unreadable/corrupt => `Err` (never data loss).
pub fn load_list<T: DeserializeOwned>(path: &Path) -> AppResult<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content =
        std::fs::read_to_string(path).map_err(|e| AppError::storage_at(path, e.to_string()))?;
    serde_json::from_str(&content).map_err(|e| AppError::storage_corrupt(path, e.to_string()))
}

/// Replace `path` with `items` serialized as pretty JSON, atomically.
pub fn write_list_atomic<T: Serialize>(path: &Path, items: &[T]) -> AppResult<()> {
    let json = serde_json::to_string_pretty(items).map_err(AppError::from)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| AppError::storage_at(dir, e.to_string()))?;

    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(AppError::from)?;
    tmp.write_all(json.as_bytes()).map_err(AppError::from)?;
    tmp.as_file().sync_all().map_err(AppError::from)?;
    // Persist replaces the destination on all supported platforms.
    tmp.persist(path)
        .map_err(|e| AppError::storage_at(path, e.error.to_string()))?;
    restrict_file_permissions(path);
    Ok(())
}

/// Run a read-modify-write cycle on `path` under the OS advisory lock.
pub fn update_list<T, F>(path: &Path, f: F) -> AppResult<()>
where
    T: DeserializeOwned + Serialize,
    F: FnOnce(&mut Vec<T>) -> AppResult<()>,
{
    let lock_path = lock_path_for(path);
    let lock_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| AppError::storage_at(&lock_path, e.to_string()))?;
    let mut lock = fd_lock::RwLock::new(lock_file);
    let _guard = lock
        .write()
        .map_err(|e| AppError::storage(format!("vault file lock failed: {}", e)))?;

    let mut items = load_list(path)?;
    f(&mut items)?;
    write_list_atomic(path, &items)
}

/// Load a single JSON document.
/// Missing => `Ok(None)`; unreadable/corrupt => `Err`.
pub fn load_doc<T: DeserializeOwned>(path: &Path) -> AppResult<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(path).map_err(|e| AppError::storage_at(path, e.to_string()))?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|e| AppError::storage_corrupt(path, e.to_string()))
}

/// Write a single JSON document atomically (temp file + rename).
pub fn write_doc_atomic<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let json = serde_json::to_string_pretty(value).map_err(AppError::from)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| AppError::storage_at(dir, e.to_string()))?;

    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(AppError::from)?;
    tmp.write_all(json.as_bytes()).map_err(AppError::from)?;
    tmp.as_file().sync_all().map_err(AppError::from)?;
    tmp.persist(path)
        .map_err(|e| AppError::storage_at(path, e.error.to_string()))?;
    restrict_file_permissions(path);
    Ok(())
}

/// Owner-only permissions on Unix; ACLs govern on Windows.
#[cfg(unix)]
fn restrict_file_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}
#[cfg(not(unix))]
fn restrict_file_permissions(_path: &Path) {}

#[cfg(test)]
mod tests;
