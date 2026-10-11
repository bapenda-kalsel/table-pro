use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DatabaseDriverType {
    #[serde(rename = "sqlite")]
    SQLite,
    #[serde(rename = "mysql")]
    MySQL,
    #[serde(rename = "postgres")]
    PostgreSQL,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EnvironmentTag {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "staging")]
    Staging,
    #[serde(rename = "production")]
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub driver: DatabaseDriverType,
    pub environment: EnvironmentTag,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub username: Option<String>,
    // Password is NOT stored here; it is fetched from OS Keychain
    pub file_path: Option<String>, // For SQLite
    pub ssl_mode: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableColumnMetadata {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub table_type: String, // "table" or "view"
    pub row_count_estimate: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResultData {
    pub columns: Vec<String>,
    pub column_types: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub execution_time_ms: f64,
    pub affected_rows: u64,
    pub total_rows: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlLogEntry {
    pub id: String,
    pub timestamp: String,
    pub query: String,
    pub duration_ms: f64,
    pub is_error: bool,
    pub error_message: Option<String>,
    pub connection_id: Option<String>,
    #[serde(default)]
    pub connection_name: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub statement_type: Option<String>,
    #[serde(default)]
    pub affected_rows: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    pub total: usize,
    pub error_count: usize,
    pub avg_duration_ms: f64,
    pub max_duration_ms: f64,
    pub slowest_query: Option<String>,
    pub by_statement_type: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub is_unique: bool,
    pub origin: String, // "c" = CREATE INDEX, "u" = UNIQUE constraint, "pk" = PRIMARY KEY
    pub columns: Vec<String>,
    pub is_partial: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyInfo {
    pub id: i64,
    pub referenced_table: String,
    pub from_column: String,
    pub to_column: Option<String>,
    pub on_update: String,
    pub on_delete: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerInfo {
    pub name: String,
    pub table_name: String,
    pub sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTableSchema {
    pub columns: Vec<TableColumnMetadata>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub triggers: Vec<TriggerInfo>,
    pub ddl: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainStep {
    pub id: i64,
    pub parent: i64,
    pub detail: String,
    pub children: Vec<ExplainStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainResult {
    pub steps: Vec<ExplainStep>,
    pub warnings: Vec<String>,
    pub has_full_scan: bool,
    pub uses_index: bool,
    pub uses_temp_btree: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub file_path: String,
    pub row_count: u64,
    pub byte_size: u64,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub driver_name: String,
    pub version: String,
    pub file_path: Option<String>,
    pub file_size_bytes: u64,
    pub page_count: u64,
    pub page_size: u64,
    pub freelist_pages: u64,
    pub journal_mode: String,
    pub encoding: String,
    pub table_count: u64,
    pub view_count: u64,
    pub index_count: u64,
    pub trigger_count: u64,
    pub total_rows: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub sql: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedCommitPayload {
    pub connection_id: String,
    pub table_name: String,
    pub primary_keys: Vec<String>,
    pub inserts: Vec<serde_json::Value>,
    pub updates: Vec<RowUpdatePayload>,
    pub deletes: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowUpdatePayload {
    pub primary_key_values: serde_json::Value, // e.g. { "id": 1 }
    pub updated_fields: serde_json::Value,     // e.g. { "email": "new@corp.com" }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedDiffSummary {
    pub insert_count: usize,
    pub update_count: usize,
    pub delete_count: usize,
    pub generated_sql: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchResult {
    pub table_name: String,
    pub column_name: String,
    pub match_count: usize,
    pub sample_rows: Vec<serde_json::Value>,
}

/// Detect the leading SQL verb of a statement (uppercase), for audit classification.
/// Skips leading `--` line comments and `/* */` block comments.
pub fn detect_statement_type(sql: &str) -> String {
    let mut s = sql.trim_start();
    loop {
        if let Some(rest) = s.strip_prefix("--") {
            match rest.find('\n') {
                Some(idx) => s = rest[idx..].trim_start(),
                None => return "OTHER".to_string(),
            }
        } else if let Some(rest) = s.strip_prefix("/*") {
            match rest.find("*/") {
                Some(idx) => s = rest[idx + 2..].trim_start(),
                None => return "OTHER".to_string(),
            }
        } else {
            break;
        }
    }
    let first_word: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    if first_word.is_empty() {
        "OTHER".to_string()
    } else {
        first_word.to_uppercase()
    }
}

#[cfg(test)]
mod tests;
