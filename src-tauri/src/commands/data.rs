//! Data-plane commands: table data browsing, ad-hoc SQL execution,
//! query plans, and staged-change preview/commit.

use crate::models::{
    ExplainResult, QueryResultData, StagedCommitPayload, StagedDiffSummary,
};
use crate::state::AppState;
use std::time::Instant;
use tauri::State;

#[tauri::command]
pub async fn get_table_data(
    connection_id: String,
    table: String,
    limit: u32,
    offset: u32,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
    state: State<'_, AppState>,
) -> Result<QueryResultData, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let sql_log = format!(
        "SELECT * FROM \"{}\" {} LIMIT {} OFFSET {}",
        table,
        filter
            .as_ref()
            .map(|f| format!("WHERE {}", f))
            .unwrap_or_default(),
        limit,
        offset
    );

    let start = Instant::now();
    match driver
        .get_table_data(
            &table,
            limit,
            offset,
            filter.as_deref(),
            sort_by.as_deref(),
            sort_dir.as_deref(),
        )
        .await
    {
        Ok(data) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    sql_log,
                    duration,
                    false,
                    None,
                    Some(data.affected_rows),
                )
                .await;
            Ok(data)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    sql_log,
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
pub async fn execute_query(
    connection_id: String,
    sql: String,
    state: State<'_, AppState>,
) -> Result<QueryResultData, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let start = Instant::now();
    match driver.execute_query(&sql).await {
        Ok(data) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    sql,
                    duration,
                    false,
                    None,
                    Some(data.affected_rows),
                )
                .await;
            Ok(data)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    sql,
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
pub async fn explain_query(
    connection_id: String,
    sql: String,
    state: State<'_, AppState>,
) -> Result<ExplainResult, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let start = Instant::now();
    match driver.explain_query(&sql).await {
        Ok(plan) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    format!("EXPLAIN QUERY PLAN {}", sql.trim()),
                    duration,
                    false,
                    None,
                    None,
                )
                .await;
            Ok(plan)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(connection_id),
                    format!("EXPLAIN QUERY PLAN {}", sql.trim()),
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
pub async fn preview_staged_changes(
    payload: StagedCommitPayload,
    state: State<'_, AppState>,
) -> Result<StagedDiffSummary, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&payload.connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    driver.preview_staged_changes(&payload).await
}

#[tauri::command]
pub async fn commit_staged_changes(
    payload: StagedCommitPayload,
    state: State<'_, AppState>,
) -> Result<QueryResultData, String> {
    let drivers = state.drivers.read().await;
    let driver = drivers
        .get(&payload.connection_id)
        .ok_or_else(|| "Not connected to database".to_string())?;

    let conn_id = payload.connection_id.clone();
    let start = Instant::now();
    match driver.commit_staged_changes(payload).await {
        Ok(res) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(conn_id),
                    "COMMIT STAGED CHANGES".to_string(),
                    duration,
                    false,
                    None,
                    Some(res.affected_rows),
                )
                .await;
            Ok(res)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64() * 1000.0;
            state
                .log_sql(
                    Some(conn_id),
                    "COMMIT STAGED CHANGES".to_string(),
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
