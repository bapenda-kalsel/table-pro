use crate::errors::AppResult;
use crate::models::{
    DatabaseStats, ExplainResult, ExportResult, ForeignKeySpec, FullTableSchema,
    GlobalSearchResult, QueryResultData, StagedCommitPayload, StagedDiffSummary,
    TableColumnMetadata, TableInfo,
};
use async_trait::async_trait;

pub mod common;
pub mod factory;
pub mod mysql;
pub mod postgres;
pub mod sqlite;

/// Database capabilities contract — data-plane only. Application policy
/// (production guards, approvals, audit, connection lifecycle) lives in the
/// service layer and must not leak into drivers.
#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    async fn ping(&self) -> AppResult<f64>;
    async fn list_tables(&self) -> AppResult<Vec<TableInfo>>;
    async fn get_table_schema(&self, table: &str) -> AppResult<Vec<TableColumnMetadata>>;
    async fn get_full_schema(&self, table: &str) -> AppResult<FullTableSchema>;
    async fn get_table_data(
        &self,
        table: &str,
        limit: u32,
        offset: u32,
        filter: Option<&str>,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
    ) -> AppResult<QueryResultData>;
    async fn execute_query(&self, sql: &str) -> AppResult<QueryResultData>;
    async fn explain_query(&self, sql: &str) -> AppResult<ExplainResult>;
    async fn preview_staged_changes(
        &self,
        payload: &StagedCommitPayload,
    ) -> AppResult<StagedDiffSummary>;
    async fn commit_staged_changes(
        &self,
        payload: StagedCommitPayload,
    ) -> AppResult<QueryResultData>;
    async fn get_table_ddl(&self, table: &str) -> AppResult<String>;
    async fn run_maintenance(&self, action: &str, table: Option<&str>) -> AppResult<String>;
    async fn search_text_in_database(&self, keyword: &str) -> AppResult<Vec<GlobalSearchResult>>;
    /// Export a table to `dest_dir`. `format`: "csv" | "json" | "sql" | "xlsx".
    /// `filter` applies a WHERE clause so exports can respect the grid's active filter.
    async fn export_table(
        &self,
        table: &str,
        format: &str,
        filter: Option<&str>,
        dest_dir: &std::path::Path,
    ) -> AppResult<ExportResult>;
    /// Import a CSV file into `table`. Creates the table (all TEXT columns) when missing.
    async fn import_csv(&self, table: &str, file_path: &str) -> AppResult<u64>;
    /// Backup the whole database into `dest_dir` (SQLite: VACUUM INTO). Returns backup path.
    async fn backup_database(&self, dest_dir: &std::path::Path) -> AppResult<String>;
    /// Server/engine version string for the status bar.
    async fn server_version(&self) -> AppResult<String>;
    /// Workbench-style "Server Status": file size, page stats, object counts.
    async fn get_database_stats(&self) -> AppResult<DatabaseStats>;
    /// phpMyAdmin "visual relations": add a FOREIGN KEY constraint.
    /// SQLite requires a table rebuild internally; returns a summary of what ran.
    async fn add_foreign_key(&self, spec: &ForeignKeySpec) -> AppResult<String>;
    /// Gracefully close all underlying connections. Called on disconnect;
    /// dropping the last driver handle alone does not guarantee pools close.
    async fn close(&self);
}
