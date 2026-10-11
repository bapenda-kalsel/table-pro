use async_trait::async_trait;
use crate::models::{
    DatabaseStats, ExplainResult, ExportResult, FullTableSchema, GlobalSearchResult,
    QueryResultData, StagedCommitPayload, StagedDiffSummary, TableColumnMetadata, TableInfo,
};

pub mod common;
pub mod mysql;
pub mod postgres;
pub mod sqlite;

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    async fn ping(&self) -> Result<f64, String>;
    async fn list_tables(&self) -> Result<Vec<TableInfo>, String>;
    async fn get_table_schema(&self, table: &str) -> Result<Vec<TableColumnMetadata>, String>;
    async fn get_full_schema(&self, table: &str) -> Result<FullTableSchema, String>;
    async fn get_table_data(
        &self,
        table: &str,
        limit: u32,
        offset: u32,
        filter: Option<&str>,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
    ) -> Result<QueryResultData, String>;
    async fn execute_query(&self, sql: &str) -> Result<QueryResultData, String>;
    async fn explain_query(&self, sql: &str) -> Result<ExplainResult, String>;
    async fn preview_staged_changes(&self, payload: &StagedCommitPayload) -> Result<StagedDiffSummary, String>;
    async fn commit_staged_changes(&self, payload: StagedCommitPayload) -> Result<QueryResultData, String>;
    async fn get_table_ddl(&self, table: &str) -> Result<String, String>;
    async fn run_maintenance(&self, action: &str, table: Option<&str>) -> Result<String, String>;
    async fn search_text_in_database(&self, keyword: &str) -> Result<Vec<GlobalSearchResult>, String>;
    /// Export a table to `dest_dir`. `format`: "csv" | "json" | "sql" | "xlsx".
    /// `filter` applies a WHERE clause so exports can respect the grid's active filter.
    async fn export_table(
        &self,
        table: &str,
        format: &str,
        filter: Option<&str>,
        dest_dir: &std::path::Path,
    ) -> Result<ExportResult, String>;
    /// Import a CSV file into `table`. Creates the table (all TEXT columns) when missing.
    async fn import_csv(&self, table: &str, file_path: &str) -> Result<u64, String>;
    /// Backup the whole database into `dest_dir` (SQLite: VACUUM INTO). Returns backup path.
    async fn backup_database(&self, dest_dir: &std::path::Path) -> Result<String, String>;
    /// Server/engine version string for the status bar.
    async fn server_version(&self) -> Result<String, String>;
    /// Workbench-style "Server Status": file size, page stats, object counts.
    async fn get_database_stats(&self) -> Result<DatabaseStats, String>;
    /// phpMyAdmin "visual relations": add a FOREIGN KEY constraint.
    /// SQLite requires a table rebuild internally; returns a summary of what ran.
    async fn add_foreign_key(
        &self,
        constraint_name: &str,
        table: &str,
        column: &str,
        ref_table: &str,
        ref_column: &str,
        on_delete: &str,
        on_update: &str,
    ) -> Result<String, String>;
}
