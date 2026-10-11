//! Operational commands: maintenance, global text search, export/import, backup.

use crate::models::{ExportResult, GlobalSearchResult};
use crate::state::AppState;
use crate::vault;
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub async fn run_maintenance(
    connection_id: String,
    action: String,
    table: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let log_msg = format!("MAINTENANCE: {} {:?}", action, table);
    let start = Instant::now();
    match driver.run_maintenance(&action, table.as_deref()).await {
        Ok(msg) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(Some(connection_id), log_msg, duration, false, None, None)
                .await;
            Ok(msg)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(Some(connection_id), log_msg, duration, true, Some(e.clone()), None)
                .await;
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn search_text_in_database(
    connection_id: String,
    keyword: String,
    state: State<'_, AppState>,
) -> Result<Vec<GlobalSearchResult>, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let log_msg = format!("GLOBAL SEARCH: '{}'", keyword);
    let start = Instant::now();
    match driver.search_text_in_database(&keyword).await {
        Ok(res) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(Some(connection_id), log_msg, duration, false, None, None)
                .await;
            Ok(res)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(Some(connection_id), log_msg, duration, true, Some(e.clone()), None)
                .await;
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn export_table(
    connection_id: String,
    table: String,
    format: String,
    filter: Option<String>,
    state: State<'_, AppState>,
) -> Result<ExportResult, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let dest = vault::get_exports_dir();
    let res = driver
        .export_table(&table, &format, filter.as_deref(), &dest)
        .await?;

    state
        .log_sql(
            Some(connection_id),
            format!("EXPORT TABLE {} -> {}", table, res.file_path),
            0.0,
            false,
            None,
            Some(res.row_count),
        )
        .await;
    Ok(res)
}

#[tauri::command]
pub async fn import_csv(
    connection_id: String,
    table: String,
    file_path: String,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let start = Instant::now();
    match driver.import_csv(&table, &file_path).await {
        Ok(count) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    format!("IMPORT CSV '{}' -> {}", file_path, table),
                    duration,
                    false,
                    None,
                    Some(count),
                )
                .await;
            Ok(count)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    format!("IMPORT CSV '{}' -> {}", file_path, table),
                    duration,
                    true,
                    Some(e.clone()),
                    None,
                )
                .await;
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn backup_database(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let dest = vault::get_backups_dir();
    let res = driver.backup_database(&dest).await?;

    state
        .log_sql(
            Some(connection_id),
            format!("BACKUP DATABASE -> {}", res),
            0.0,
            false,
            None,
            None,
        )
        .await;
    Ok(res)
}

#[tauri::command]
pub async fn get_data_dir(kind: String) -> Result<String, String> {
    let path = match kind.as_str() {
        "exports" => vault::get_exports_dir(),
        "backups" => vault::get_backups_dir(),
        _ => vault::get_config_dir(),
    };
    Ok(path.to_string_lossy().to_string())
}
