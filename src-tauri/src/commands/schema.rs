//! Schema introspection commands: table list, columns, full schema, DDL,
//! server version and database statistics.

use crate::models::{
    DatabaseStats, FullTableSchema, TableColumnMetadata, TableInfo,
};
use crate::state::AppState;
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub async fn list_tables(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TableInfo>, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let start = Instant::now();
    match driver.list_tables().await {
        Ok(tables) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    "LIST TABLES".to_string(),
                    duration,
                    false,
                    None,
                    None,
                )
                .await;
            Ok(tables)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    "LIST TABLES".to_string(),
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
pub async fn get_table_schema(
    connection_id: String,
    table: String,
    state: State<'_, AppState>,
) -> Result<Vec<TableColumnMetadata>, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let start = Instant::now();
    let query_label = format!("PRAGMA table_info(\"{}\")", table);
    match driver.get_table_schema(&table).await {
        Ok(cols) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(Some(connection_id), query_label, duration, false, None, None)
                .await;
            Ok(cols)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    query_label,
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
pub async fn get_full_schema(
    connection_id: String,
    table: String,
    state: State<'_, AppState>,
) -> Result<FullTableSchema, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;
    driver.get_full_schema(&table).await
}

#[tauri::command]
pub async fn get_table_ddl(
    connection_id: String,
    table: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;
    driver.get_table_ddl(&table).await
}

#[tauri::command]
pub async fn get_server_version(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;
    driver.server_version().await
}

#[tauri::command]
pub async fn get_database_stats(
    connection_id: String,
    state: State<'_, AppState>,
) -> Result<DatabaseStats, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;
    driver.get_database_stats().await
}

/// phpMyAdmin "visual relations": add a FOREIGN KEY between two columns.
/// `on_delete`/`on_update` accept CASCADE / RESTRICT / SET NULL / SET DEFAULT / NO ACTION.
#[tauri::command]
pub async fn add_foreign_key(
    connection_id: String,
    constraint_name: String,
    table: String,
    column: String,
    ref_table: String,
    ref_column: String,
    on_delete: String,
    on_update: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let start = Instant::now();
    let label = format!(
        "ALTER TABLE \"{}\" ADD CONSTRAINT \"{}\" FOREIGN KEY (\"{}\") REFERENCES \"{}\"(\"{}\")",
        table, constraint_name, column, ref_table, ref_column
    );
    match driver
        .add_foreign_key(
            &constraint_name,
            &table,
            &column,
            &ref_table,
            &ref_column,
            &on_delete,
            &on_update,
        )
        .await
    {
        Ok(msg) => {
            state
                .log_sql(
                    Some(connection_id),
                    label,
                    start.elapsed().as_secs_f64() * 1000.0,
                    false,
                    None,
                    None,
                )
                .await;
            Ok(msg)
        }
        Err(e) => {
            state
                .log_sql(
                    Some(connection_id),
                    label,
                    start.elapsed().as_secs_f64() * 1000.0,
                    true,
                    Some(e.clone()),
                    None,
                )
                .await;
            Err(e)
        }
    }
}
