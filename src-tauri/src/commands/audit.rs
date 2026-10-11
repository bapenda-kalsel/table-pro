//! SQL log (in-memory console) + persistent audit trail commands.

use crate::errors::AppResult;
use crate::models::{AuditStats, SqlLogEntry};
use crate::state::AppState;
use crate::vault;
use tauri::State;

#[tauri::command]
pub async fn get_sql_logs(state: State<'_, AppState>) -> AppResult<Vec<SqlLogEntry>> {
    Ok(state.audit.console_entries().await)
}

#[tauri::command]
pub async fn clear_sql_logs(state: State<'_, AppState>) -> AppResult<()> {
    state.audit.clear_console().await;
    Ok(())
}

/// Whether durable audit persistence has failed — the UI should warn the
/// user that the on-disk trail may be incomplete.
#[tauri::command]
pub async fn audit_storage_degraded(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.audit.is_degraded())
}

#[tauri::command]
pub async fn get_audit_log(
    limit: Option<u32>,
    connection_id: Option<String>,
    text: Option<String>,
    errors_only: Option<bool>,
    statement_type: Option<String>,
) -> AppResult<Vec<SqlLogEntry>> {
    Ok(vault::load_audit_filtered(
        limit.map(|l| l as usize),
        connection_id.as_deref(),
        text.as_deref(),
        errors_only.unwrap_or(false),
        statement_type.as_deref(),
    ))
}

#[tauri::command]
pub async fn get_audit_stats() -> AppResult<AuditStats> {
    Ok(vault::audit_stats())
}

#[tauri::command]
pub async fn clear_audit_log() -> AppResult<()> {
    vault::clear_audit()
}

#[tauri::command]
pub async fn export_audit_log(format: String) -> AppResult<String> {
    vault::export_audit(&format)
}
