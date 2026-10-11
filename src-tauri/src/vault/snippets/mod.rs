//! Saved query snippets (`snippets.json`) — Workbench-style reuse of SQL.

use super::{file_in_config_dir, json_file};
use crate::errors::AppResult;
use crate::models::Snippet;
use std::path::PathBuf;

fn get_snippets_file() -> PathBuf {
    file_in_config_dir("snippets.json")
}

fn load_snippets() -> AppResult<Vec<Snippet>> {
    json_file::load_list(&get_snippets_file())
}

pub fn list_snippets() -> AppResult<Vec<Snippet>> {
    load_snippets()
}

/// Insert-or-replace by `snippet.id` under the file lock.
pub fn save_snippet(snippet: Snippet) -> AppResult<()> {
    json_file::update_list(&get_snippets_file(), move |items: &mut Vec<Snippet>| {
        if let Some(pos) = items.iter().position(|s| s.id == snippet.id) {
            items[pos] = snippet.clone();
        } else {
            items.push(snippet.clone());
        }
        Ok(())
    })
}

pub fn delete_snippet(id: &str) -> AppResult<()> {
    json_file::update_list(&get_snippets_file(), |items: &mut Vec<Snippet>| {
        items.retain(|s| s.id != id);
        Ok(())
    })
}
