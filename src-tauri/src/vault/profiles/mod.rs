//! Connection profiles (`connections.json`) — validated, locked, atomic.
//! Passwords never enter the JSON file — only the OS keychain (see
//! `vault::credentials`).

use super::json_file;
use crate::errors::AppResult;
use crate::models::ConnectionProfile;
use std::path::Path;

pub(crate) fn load_profiles_from(path: &Path) -> AppResult<Vec<ConnectionProfile>> {
    json_file::load_list(path)
}

/// Insert-or-replace by `profile.id` under the file lock.
pub(crate) fn save_profile_to(path: &Path, profile: ConnectionProfile) -> AppResult<()> {
    json_file::update_list(path, move |items: &mut Vec<ConnectionProfile>| {
        if let Some(pos) = items.iter().position(|p| p.id == profile.id) {
            items[pos] = profile.clone();
        } else {
            items.push(profile.clone());
        }
        Ok(())
    })
}

/// Remove a profile by id. Credential cleanup is the caller's job —
/// deleting the file entry first keeps a failed credential delete from
/// resurrecting the profile.
pub(crate) fn delete_profile_from(path: &Path, id: &str) -> AppResult<()> {
    json_file::update_list::<ConnectionProfile, _>(path, |items| {
        items.retain(|p| p.id != id);
        Ok(())
    })
}

#[cfg(test)]
mod tests;
