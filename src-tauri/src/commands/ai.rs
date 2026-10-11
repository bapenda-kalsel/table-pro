use crate::errors::{AppError, AppResult};
use crate::models::TableColumnMetadata;
use crate::services::ai::{schema_context, AiConfigView};
use crate::state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGenerateResult {
    pub sql: String,
}

#[tauri::command]
pub async fn ai_get_config(state: State<'_, AppState>) -> AppResult<AiConfigView> {
    state.ai.get_config().await
}

#[tauri::command]
pub async fn ai_save_config(
    provider: String,
    model: String,
    base_url: Option<String>,
    api_key: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<AiConfigView> {
    state
        .ai
        .save_config(provider, model, base_url, api_key)
        .await
}

const MAX_SCHEMA_TABLES: usize = 30;

#[tauri::command]
pub async fn ai_generate(
    connection_id: String,
    prompt: String,
    current_sql: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<AiGenerateResult> {
    if prompt.trim().is_empty() {
        return Err(AppError::validation("Describe what the SQL should do"));
    }
    let guard = state.connections.acquire(&connection_id).await?;

    // Bounded schema context: names + types only, no row data.
    let tables = guard.conn.driver.list_tables().await?;
    let mut context: Vec<(String, Vec<TableColumnMetadata>)> = Vec::new();
    for t in tables
        .iter()
        .filter(|t| t.table_type == "table")
        .take(MAX_SCHEMA_TABLES)
    {
        match guard.conn.driver.get_table_schema(&t.name).await {
            Ok(cols) => context.push((t.name.clone(), cols)),
            Err(_) => context.push((t.name.clone(), vec![])),
        }
    }
    let schema_text = schema_context(&context, MAX_SCHEMA_TABLES);

    state
        .audit
        .run(
            &guard.conn,
            "AI GENERATE SQL".to_string(),
            state.ai.generate_sql(
                &guard.conn.driver_type,
                &schema_text,
                prompt.trim(),
                current_sql.as_deref(),
            ),
            |_| None,
        )
        .await
        .map(|sql| AiGenerateResult { sql })
}
