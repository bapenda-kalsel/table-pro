//! Connection profile CRUD + connect/disconnect — transport layer only.
//! Validation, credentials, driver construction, and session lifecycle
//! live in `services::connections` and `drivers::factory`.

use crate::errors::AppResult;
use crate::models::ConnectionProfile;
use crate::state::AppState;
use crate::vault::credentials::SecretIntent;
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub async fn get_connection_profiles(
    state: State<'_, AppState>,
) -> AppResult<Vec<ConnectionProfile>> {
    state.connections.profiles()
}

#[tauri::command]
pub async fn save_connection_profile(
    profile: ConnectionProfile,
    password: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    // Empty string means "keep whatever is stored" — the UI never sends an
    // intentional credential clear through this field.
    let secret = match password.filter(|p| !p.is_empty()) {
        Some(pw) => SecretIntent::Set(pw),
        None => SecretIntent::Keep,
    };
    state.connections.save_profile(profile, secret).await
}

#[tauri::command]
pub async fn delete_connection_profile(id: String, state: State<'_, AppState>) -> AppResult<()> {
    state.connections.delete_profile(&id).await
}

/// Connectivity probe for the form — shares validation, credential, and
/// TLS rules with `connect_database`, then always closes the candidate.
#[tauri::command]
pub async fn test_connection(
    profile: ConnectionProfile,
    password: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<f64> {
    state
        .connections
        .test_connection(&profile, password.filter(|p| !p.is_empty()))
        .await
}

/// `password` is optional: when given (e.g. straight after "Save & Connect")
/// it overrides the keychain, so the just-typed credential is what connects —
/// immune to a stale/failed keychain write.
#[tauri::command]
pub async fn connect_database(
    id: String,
    password: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<f64> {
    let start = Instant::now();
    let result = state
        .connections
        .connect(&id, password.filter(|p| !p.is_empty()))
        .await;
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    let meta = state.connections.session_meta(&id).await;
    state
        .audit
        .record(
            Some(id),
            meta.as_ref().map(|m| m.0.clone()),
            meta.as_ref().map(|m| m.1.clone()),
            "CONNECT DATABASE".to_string(),
            duration_ms,
            result.is_err(),
            result.as_ref().err().map(|e| e.message()),
            None,
        )
        .await;
    result
}

#[tauri::command]
pub async fn disconnect_database(id: String, state: State<'_, AppState>) -> AppResult<()> {
    let meta = state.connections.session_meta(&id).await;
    let result = state.connections.disconnect(&id).await;
    state
        .audit
        .record(
            Some(id),
            meta.as_ref().map(|m| m.0.clone()),
            meta.as_ref().map(|m| m.1.clone()),
            "DISCONNECT DATABASE".to_string(),
            0.0,
            result.is_err(),
            result.as_ref().err().map(|e| e.message()),
            None,
        )
        .await;
    result
}
