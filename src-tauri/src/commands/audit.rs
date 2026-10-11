//! SQL log (in-memory console) + persistent audit trail commands.

use crate::models::{AuditStats, SqlLogEntry};
use crate::state::AppState;
use crate::vault;
use tauri::State;

#[tauri::command]
pub async fn get_sql_logs(state: State<'_, AppState>) -> Result<Vec<SqlLogEntry>, String> {
    let logs = state.logs.read().await;
    Ok(logs.clone())
}

#[tauri::command]
pub async fn clear_sql_logs(state: State<'_, AppState>) -> Result<(), String> {
    let mut logs = state.logs.write().await;
    logs.clear();
    Ok(())
}

#[tauri::command]
pub async fn get_audit_log(
    limit: Option<u32>,
    connection_id: Option<String>,
    text: Option<String>,
    errors_only: Option<bool>,
    statement_type: Option<String>,
) -> Result<Vec<SqlLogEntry>, String> {
    Ok(vault::load_audit_filtered(
        limit.map(|l| l as usize),
        connection_id.as_deref(),
        text.as_deref(),
        errors_only.unwrap_or(false),
        statement_type.as_deref(),
    ))
}

#[tauri::command]
pub async fn get_audit_stats() -> Result<AuditStats, String> {
    Ok(vault::audit_stats())
}

#[tauri::command]
pub async fn clear_audit_log() -> Result<(), String> {
    vault::clear_audit()
}

#[tauri::command]
pub async fn export_audit_log(format: String) -> Result<String, String> {
    vault::export_audit(&format)
}
