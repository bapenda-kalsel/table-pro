//! Persistent query audit log (`audit.jsonl`) — append-only JSONL with
//! compaction, in-memory filtering, stats aggregation and CSV/JSON export.
//!
//! Core logic is path-parametrized so it can be unit-tested against temp
//! files without touching the real `~/.tablepro` directory.

use super::{file_in_config_dir, get_exports_dir};
use crate::errors::{AppError, AppResult};
use crate::models::{AuditStats, SqlLogEntry};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Max audit entries retained when the log file is compacted.
const AUDIT_MAX_ENTRIES: usize = 20_000;

fn get_audit_file() -> std::path::PathBuf {
    file_in_config_dir("audit.jsonl")
}

/// Append a single audit entry to `file`, compacting to the last
/// AUDIT_MAX_ENTRIES lines when the file grows beyond ~320 bytes/entry * cap.
fn append_audit_to(file: &Path, entry: &SqlLogEntry) -> AppResult<()> {
    let line = serde_json::to_string(entry).map_err(AppError::from)?;

    let mut needs_compact = false;
    if let Ok(meta) = fs::metadata(file) {
        if meta.len() > (AUDIT_MAX_ENTRIES as u64) * 320 {
            needs_compact = true;
        }
    }

    if needs_compact {
        let mut entries = load_audit_entries_from(file);
        entries.push(entry.clone());
        let keep_from = entries.len().saturating_sub(AUDIT_MAX_ENTRIES);
        let kept: Vec<String> = entries[keep_from..]
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect();
        fs::write(file, kept.join("\n") + "\n").map_err(AppError::from)?;
        return Ok(());
    }

    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
        .map_err(|e| AppError::storage_at(file, e.to_string()))?;
    writeln!(f, "{}", line).map_err(AppError::from)
}

fn load_audit_entries_from(file: &Path) -> Vec<SqlLogEntry> {
    if !file.exists() {
        return Vec::new();
    }
    match fs::read_to_string(file) {
        Ok(content) => content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str::<SqlLogEntry>(l).ok())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// In-memory filtering of audit entries (kept pure for testability).
fn filter_audit_entries(
    entries: Vec<SqlLogEntry>,
    limit: Option<usize>,
    connection_id: Option<&str>,
    text: Option<&str>,
    errors_only: bool,
    statement_type: Option<&str>,
) -> Vec<SqlLogEntry> {
    let text_lc = text.map(|t| t.to_lowercase());
    let type_uc = statement_type.map(|t| t.to_uppercase());
    let mut out: Vec<SqlLogEntry> = entries
        .into_iter()
        .filter(|e| {
            if errors_only && !e.is_error {
                return false;
            }
            if let Some(cid) = connection_id {
                if e.connection_id.as_deref() != Some(cid) {
                    return false;
                }
            }
            if let Some(ref t) = type_uc {
                if e.statement_type.as_deref().unwrap_or("OTHER") != t.as_str() {
                    return false;
                }
            }
            if let Some(ref needle) = text_lc {
                let in_query = e.query.to_lowercase().contains(needle);
                let in_err = e
                    .error_message
                    .as_ref()
                    .map(|m| m.to_lowercase().contains(needle))
                    .unwrap_or(false);
                if !in_query && !in_err {
                    return false;
                }
            }
            true
        })
        .collect();

    if let Some(n) = limit {
        if out.len() > n {
            out = out[out.len() - n..].to_vec();
        }
    }
    out
}

fn audit_stats_of(entries: &[SqlLogEntry]) -> AuditStats {
    let total = entries.len();
    let error_count = entries.iter().filter(|e| e.is_error).count();
    let mut sum = 0.0;
    let mut max_ms = 0.0_f64;
    let mut slowest_query: Option<String> = None;
    let mut by_type: HashMap<String, usize> = HashMap::new();

    for e in entries {
        sum += e.duration_ms;
        if e.duration_ms > max_ms {
            max_ms = e.duration_ms;
            slowest_query = Some(e.query.clone());
        }
        let key = e
            .statement_type
            .clone()
            .unwrap_or_else(|| "OTHER".to_string());
        *by_type.entry(key).or_insert(0) += 1;
    }

    AuditStats {
        total,
        error_count,
        avg_duration_ms: if total > 0 { sum / total as f64 } else { 0.0 },
        max_duration_ms: max_ms,
        slowest_query,
        by_statement_type: by_type,
    }
}

fn serialize_audit(entries: &[SqlLogEntry], format: &str) -> AppResult<String> {
    if format == "csv" {
        let mut out = String::from(
            "id,timestamp,connection,environment,statement_type,duration_ms,is_error,error_message,query\n",
        );
        for e in entries {
            let esc = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                e.id,
                e.timestamp,
                esc(e.connection_name.as_deref().unwrap_or("")),
                esc(e.environment.as_deref().unwrap_or("")),
                e.statement_type.as_deref().unwrap_or("OTHER"),
                e.duration_ms,
                e.is_error,
                esc(e.error_message.as_deref().unwrap_or("")),
                esc(&e.query),
            ));
        }
        Ok(out)
    } else {
        serde_json::to_string_pretty(entries).map_err(AppError::from)
    }
}

// ---- Public API used by services (wired to the real ~/.tablepro paths) ----

/// Append a single audit entry.
/// Returns an error instead of swallowing it: the service layer decides
/// whether to degrade gracefully or surface a warning to the user.
pub fn append_audit(entry: &SqlLogEntry) -> AppResult<()> {
    append_audit_to(&get_audit_file(), entry)
}

/// Load audit entries filtered by connection / text / error-only / statement type.
/// `text` matches query or error message (case-insensitive substring).
pub fn load_audit_filtered(
    limit: Option<usize>,
    connection_id: Option<&str>,
    text: Option<&str>,
    errors_only: bool,
    statement_type: Option<&str>,
) -> Vec<SqlLogEntry> {
    filter_audit_entries(
        load_audit_entries_from(&get_audit_file()),
        limit,
        connection_id,
        text,
        errors_only,
        statement_type,
    )
}

pub fn clear_audit() -> AppResult<()> {
    let file = get_audit_file();
    if file.exists() {
        fs::write(&file, "").map_err(|e| AppError::storage_at(&file, e.to_string()))?;
    }
    Ok(())
}

pub fn audit_stats() -> AuditStats {
    audit_stats_of(&load_audit_entries_from(&get_audit_file()))
}

/// Serialize audit entries to a CSV or JSON export file inside `~/.tablepro/exports/`.
pub fn export_audit(format: &str) -> AppResult<String> {
    let entries = load_audit_entries_from(&get_audit_file());
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let mut path = get_exports_dir();
    path.push(format!(
        "audit-{}.{}",
        ts,
        if format == "csv" { "csv" } else { "json" }
    ));
    let content = serialize_audit(&entries, format)?;
    fs::write(&path, content).map_err(|e| AppError::storage_at(&path, e.to_string()))?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests;
