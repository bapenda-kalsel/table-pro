//! Schema introspection commands: table list, columns, full schema, DDL,
//! server version and database statistics. Transport only.

use crate::errors::AppResult;
use crate::models::{
    DatabaseStats, ForeignKeySpec, FullTableSchema, TableColumnMetadata, TableInfo,
};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_tables(
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<TableInfo>> {
    let guard = state.connections.acquire(&connection_id).await?;
    state
        .audit
        .run(
            &guard.conn,
            "LIST TABLES".to_string(),
            guard.conn.driver.list_tables(),
            |_| None,
        )
        .await
}

#[tauri::command]
pub async fn get_table_schema(
    connection_id: String,
    table: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<TableColumnMetadata>> {
    let guard = state.connections.acquire(&connection_id).await?;
    state
        .audit
        .run(
            &guard.conn,
            format!("GET TABLE SCHEMA \"{}\"", table),
            guard.conn.driver.get_table_schema(&table),
            |_| None,
        )
        .await
}

#[tauri::command]
pub async fn get_full_schema(
    connection_id: String,
    table: String,
    state: State<'_, AppState>,
) -> AppResult<FullTableSchema> {
    let guard = state.connections.acquire(&connection_id).await?;
    guard.conn.driver.get_full_schema(&table).await
}

#[tauri::command]
pub async fn get_table_ddl(
    connection_id: String,
    table: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let guard = state.connections.acquire(&connection_id).await?;
    guard.conn.driver.get_table_ddl(&table).await
}

#[tauri::command]
pub async fn get_server_version(
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let guard = state.connections.acquire(&connection_id).await?;
    guard.conn.driver.server_version().await
}

#[tauri::command]
pub async fn get_database_stats(
    connection_id: String,
    state: State<'_, AppState>,
) -> AppResult<DatabaseStats> {
    let guard = state.connections.acquire(&connection_id).await?;
    guard.conn.driver.get_database_stats().await
}

/// phpMyAdmin "visual relations": add a FOREIGN KEY between two columns.
/// `on_delete`/`on_update` accept CASCADE / RESTRICT / SET NULL / SET DEFAULT / NO ACTION.
#[tauri::command]
pub async fn add_foreign_key(
    connection_id: String,
    spec: ForeignKeySpec,
    confirm_token: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let guard = state.connections.acquire(&connection_id).await?;
    state.approvals.require(
        &guard.conn,
        &format!(
            "add_fk:{}.{}->{}.{}",
            spec.table, spec.column, spec.ref_table, spec.ref_column
        ),
        confirm_token.as_deref(),
        format!(
            "Add FOREIGN KEY {}.{} → {}.{}?",
            spec.table, spec.column, spec.ref_table, spec.ref_column
        ),
    )?;
    state
        .audit
        .run(
            &guard.conn,
            format!(
                "ALTER TABLE \"{}\" ADD CONSTRAINT \"{}\" FOREIGN KEY (\"{}\") REFERENCES \"{}\"(\"{}\")",
                spec.table, spec.constraint_name, spec.column, spec.ref_table, spec.ref_column
            ),
            guard.conn.driver.add_foreign_key(&spec),
            |_| None,
        )
        .await
}
