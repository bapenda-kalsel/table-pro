//! Data-plane commands: table data browsing, ad-hoc SQL execution,
//! query plans, and staged-change preview/commit. Transport only —
//! policy and orchestration live in services.

use crate::errors::AppResult;
use crate::models::{
    ExplainResult, QueryResultData, StagedCommitPayload, StagedDiffSummary, TableDataQuery,
};
use crate::services::security::{self, SqlRisk};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_table_data(
    connection_id: String,
    query: TableDataQuery,
    state: State<'_, AppState>,
) -> AppResult<QueryResultData> {
    let guard = state.connections.acquire(&connection_id).await?;

    let label = format!(
        "SELECT * FROM \"{}\" {} LIMIT {} OFFSET {}",
        query.table,
        query
            .filter
            .as_ref()
            .map(|f| format!("WHERE {}", f))
            .unwrap_or_default(),
        query.limit,
        query.offset
    );

    state
        .audit
        .run(
            &guard.conn,
            label,
            guard.conn.driver.get_table_data(
                &query.table,
                query.limit,
                query.offset,
                query.filter.as_deref(),
                query.sort_by.as_deref(),
                query.sort_dir.as_deref(),
            ),
            |d| Some(d.affected_rows),
        )
        .await
}

#[tauri::command]
pub async fn execute_query(
    connection_id: String,
    sql: String,
    confirm_token: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<QueryResultData> {
    let guard = state.connections.acquire(&connection_id).await?;
    // Backend-enforced production guard: the whole batch is analyzed, and
    // anything not provably read-only needs a one-shot approval bound to
    // this exact SQL on this exact session.
    if security::analyze_sql(&sql, &guard.conn.driver_type) != SqlRisk::Read {
        state.approvals.require(
            &guard.conn,
            &sql,
            confirm_token.as_deref(),
            format!(
                "This statement can modify data or schema:\n\n{}",
                truncate_for_display(&sql)
            ),
        )?;
    }
    state
        .audit
        .run(
            &guard.conn,
            sql.clone(),
            guard.conn.driver.execute_query(&sql),
            |d| Some(d.affected_rows),
        )
        .await
}

fn truncate_for_display(s: &str) -> String {
    const MAX: usize = 400;
    if s.len() <= MAX {
        s.to_string()
    } else {
        format!("{}…", &s[..MAX])
    }
}

#[tauri::command]
pub async fn explain_query(
    connection_id: String,
    sql: String,
    state: State<'_, AppState>,
) -> AppResult<ExplainResult> {
    let guard = state.connections.acquire(&connection_id).await?;
    state
        .audit
        .run(
            &guard.conn,
            format!("EXPLAIN QUERY PLAN {}", sql.trim()),
            guard.conn.driver.explain_query(&sql),
            |_| None,
        )
        .await
}

#[tauri::command]
pub async fn preview_staged_changes(
    payload: StagedCommitPayload,
    state: State<'_, AppState>,
) -> AppResult<StagedDiffSummary> {
    let guard = state.connections.acquire(&payload.connection_id).await?;
    guard.conn.driver.preview_staged_changes(&payload).await
}

#[tauri::command]
pub async fn commit_staged_changes(
    payload: StagedCommitPayload,
    confirm_token: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<QueryResultData> {
    let guard = state.connections.acquire(&payload.connection_id).await?;
    // Staged commit is inherently a write — gate before planning.
    let sig = serde_json::to_string(&payload).unwrap_or_else(|_| payload.table_name.clone());
    state.approvals.require(
        &guard.conn,
        &sig,
        confirm_token.as_deref(),
        format!(
            "Commit staged changes to table '{}' ({} inserts, {} updates, {} deletes)?",
            payload.table_name,
            payload.inserts.len(),
            payload.updates.len(),
            payload.deletes.len()
        ),
    )?;
    state
        .audit
        .run(
            &guard.conn,
            "COMMIT STAGED CHANGES".to_string(),
            guard.conn.driver.commit_staged_changes(payload),
            |r| Some(r.affected_rows),
        )
        .await
}
