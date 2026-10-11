//! Connection profile CRUD + connect/disconnect + driver construction.

use crate::drivers::mysql::MysqlDriver;
use crate::drivers::postgres::PostgresDriver;
use crate::drivers::sqlite::SqliteDriver;
use crate::drivers::DatabaseDriver;
use crate::models::{ConnectionProfile, DatabaseDriverType};
use crate::state::AppState;
use crate::vault;
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub async fn get_connection_profiles() -> Result<Vec<ConnectionProfile>, String> {
    Ok(vault::load_profiles())
}

#[tauri::command]
pub async fn save_connection_profile(
    profile: ConnectionProfile,
    password: Option<String>,
) -> Result<(), String> {
    if let Some(pwd) = password {
        if !pwd.is_empty() {
            // Surface keychain failures instead of silently leaving a stale
            // credential that every later connect() would read.
            vault::save_password(&profile.id, &pwd)?;
        }
    }
    vault::save_profile(profile)
}

#[tauri::command]
pub async fn delete_connection_profile(id: String) -> Result<(), String> {
    vault::delete_profile(&id)
}

/// Build a live driver for a connection profile (used by test + connect).
/// Password comes from the OS keychain unless `inline_password` is given.
async fn build_driver(
    profile: &ConnectionProfile,
    inline_password: Option<&str>,
) -> Result<Box<dyn DatabaseDriver>, String> {
    match profile.driver {
        DatabaseDriverType::SQLite => {
            let path = profile
                .file_path
                .clone()
                .unwrap_or_else(|| ":memory:".to_string());
            Ok(Box::new(SqliteDriver::new(&path).await?))
        }
        DatabaseDriverType::MySQL => {
            let host = profile.host.clone().unwrap_or_else(|| "localhost".to_string());
            let port = profile.port.unwrap_or(3306);
            let database = profile
                .database
                .clone()
                .filter(|d| !d.is_empty())
                .ok_or("MySQL requires a database name")?;
            let username = profile
                .username
                .clone()
                .filter(|u| !u.is_empty())
                .ok_or("MySQL requires a username")?;
            let password = inline_password
                .map(|p| p.to_string())
                .or_else(|| vault::get_password(&profile.id).ok())
                .unwrap_or_default();
            Ok(Box::new(
                MysqlDriver::new(&host, port, &database, &username, &password).await?,
            ))
        }
        DatabaseDriverType::PostgreSQL => {
            let host = profile.host.clone().unwrap_or_else(|| "localhost".to_string());
            let port = profile.port.unwrap_or(5432);
            let database = profile
                .database
                .clone()
                .filter(|d| !d.is_empty())
                .ok_or("PostgreSQL requires a database name")?;
            let username = profile
                .username
                .clone()
                .filter(|u| !u.is_empty())
                .ok_or("PostgreSQL requires a username")?;
            let password = inline_password
                .map(|p| p.to_string())
                .or_else(|| vault::get_password(&profile.id).ok())
                .unwrap_or_default();
            Ok(Box::new(
                PostgresDriver::new(&host, port, &database, &username, &password).await?,
            ))
        }
    }
}

#[tauri::command]
pub async fn test_connection(
    profile: ConnectionProfile,
    password: Option<String>,
) -> Result<f64, String> {
    // For test_connection the password may not be saved yet — accept it inline.
    let driver = build_driver(&profile, password.as_deref()).await?;
    driver.ping().await
}

/// `password` is optional: when given (e.g. straight after "Save & Connect")
/// it overrides the keychain, so the just-typed credential is what connects —
/// immune to a stale/failed keychain write.
#[tauri::command]
pub async fn connect_database(
    id: String,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<f64, String> {
    let profiles = vault::load_profiles();
    let profile = profiles
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Connection profile '{}' not found", id))?;

    let start = Instant::now();
    let inline = password.filter(|p| !p.is_empty());
    let driver = build_driver(&profile, inline.as_deref()).await?;
    let mut drivers = state.drivers.write().await;
    drivers.insert(id.clone(), driver);
    drop(drivers);

    // Cache name/environment so every audited query carries connection context.
    state
        .register_conn_meta(
            &id,
            profile.name,
            serde_json::to_value(&profile.environment)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "local".to_string()),
        )
        .await;

    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    state
        .log_sql(
            Some(id),
            "CONNECT DATABASE".to_string(),
            duration_ms,
            false,
            None,
            None,
        )
        .await;

    Ok(duration_ms)
}

#[tauri::command]
pub async fn disconnect_database(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut drivers = state.drivers.write().await;
    drivers.remove(&id);
    state
        .log_sql(
            Some(id),
            "DISCONNECT DATABASE".to_string(),
            0.0,
            false,
            None,
            None,
        )
        .await;
    Ok(())
}

#[cfg(test)]
mod tests;
