//! Persistent storage layer under `~/.tablepro`.
//!
//! - `json_file`    — atomic, locked JSON-list persistence primitives
//! - `credentials`  — OS keychain behind a fakeable `CredentialStore` trait
//! - `profiles`     — connection profiles (`connections.json`)
//! - `audit`        — append-only query audit (`audit.jsonl`)
//! - `snippets`     — saved query snippets (`snippets.json`)
//!
//! All public functions are re-exported here so callers keep using `vault::*`.

mod audit;
pub mod credentials;
pub mod json_file;
pub(crate) mod profiles;
mod snippets;

pub use audit::{append_audit, audit_stats, clear_audit, export_audit, load_audit_filtered};
pub use snippets::{delete_snippet, list_snippets, save_snippet};

use std::fs;
use std::path::PathBuf;

pub(crate) const SERVICE_NAME: &str = "com.tablepro.vault";

pub fn get_config_dir() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".tablepro");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path
}

pub fn get_exports_dir() -> PathBuf {
    let mut path = get_config_dir();
    path.push("exports");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path
}

pub fn get_backups_dir() -> PathBuf {
    let mut path = get_config_dir();
    path.push("backups");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path
}

/// Path of a named file inside the config dir (e.g. `audit.jsonl`).
pub(crate) fn file_in_config_dir(name: &str) -> PathBuf {
    let mut path = get_config_dir();
    path.push(name);
    path
}
