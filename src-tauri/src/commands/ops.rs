//! Operational commands: maintenance, global text search, export/import, backup.
//! Transport only — policy and orchestration live in services.

use crate::errors::AppResult;
use crate::models::{ExportResult, GlobalSearchResult};
use crate::state::AppState;
use crate::vault;
use tauri::State;

#[tauri::command]
pub async fn run_maintenance(
    connection_id: String,
    action: String,
    table: Option<String>,
    confirm_token: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let guard = state.connections.acquire(&connection_id).await?;
    state.approvals.require(
        &guard.conn,
        &format!("maintenance:{}:{:?}", action, table),
        confirm_token.as_deref(),
        format!(
            "Run maintenance '{}' on {}?",
            action,
            table.as_deref().unwrap_or("the whole database")
        ),
    )?;
    state
        .audit
        .run(
            &guard.conn,
            format!("MAINTENANCE: {} {:?}", action, table),
            guard.conn.driver.run_maintenance(&action, table.as_deref()),
            |_| None,
        )
        .await
}

#[tauri::command]
pub async fn search_text_in_database(
    connection_id: String,
    keyword: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<GlobalSearchResult>> {
    let guard = state.connections.acquire(&connection_id).await?;
    state
        .audit
        .run(
            &guard.conn,
            format!("GLOBAL SEARCH: '{}'", keyword),
            guard.conn.driver.search_text_in_database(&keyword),
            |_| None,
        )
        .await
}

#[tauri::command]
pub async fn export_table(
    connection_id: String,
    table: String,
    format: String,
    filter: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<ExportResult> {
    let guard = state.connections.acquire(&connection_id).await?;
    let dest = vault::get_exports_dir();
    state
        .audit
        .run(
            &guard.conn,
            format!("EXPORT TABLE {} -> {}", table, format),
            guard
                .conn
                .driver
                .export_table(&table, &format, filter.as_deref(), &dest),
            |r| Some(r.row_count),
        )
        .await
}

#[tauri::command]
pub async fn import_csv(
    connection_id: String,
    table: String,
    file_path: String,
    confirm_token: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<u64> {
    let guard = state.connections.acquire(&connection_id).await?;
    state.approvals.require(
        &guard.conn,
        &format!("import_csv:{}:{}", table, file_path),
        confirm_token.as_deref(),
        format!("Import CSV file into table '{}'?", table),
    )?;
    state
        .audit
        .run(
            &guard.conn,
            format!("IMPORT CSV '{}' -> {}", file_path, table),
            guard.conn.driver.import_csv(&table, &file_path),
            |count| Some(*count),
        )
        .await
}

#[tauri::command]
pub async fn backup_database(
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let guard = state.connections.acquire(&connection_id).await?;
    let dest = vault::get_backups_dir();
    state
        .audit
        .run(
            &guard.conn,
            "BACKUP DATABASE".to_string(),
            guard.conn.driver.backup_database(&dest),
            |_| None,
        )
        .await
}

#[tauri::command]
pub async fn get_data_dir(kind: String) -> AppResult<String> {
    let path = match kind.as_str() {
        "exports" => vault::get_exports_dir(),
        "backups" => vault::get_backups_dir(),
        _ => vault::get_config_dir(),
    };
    Ok(path.to_string_lossy().to_string())
}
