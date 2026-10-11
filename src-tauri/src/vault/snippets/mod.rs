//! Saved query snippets (`snippets.json`) — Workbench-style reuse of SQL.

use super::file_in_config_dir;
use crate::models::Snippet;
use std::fs;
use std::path::Path;

fn get_snippets_file() -> std::path::PathBuf {
    file_in_config_dir("snippets.json")
}

fn load_snippets_from(file: &Path) -> Vec<Snippet> {
    if !file.exists() {
        return Vec::new();
    }
    match fs::read_to_string(file) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save_snippet_to(file: &Path, snippet: Snippet) -> Result<(), String> {
    let mut snippets = load_snippets_from(file);
    if let Some(pos) = snippets.iter().position(|s| s.id == snippet.id) {
        snippets[pos] = snippet;
    } else {
        snippets.push(snippet);
    }
    let json = serde_json::to_string_pretty(&snippets).map_err(|e| e.to_string())?;
    fs::write(file, json).map_err(|e| e.to_string())
}

fn delete_snippet_from(file: &Path, id: &str) -> Result<(), String> {
    let mut snippets = load_snippets_from(file);
    snippets.retain(|s| s.id != id);
    let json = serde_json::to_string_pretty(&snippets).map_err(|e| e.to_string())?;
    fs::write(file, json).map_err(|e| e.to_string())
}

pub fn list_snippets() -> Vec<Snippet> {
    load_snippets_from(&get_snippets_file())
}

pub fn save_snippet(snippet: Snippet) -> Result<(), String> {
    save_snippet_to(&get_snippets_file(), snippet)
}

pub fn delete_snippet(id: &str) -> Result<(), String> {
    delete_snippet_from(&get_snippets_file(), id)
}

#[cfg(test)]
mod tests;
