//! Saved query snippets (Workbench-style) persisted under ~/.tablepro.

use crate::errors::AppResult;
use crate::models::Snippet;
use crate::vault;

#[tauri::command]
pub async fn list_snippets() -> AppResult<Vec<Snippet>> {
    vault::list_snippets()
}

#[tauri::command]
pub async fn save_snippet(snippet: Snippet) -> AppResult<()> {
    vault::save_snippet(snippet)
}

#[tauri::command]
pub async fn delete_snippet(id: String) -> AppResult<()> {
    vault::delete_snippet(&id)
}
