//! Connection profiles (`connections.json`) + OS-keychain password storage.
//! Passwords never enter the JSON file — only the keychain.

use super::{file_in_config_dir, SERVICE_NAME};
use crate::models::ConnectionProfile;
use keyring::Entry;
use std::fs;
use std::path::Path;

fn get_profiles_file() -> std::path::PathBuf {
    file_in_config_dir("connections.json")
}

pub fn load_profiles() -> Vec<ConnectionProfile> {
    load_profiles_from(&get_profiles_file())
}

fn load_profiles_from(file: &Path) -> Vec<ConnectionProfile> {
    if !file.exists() {
        return Vec::new();
    }
    match fs::read_to_string(file) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save_profile_to(file: &Path, profile: ConnectionProfile) -> Result<(), String> {
    let mut profiles = load_profiles_from(file);
    if let Some(pos) = profiles.iter().position(|p| p.id == profile.id) {
        profiles[pos] = profile;
    } else {
        profiles.push(profile);
    }
    let json = serde_json::to_string_pretty(&profiles).map_err(|e| e.to_string())?;
    fs::write(file, json).map_err(|e| e.to_string())
}

pub fn save_profile(profile: ConnectionProfile) -> Result<(), String> {
    save_profile_to(&get_profiles_file(), profile)
}

pub fn delete_profile(id: &str) -> Result<(), String> {
    let mut profiles = load_profiles();
    profiles.retain(|p| p.id != id);
    let file = get_profiles_file();
    let json = serde_json::to_string_pretty(&profiles).map_err(|e| e.to_string())?;
    let _ = fs::write(file, json);
    // Also remove password from Keychain
    let _ = delete_password(id);
    Ok(())
}

pub fn save_password(connection_id: &str, password: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, connection_id).map_err(|e| e.to_string())?;
    if entry.set_password(password).is_ok() {
        return Ok(());
    }
    // macOS can refuse to UPDATE an item created by a differently-signed build
    // (errSecAuthFailed). Delete the stale credential and write a fresh one.
    let _ = entry.delete_credential();
    entry
        .set_password(password)
        .map_err(|e| format!("Keychain save failed: {}", e))
}

/// Fetch a connection's secret from the OS keychain (used by TCP drivers).
pub fn get_password(connection_id: &str) -> Result<String, String> {
    let entry = Entry::new(SERVICE_NAME, connection_id).map_err(|e| e.to_string())?;
    entry.get_password().map_err(|e| e.to_string())
}

pub fn delete_password(connection_id: &str) -> Result<(), String> {
    if let Ok(entry) = Entry::new(SERVICE_NAME, connection_id) {
        let _ = entry.delete_credential();
    }
    Ok(())
}

#[cfg(test)]
mod tests;
