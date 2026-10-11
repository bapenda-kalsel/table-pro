//! Saved query snippets (Workbench-style) persisted under ~/.tablepro.

use crate::models::Snippet;
use crate::vault;

#[tauri::command]
pub async fn list_snippets() -> Result<Vec<Snippet>, String> {
    Ok(vault::list_snippets())
}

#[tauri::command]
pub async fn save_snippet(snippet: Snippet) -> Result<(), String> {
    vault::save_snippet(snippet)
}

#[tauri::command]
pub async fn delete_snippet(id: String) -> Result<(), String> {
    vault::delete_snippet(&id)
}
