use super::common::{staged, Dialect, ParamStyle};
use super::DatabaseDriver;
use crate::errors::{AppError, AppResult, ResultExt};
use crate::models::{
    DatabaseStats, ExplainResult, ExplainStep, ExportResult, ForeignKeyInfo, ForeignKeySpec,
    FullTableSchema, GlobalSearchResult, IndexInfo, QueryResultData, StagedCommitPayload,
    StagedDiffSummary, TableColumnMetadata, TableInfo, TriggerInfo,
};
use async_trait::async_trait;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::{Column, Row, TypeInfo};
use std::time::Instant;

pub struct SqliteDriver {
    pool: SqlitePool,
    db_path: String,
}

/// Quote a SQLite identifier, escaping embedded double-quotes.
fn qi(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Bind a JSON scalar to a SQLite query placeholder.
fn bind_json<'q>(
    q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    v: &'q serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    match v {
        serde_json::Value::Null => q.bind(None::<String>),
        serde_json::Value::Bool(b) => q.bind(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                q.bind(i)
            } else {
                q.bind(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => q.bind(s.as_str()),
        // Rejected by staged-plan validation; render as text if reached.
        other => q.bind(other.to_string()),
    }
}

impl SqliteDriver {
    pub async fn new(path: &str) -> AppResult<Self> {
        let conn_str = if path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            // Auto create SQLite file if not exists
            if !path.starts_with("sqlite:") {
                format!("sqlite://{}?mode=rwc", path)
            } else {
                path.to_string()
            }
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&conn_str)
            .await
            .db_with(|e| format!("Failed to open SQLite database: {}", e))?;

        Ok(Self {
            pool,
            db_path: path.to_string(),
        })
    }

    fn row_to_json(row: &SqliteRow) -> Vec<serde_json::Value> {
        let mut values = Vec::new();
        for (i, _col) in row.columns().iter().enumerate() {
            let val = if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
                match v {
                    Some(num) => serde_json::json!(num),
                    None => serde_json::Value::Null,
                }
            } else if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
                match v {
                    Some(num) => serde_json::json!(num),
                    None => serde_json::Value::Null,
                }
            } else if let Ok(v) = row.try_get::<Option<String>, _>(i) {
                match v {
                    Some(s) => serde_json::json!(s),
                    None => serde_json::Value::Null,
                }
            } else if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
                match v {
                    Some(b) => serde_json::json!(b),
                    None => serde_json::Value::Null,
                }
            } else if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(i) {
                match v {
                    Some(bytes) => serde_json::json!(format!("<BLOB {} bytes>", bytes.len())),
                    None => serde_json::Value::Null,
                }
            } else {
                serde_json::Value::Null
            };
            values.push(val);
        }
        values
    }

    fn row_to_map(row: &SqliteRow) -> serde_json::Map<String, serde_json::Value> {
        let vals = Self::row_to_json(row);
        let mut obj = serde_json::Map::new();
        for (i, c) in row.columns().iter().enumerate() {
            obj.insert(
                c.name().to_string(),
                vals.get(i).cloned().unwrap_or(serde_json::Value::Null),
            );
        }
        obj
    }

    /// Literal escaping used by generated SQL (exports & staged-change diff).
    fn sql_literal(v: &serde_json::Value) -> String {
        match v {
            serde_json::Value::Null => "NULL".to_string(),
            serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
            _ => format!("'{}'", v.to_string().replace('\'', "''")),
        }
    }

    /// Validate staged payload against live metadata and build the bound plan.
    async fn staged_plan(
        &self,
        payload: &StagedCommitPayload,
    ) -> AppResult<Vec<staged::BoundStatement>> {
        let cols = self.get_table_schema(&payload.table_name).await?;
        let pk: Vec<String> = cols
            .iter()
            .filter(|c| c.is_primary_key)
            .map(|c| c.name.clone())
            .collect();
        let all: Vec<String> = cols.iter().map(|c| c.name.clone()).collect();
        staged::build_staged_plan(Dialect::Ansi, ParamStyle::Question, payload, &pk, &all)
    }

    /// Full logical backup: DDL + INSERTs for all tables, plus views/triggers/indexes.
    /// Used for :memory: databases (which cannot VACUUM INTO through a shared-cache
    /// pool) and as a fallback when a binary backup fails.
    async fn dump_database(
        &self,
        dest_dir: &std::path::Path,
        base: &str,
        ts: &str,
    ) -> AppResult<String> {
        let mut out =
            String::from("-- TablePro backup\nPRAGMA foreign_keys=off;\nBEGIN TRANSACTION;\n\n");

        for t in self.list_tables().await? {
            if t.table_type != "table" {
                continue;
            }
            if let Ok(ddl) = self.get_table_ddl(&t.name).await {
                out.push_str(&format!("{};\n", ddl.trim_end_matches(';')));
            }
            let rows = sqlx::query(&format!("SELECT * FROM {}", qi(&t.name)))
                .fetch_all(&self.pool)
                .await
                .map_err(AppError::from)?;
            for r in &rows {
                let map = Self::row_to_map(r);
                let cols: Vec<String> = map.keys().map(|k| qi(k)).collect();
                let vals: Vec<String> = map.values().map(Self::sql_literal).collect();
                out.push_str(&format!(
                    "INSERT INTO {} ({}) VALUES ({});\n",
                    qi(&t.name),
                    cols.join(", "),
                    vals.join(", ")
                ));
            }
            out.push('\n');
        }

        // Views, triggers, and user indexes (DDL already emitted for tables above).
        let extras = sqlx::query(
            "SELECT sql FROM sqlite_master WHERE sql IS NOT NULL AND type IN ('view','trigger','index') AND name NOT LIKE 'sqlite_%';",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        for r in extras {
            let s: String = r.try_get("sql").unwrap_or_default();
            if !s.trim().is_empty() {
                out.push_str(&format!("{};\n", s.trim_end_matches(';')));
            }
        }

        out.push_str("\nCOMMIT;\n");

        let mut path = dest_dir.to_path_buf();
        path.push(format!("{}-{}.sql", base, ts));
        std::fs::write(&path, out)
            .map_err(|e| AppError::storage_at(&path, format!("Backup dump failed: {}", e)))?;
        Ok(path.to_string_lossy().to_string())
    }
}

#[async_trait]
impl DatabaseDriver for SqliteDriver {
    async fn ping(&self) -> AppResult<f64> {
        let start = Instant::now();
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    }

    async fn list_tables(&self) -> AppResult<Vec<TableInfo>> {
        let rows = sqlx::query(
            "SELECT name, type FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY name ASC;",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        let mut tables = Vec::new();
        for row in rows {
            let name: String = row.try_get("name").unwrap_or_default();
            let table_type: String = row.try_get("type").unwrap_or_else(|_| "table".to_string());

            // Get row count estimate (skipped for views — COUNT(*) on a view can be expensive)
            let row_count: Option<i64> = if table_type == "view" {
                None
            } else {
                let count_query = format!("SELECT COUNT(*) as cnt FROM {}", qi(&name));
                match sqlx::query(&count_query).fetch_one(&self.pool).await {
                    Ok(c_row) => c_row.try_get("cnt").ok(),
                    Err(_) => None,
                }
            };

            tables.push(TableInfo {
                name,
                table_type,
                row_count_estimate: row_count,
            });
        }

        Ok(tables)
    }

    async fn get_table_schema(&self, table: &str) -> AppResult<Vec<TableColumnMetadata>> {
        let query = format!("PRAGMA table_info({});", qi(table));
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        let mut columns = Vec::new();
        for row in rows {
            let name: String = row.try_get("name").unwrap_or_default();
            let data_type: String = row.try_get("type").unwrap_or_else(|_| "TEXT".to_string());
            let notnull: i64 = row.try_get("notnull").unwrap_or(0);
            let pk: i64 = row.try_get("pk").unwrap_or(0);
            let dflt_value: Option<String> = row.try_get("dflt_value").ok();

            columns.push(TableColumnMetadata {
                name,
                data_type,
                is_nullable: notnull == 0,
                is_primary_key: pk > 0,
                default_value: dflt_value,
                comment: None,
            });
        }

        Ok(columns)
    }

    async fn get_full_schema(&self, table: &str) -> AppResult<FullTableSchema> {
        let columns = self.get_table_schema(table).await?;

        // --- Indexes ---
        let idx_rows = sqlx::query(&format!("PRAGMA index_list({});", qi(table)))
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        let mut indexes = Vec::new();
        for ir in idx_rows {
            let idx_name: String = ir.try_get("name").unwrap_or_default();
            let unique: i64 = ir.try_get("unique").unwrap_or(0);
            let origin: String = ir.try_get("origin").unwrap_or_else(|_| "c".to_string());
            let partial: i64 = ir.try_get("partial").unwrap_or(0);

            let col_rows = sqlx::query(&format!("PRAGMA index_info({});", qi(&idx_name)))
                .fetch_all(&self.pool)
                .await
                .unwrap_or_default();
            let cols: Vec<String> = col_rows
                .iter()
                .map(|r| r.try_get::<String, _>("name").unwrap_or_default())
                .collect();

            indexes.push(IndexInfo {
                name: idx_name,
                is_unique: unique == 1,
                origin,
                columns: cols,
                is_partial: partial == 1,
            });
        }

        // --- Foreign Keys ---
        let fk_rows = sqlx::query(&format!("PRAGMA foreign_key_list({});", qi(table)))
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        let foreign_keys: Vec<ForeignKeyInfo> = fk_rows
            .iter()
            .map(|r| ForeignKeyInfo {
                id: r.try_get::<i64, _>("id").unwrap_or(0),
                referenced_table: r.try_get::<String, _>("table").unwrap_or_default(),
                from_column: r.try_get::<String, _>("from").unwrap_or_default(),
                to_column: r.try_get::<Option<String>, _>("to").unwrap_or_default(),
                on_update: r
                    .try_get::<String, _>("on_update")
                    .unwrap_or_else(|_| "NO ACTION".into()),
                on_delete: r
                    .try_get::<String, _>("on_delete")
                    .unwrap_or_else(|_| "NO ACTION".into()),
            })
            .collect();

        // --- Triggers on this table ---
        let trig_rows = sqlx::query(
            "SELECT name, tbl_name, sql FROM sqlite_master WHERE type = 'trigger' AND tbl_name = $1 ORDER BY name;",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        let triggers: Vec<TriggerInfo> = trig_rows
            .iter()
            .map(|r| TriggerInfo {
                name: r.try_get::<String, _>("name").unwrap_or_default(),
                table_name: r.try_get::<String, _>("tbl_name").unwrap_or_default(),
                sql: r.try_get::<String, _>("sql").unwrap_or_default(),
            })
            .collect();

        let ddl = self.get_table_ddl(table).await.ok();

        Ok(FullTableSchema {
            columns,
            indexes,
            foreign_keys,
            triggers,
            ddl,
        })
    }

    async fn get_table_data(
        &self,
        table: &str,
        limit: u32,
        offset: u32,
        filter: Option<&str>,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
    ) -> AppResult<QueryResultData> {
        let start = Instant::now();

        // Resolve the real column list up-front: used to render headers when the
        // result set is empty and to validate the sort column name.
        let schema_cols = self.get_table_schema(table).await.unwrap_or_default();
        let valid_cols: Vec<&str> = schema_cols.iter().map(|c| c.name.as_str()).collect();

        let mut query = format!("SELECT * FROM {}", qi(table));
        if let Some(f) = filter {
            if !f.trim().is_empty() {
                query.push_str(&format!(" WHERE {}", f));
            }
        }

        if let Some(col) = sort_by {
            if valid_cols.contains(&col) {
                let dir = match sort_dir.unwrap_or("ASC").to_uppercase().as_str() {
                    "DESC" => "DESC",
                    _ => "ASC",
                };
                query.push_str(&format!(" ORDER BY {} {}", qi(col), dir));
            }
        }

        query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .db_with(|e| format!("Query failed: {}", e))?;

        let execution_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        let mut columns = Vec::new();
        let mut column_types = Vec::new();
        if let Some(first_row) = rows.first() {
            for col in first_row.columns() {
                columns.push(col.name().to_string());
                column_types.push(col.type_info().name().to_string());
            }
        } else {
            // Empty result set still needs headers for the grid/insert buffer.
            for c in &schema_cols {
                columns.push(c.name.clone());
                column_types.push(c.data_type.clone());
            }
        }

        let mut result_rows = Vec::new();
        for r in &rows {
            result_rows.push(Self::row_to_json(r));
        }

        // Get total rows in table (respecting the active filter for pagination)
        let mut count_sql = format!("SELECT COUNT(*) as total FROM {}", qi(table));
        if let Some(f) = filter {
            if !f.trim().is_empty() {
                count_sql.push_str(&format!(" WHERE {}", f));
            }
        }
        let total_rows: u64 = sqlx::query(&count_sql)
            .fetch_one(&self.pool)
            .await
            .map(|r| r.try_get::<i64, _>("total").unwrap_or(0) as u64)
            .unwrap_or(result_rows.len() as u64);

        Ok(QueryResultData {
            columns,
            column_types,
            rows: result_rows,
            execution_time_ms,
            affected_rows: rows.len() as u64,
            total_rows,
        })
    }

    async fn execute_query(&self, sql: &str) -> AppResult<QueryResultData> {
        let start = Instant::now();
        let trimmed = sql.trim();
        let upper = trimmed.to_uppercase();

        if upper.starts_with("SELECT")
            || upper.starts_with("PRAGMA")
            || upper.starts_with("EXPLAIN")
            || upper.starts_with("WITH")
            || upper.starts_with("VALUES")
        {
            let rows = sqlx::query(sql)
                .fetch_all(&self.pool)
                .await
                .map_err(AppError::from)?;

            let execution_time_ms = start.elapsed().as_secs_f64() * 1000.0;

            let mut columns = Vec::new();
            let mut column_types = Vec::new();
            if let Some(first_row) = rows.first() {
                for col in first_row.columns() {
                    columns.push(col.name().to_string());
                    column_types.push(col.type_info().name().to_string());
                }
            }

            let mut result_rows = Vec::new();
            for r in &rows {
                result_rows.push(Self::row_to_json(r));
            }

            Ok(QueryResultData {
                columns,
                column_types,
                rows: result_rows,
                execution_time_ms,
                affected_rows: rows.len() as u64,
                total_rows: rows.len() as u64,
            })
        } else {
            let res = sqlx::query(sql)
                .execute(&self.pool)
                .await
                .map_err(AppError::from)?;

            let execution_time_ms = start.elapsed().as_secs_f64() * 1000.0;

            Ok(QueryResultData {
                columns: vec!["Affected Rows".to_string()],
                column_types: vec!["INTEGER".to_string()],
                rows: vec![vec![serde_json::json!(res.rows_affected())]],
                execution_time_ms,
                affected_rows: res.rows_affected(),
                total_rows: 1,
            })
        }
    }

    async fn explain_query(&self, sql: &str) -> AppResult<ExplainResult> {
        let trimmed = sql.trim().trim_end_matches(';');
        let explain_sql = format!("EXPLAIN QUERY PLAN {}", trimmed);

        let rows = sqlx::query(&explain_sql)
            .fetch_all(&self.pool)
            .await
            .db_with(|e| format!("EXPLAIN failed: {}", e))?;

        // Flat steps: (id, parent, detail)
        let mut flat: Vec<(i64, i64, String)> = Vec::new();
        for r in &rows {
            let id: i64 = r.try_get(0).unwrap_or(0);
            let parent: i64 = r.try_get(1).unwrap_or(0);
            let detail: String = r.try_get(3).unwrap_or_default();
            flat.push((id, parent, detail));
        }

        // Build tree from flat (id, parent) pairs.
        fn build_tree(parent_id: i64, flat: &[(i64, i64, String)]) -> Vec<ExplainStep> {
            flat.iter()
                .filter(|(_, p, _)| *p == parent_id)
                .map(|(id, _, detail)| ExplainStep {
                    id: *id,
                    parent: parent_id,
                    detail: detail.clone(),
                    children: build_tree(*id, flat),
                })
                .collect()
        }
        // Root nodes have parent = 0, but a parent that isn't itself present also counts as root.
        let ids: std::collections::HashSet<i64> = flat.iter().map(|(id, _, _)| *id).collect();
        let mut steps: Vec<ExplainStep> = flat
            .iter()
            .filter(|(_, p, _)| !ids.contains(p))
            .map(|(id, p, detail)| ExplainStep {
                id: *id,
                parent: *p,
                detail: detail.clone(),
                children: build_tree(*id, &flat),
            })
            .collect();
        // Fallback for engines reporting all roots under parent 0.
        if steps.is_empty() {
            steps = build_tree(0, &flat);
        }

        // --- Audit warnings derived from the plan ---
        let mut warnings = Vec::new();
        let mut has_full_scan = false;
        let mut uses_index = false;
        let mut uses_temp_btree = false;

        for (_, _, detail) in &flat {
            let d = detail.to_uppercase();
            if d.contains("SCAN")
                && !d.contains("USING INDEX")
                && !d.contains("USING COVERING INDEX")
            {
                has_full_scan = true;
                warnings.push(format!(
                    "Full table scan: '{}' — consider adding an index on the filtered columns.",
                    detail
                ));
            }
            if d.contains("USING INDEX") || d.contains("USING COVERING INDEX") {
                uses_index = true;
            }
            if d.contains("TEMP B-TREE") || d.contains("TEMPORARY") {
                uses_temp_btree = true;
                warnings.push(format!(
                    "'{}' — requires a temporary B-tree (sort/group materialization). An index on the ORDER/GROUP columns may avoid it.",
                    detail
                ));
            }
        }
        if !uses_index && has_full_scan {
            warnings.push("No index was used by this query.".to_string());
        }
        if warnings.is_empty() {
            warnings.push("Plan looks healthy: index access detected, no full scans.".to_string());
        }

        Ok(ExplainResult {
            steps,
            warnings,
            has_full_scan,
            uses_index,
            uses_temp_btree,
        })
    }

    async fn preview_staged_changes(
        &self,
        payload: &StagedCommitPayload,
    ) -> AppResult<StagedDiffSummary> {
        let plan = self.staged_plan(payload).await?;
        Ok(staged::diff_from_plan(payload, &plan))
    }

    async fn commit_staged_changes(
        &self,
        payload: StagedCommitPayload,
    ) -> AppResult<QueryResultData> {
        let plan = self.staged_plan(&payload).await?;
        let start = Instant::now();

        let mut tx = self
            .pool
            .begin()
            .await
            .db_with(|e| format!("Transaction begin failed: {}", e))?;

        let mut total_affected = 0;
        for stmt in &plan {
            let mut q = sqlx::query(&stmt.sql);
            for v in &stmt.binds {
                q = bind_json(q, v);
            }
            let res = q
                .execute(&mut *tx)
                .await
                .db_with(|e| format!("Failed executing '{}': {}", stmt.preview, e))?;
            total_affected += res.rows_affected();
        }

        tx.commit()
            .await
            .db_with(|e| format!("Transaction commit failed: {}", e))?;

        let execution_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        Ok(QueryResultData {
            columns: vec![
                "Committed Statements".to_string(),
                "Total Affected".to_string(),
            ],
            column_types: vec!["INTEGER".to_string(), "INTEGER".to_string()],
            rows: vec![vec![
                serde_json::json!(plan.len()),
                serde_json::json!(total_affected),
            ]],
            execution_time_ms,
            affected_rows: total_affected,
            total_rows: 1,
        })
    }

    async fn get_table_ddl(&self, table: &str) -> AppResult<String> {
        let row = sqlx::query("SELECT sql FROM sqlite_master WHERE name = $1;")
            .bind(table)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?;

        match row {
            Some(r) => Ok(r.try_get("sql").unwrap_or_default()),
            None => Err(AppError::not_found(format!("Table '{}' not found", table))),
        }
    }

    async fn run_maintenance(&self, action: &str, table: Option<&str>) -> AppResult<String> {
        match action.to_lowercase().as_str() {
            "vacuum" => {
                sqlx::query("VACUUM;")
                    .execute(&self.pool)
                    .await
                    .map_err(AppError::from)?;
                Ok(
                    "Database VACUUM completed successfully. Unused disk space reclaimed."
                        .to_string(),
                )
            }
            "analyze" => {
                let sql = match table {
                    Some(t) => format!("ANALYZE {};", qi(t)),
                    None => "ANALYZE;".to_string(),
                };
                sqlx::query(&sql)
                    .execute(&self.pool)
                    .await
                    .map_err(AppError::from)?;
                Ok(format!(
                    "{} completed — query planner statistics refreshed.",
                    sql.trim_end_matches(';')
                ))
            }
            "reindex" => {
                let sql = match table {
                    Some(t) => format!("REINDEX {};", qi(t)),
                    None => "REINDEX;".to_string(),
                };
                sqlx::query(&sql)
                    .execute(&self.pool)
                    .await
                    .map_err(AppError::from)?;
                Ok(format!(
                    "{} completed — indexes rebuilt.",
                    sql.trim_end_matches(';')
                ))
            }
            "integrity_check" => {
                let rows = sqlx::query("PRAGMA integrity_check;")
                    .fetch_all(&self.pool)
                    .await
                    .map_err(AppError::from)?;

                let mut results = Vec::new();
                for r in rows {
                    let msg: String = r.try_get(0).unwrap_or_default();
                    results.push(msg);
                }
                Ok(results.join("\n"))
            }
            _ => Err(AppError::unsupported(format!(
                "Unsupported maintenance action for SQLite: {}",
                action
            ))),
        }
    }

    async fn search_text_in_database(&self, keyword: &str) -> AppResult<Vec<GlobalSearchResult>> {
        let tables = self.list_tables().await?;
        let mut results = Vec::new();
        let sanitized_keyword = keyword.replace('\'', "''");

        for table in tables {
            let schema = self.get_table_schema(&table.name).await?;
            for col in schema {
                let dt = col.data_type.to_uppercase();
                // Columns without a declared type can still hold text in SQLite.
                let is_text = dt.contains("CHAR")
                    || dt.contains("TEXT")
                    || dt.contains("CLOB")
                    || dt.is_empty();

                if is_text {
                    let search_sql = format!(
                        "SELECT * FROM {} WHERE {} LIKE '%{}%' LIMIT 5;",
                        qi(&table.name),
                        qi(&col.name),
                        sanitized_keyword
                    );
                    if let Ok(rows) = sqlx::query(&search_sql).fetch_all(&self.pool).await {
                        if !rows.is_empty() {
                            let mut samples = Vec::new();
                            for r in rows.iter().take(3) {
                                samples.push(serde_json::Value::Object(Self::row_to_map(r)));
                            }
                            results.push(GlobalSearchResult {
                                table_name: table.name.clone(),
                                column_name: col.name,
                                match_count: rows.len(),
                                sample_rows: samples,
                            });
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    async fn export_table(
        &self,
        table: &str,
        format: &str,
        filter: Option<&str>,
        dest_dir: &std::path::Path,
    ) -> AppResult<ExportResult> {
        let mut query = format!("SELECT * FROM {}", qi(table));
        if let Some(f) = filter {
            if !f.trim().is_empty() {
                query.push_str(&format!(" WHERE {}", f));
            }
        }
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .db_with(|e| format!("Export query failed: {}", e))?;

        // Column headers: from result rows, falling back to table schema for empty sets.
        let headers: Vec<String> = if let Some(first) = rows.first() {
            first
                .columns()
                .iter()
                .map(|c| c.name().to_string())
                .collect()
        } else {
            self.get_table_schema(table)
                .await
                .unwrap_or_default()
                .iter()
                .map(|c| c.name.clone())
                .collect()
        };

        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let safe_table: String = table
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        let (ext, bytes): (&str, Vec<u8>) = match format {
            "csv" => {
                let mut wtr = csv::Writer::from_writer(Vec::new());
                wtr.write_record(&headers).map_err(AppError::from)?;
                for r in &rows {
                    let vals: Vec<String> = Self::row_to_json(r)
                        .iter()
                        .map(|v| match v {
                            serde_json::Value::Null => String::new(),
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect();
                    wtr.write_record(&vals).map_err(AppError::from)?;
                }
                ("csv", wtr.into_inner().map_err(AppError::from)?)
            }
            "json" => {
                let arr: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|r| serde_json::Value::Object(Self::row_to_map(r)))
                    .collect();
                (
                    "json",
                    serde_json::to_string_pretty(&arr)
                        .map_err(AppError::from)?
                        .into_bytes(),
                )
            }
            "sql" => {
                let mut out = String::new();
                if let Ok(ddl) = self.get_table_ddl(table).await {
                    out.push_str(&format!("{};\n\n", ddl.trim_end_matches(';')));
                }
                for r in &rows {
                    let map = Self::row_to_map(r);
                    let cols: Vec<String> = map.keys().map(|k| qi(k)).collect();
                    let vals: Vec<String> = map.values().map(Self::sql_literal).collect();
                    out.push_str(&format!(
                        "INSERT INTO {} ({}) VALUES ({});\n",
                        qi(table),
                        cols.join(", "),
                        vals.join(", ")
                    ));
                }
                ("sql", out.into_bytes())
            }
            "xlsx" => {
                let mut wb = rust_xlsxwriter::Workbook::new();
                // Sheet name: max 31 chars, no special chars.
                let sheet_name: String = safe_table.chars().take(31).collect();
                let ws = wb
                    .add_worksheet()
                    .set_name(&sheet_name)
                    .map_err(AppError::from)?;

                let bold = rust_xlsxwriter::Format::new().set_bold();
                for (c, h) in headers.iter().enumerate() {
                    ws.write_string_with_format(0, c as u16, h, &bold)
                        .map_err(AppError::from)?;
                }
                for (ri, r) in rows.iter().enumerate() {
                    let vals = Self::row_to_json(r);
                    for (ci, v) in vals.iter().enumerate() {
                        let (row32, col16) = ((ri + 1) as u32, ci as u16);
                        match v {
                            serde_json::Value::Null => {}
                            serde_json::Value::Bool(b) => {
                                ws.write_boolean(row32, col16, *b).map_err(AppError::from)?;
                            }
                            serde_json::Value::Number(n) => {
                                ws.write_number(row32, col16, n.as_f64().unwrap_or(0.0))
                                    .map_err(AppError::from)?;
                            }
                            other => {
                                let s = match other {
                                    serde_json::Value::String(s) => s.clone(),
                                    o => o.to_string(),
                                };
                                ws.write_string(row32, col16, &s).map_err(AppError::from)?;
                            }
                        }
                    }
                }
                // Auto-width approximation so the file opens readable.
                ws.autofit();
                ("xlsx", wb.save_to_buffer().map_err(AppError::from)?)
            }
            other => {
                return Err(AppError::unsupported(format!(
                    "Unsupported export format: {}",
                    other
                )))
            }
        };

        let mut path = dest_dir.to_path_buf();
        path.push(format!("{}-{}.{}", safe_table, ts, ext));
        std::fs::write(&path, &bytes)
            .map_err(|e| AppError::storage_at(&path, format!("Failed writing export: {}", e)))?;

        Ok(ExportResult {
            file_path: path.to_string_lossy().to_string(),
            row_count: rows.len() as u64,
            byte_size: bytes.len() as u64,
            format: format.to_string(),
        })
    }

    async fn import_csv(&self, table: &str, file_path: &str) -> AppResult<u64> {
        let mut rdr = csv::Reader::from_path(file_path).map_err(|e| {
            AppError::storage_at(
                std::path::Path::new(file_path),
                format!("Cannot open CSV: {}", e),
            )
        })?;

        let headers: Vec<String> = rdr
            .headers()
            .map_err(AppError::from)?
            .iter()
            .map(|h| h.trim().to_string())
            .collect();

        if headers.is_empty() {
            return Err(AppError::validation("CSV file has no header row"));
        }

        // Validate header names: identifiers only, to keep generated DDL safe.
        for h in &headers {
            if h.is_empty() || !h.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(AppError::validation(format!(
                    "Invalid CSV header '{}'. Headers must be alphanumeric/underscore.",
                    h
                )));
            }
        }

        // Create the table when missing (all TEXT columns; SQLite affinity handles the rest).
        let col_defs: Vec<String> = headers.iter().map(|h| format!("{} TEXT", qi(h))).collect();
        let create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {} ({});",
            qi(table),
            col_defs.join(", ")
        );
        sqlx::query(&create_sql)
            .execute(&self.pool)
            .await
            .db_with(|e| format!("Failed creating table: {}", e))?;

        let placeholders: Vec<String> = headers.iter().map(|_| "?".to_string()).collect();
        let insert_sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            qi(table),
            headers.iter().map(|h| qi(h)).collect::<Vec<_>>().join(", "),
            placeholders.join(", ")
        );

        let mut tx = self
            .pool
            .begin()
            .await
            .db_with(|e| format!("Transaction begin failed: {}", e))?;

        let mut count: u64 = 0;
        for record in rdr.records() {
            let rec = record.db_with(|e| format!("CSV parse error: {}", e))?;
            let mut q = sqlx::query(&insert_sql);
            for i in 0..headers.len() {
                let v = rec.get(i).unwrap_or("");
                if v.is_empty() {
                    q = q.bind(None::<String>);
                } else {
                    q = q.bind(v.to_string());
                }
            }
            q.execute(&mut *tx)
                .await
                .db_with(|e| format!("Import failed at row {}: {}", count + 1, e))?;
            count += 1;
        }

        tx.commit()
            .await
            .db_with(|e| format!("Transaction commit failed: {}", e))?;

        Ok(count)
    }

    async fn backup_database(&self, dest_dir: &std::path::Path) -> AppResult<String> {
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let base: String = std::path::Path::new(&self.db_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .filter(|s| {
                !s.is_empty()
                    && s.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            })
            .unwrap_or_else(|| "backup".to_string());

        // Binary snapshot via VACUUM INTO for file-backed databases.
        // (Shared-cache :memory: pools cannot VACUUM INTO — those fall back to a
        // logical .sql dump below.)
        if self.db_path != ":memory:" {
            let mut path = dest_dir.to_path_buf();
            path.push(format!("{}-{}.db", base, ts));
            let vacuum_sql = format!(
                "VACUUM INTO '{}'",
                path.to_string_lossy().replace('\'', "''")
            );
            if sqlx::query(&vacuum_sql).execute(&self.pool).await.is_ok() && path.exists() {
                return Ok(path.to_string_lossy().to_string());
            }
        }

        // Logical dump fallback: schema DDL + INSERTs for every table, then
        // views/triggers/indexes — portable and works for :memory: too.
        self.dump_database(dest_dir, &base, &ts).await
    }

    async fn server_version(&self) -> AppResult<String> {
        let row = sqlx::query("SELECT sqlite_version() AS v;")
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;
        let v: String = row.try_get("v").unwrap_or_default();
        Ok(format!("SQLite {}", v))
    }

    async fn get_database_stats(&self) -> AppResult<DatabaseStats> {
        async fn pragma_i64(pool: &SqlitePool, name: &str) -> i64 {
            sqlx::query(&format!("PRAGMA {};", name))
                .fetch_one(pool)
                .await
                .and_then(|r| r.try_get::<i64, _>(0))
                .unwrap_or(0)
        }
        async fn pragma_str(pool: &SqlitePool, name: &str) -> String {
            sqlx::query(&format!("PRAGMA {};", name))
                .fetch_one(pool)
                .await
                .and_then(|r| r.try_get::<String, _>(0))
                .unwrap_or_default()
        }

        let page_count = pragma_i64(&self.pool, "page_count").await.max(0) as u64;
        let page_size = pragma_i64(&self.pool, "page_size").await.max(0) as u64;
        let freelist_pages = pragma_i64(&self.pool, "freelist_count").await.max(0) as u64;
        let journal_mode = pragma_str(&self.pool, "journal_mode").await;
        let encoding = pragma_str(&self.pool, "encoding").await;

        let version_row = sqlx::query("SELECT sqlite_version() AS v;")
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;
        let ver: String = version_row.try_get("v").unwrap_or_default();

        // Object counts by type
        let cnt_rows = sqlx::query(
            "SELECT type, COUNT(*) AS n FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' GROUP BY type;",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        let mut table_count = 0u64;
        let mut view_count = 0u64;
        let mut index_count = 0u64;
        let mut trigger_count = 0u64;
        for r in &cnt_rows {
            let t: String = r.try_get("type").unwrap_or_default();
            let n: i64 = r.try_get("n").unwrap_or(0);
            match t.as_str() {
                "table" => table_count = n as u64,
                "view" => view_count = n as u64,
                "index" => index_count = n as u64,
                "trigger" => trigger_count = n as u64,
                _ => {}
            }
        }

        // Total rows across user tables
        let mut total_rows = 0u64;
        let tables = self.list_tables().await.unwrap_or_default();
        for t in &tables {
            if t.table_type == "table" {
                total_rows += t.row_count_estimate.unwrap_or(0).max(0) as u64;
            }
        }

        // File size: real file, or pages*page_size for :memory:
        let file_path = if self.db_path == ":memory:" {
            None
        } else {
            Some(self.db_path.clone())
        };
        let file_size_bytes = file_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(page_count * page_size);

        Ok(DatabaseStats {
            driver_name: "SQLite".to_string(),
            version: format!("SQLite {}", ver),
            file_path,
            file_size_bytes,
            page_count,
            page_size,
            freelist_pages,
            journal_mode,
            encoding,
            table_count,
            view_count,
            index_count,
            trigger_count,
            total_rows,
        })
    }

    /// SQLite has no `ALTER TABLE ADD CONSTRAINT` — rebuild the table:
    /// recreate it under a temp name with the FK clause appended, copy rows,
    /// swap names, and restore indexes/triggers. Everything runs in a single
    /// transaction with `foreign_keys` disabled and a `foreign_key_check`
    /// gate before COMMIT, so existing data can never be lost silently.
    async fn add_foreign_key(&self, spec: &ForeignKeySpec) -> AppResult<String> {
        use super::common::sanitize_fk_action;
        use sqlx::Acquire;

        let table = spec.table.as_str();
        let fk_clause = format!(
            "CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE {} ON UPDATE {}",
            qi(&spec.constraint_name),
            qi(&spec.column),
            qi(&spec.ref_table),
            qi(&spec.ref_column),
            sanitize_fk_action(&spec.on_delete),
            sanitize_fk_action(&spec.on_update),
        );

        // Original CREATE TABLE statement — we inject the constraint into it.
        let ddl_row =
            sqlx::query("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?;")
                .bind(table)
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::from)?;
        let ddl: String = ddl_row
            .and_then(|r| r.try_get::<Option<String>, _>("sql").ok().flatten())
            .ok_or_else(|| AppError::not_found(format!("Table '{}' not found", table)))?;

        let tmp_name = format!("{}__fk_rebuild", table);
        let new_ddl = inject_constraint(&retarget_create_table(&ddl, &tmp_name)?, &fk_clause)?;

        // Column list for the explicit INSERT ... SELECT (order-safe).
        let cols = self.get_table_schema(table).await?;
        let col_list = cols
            .iter()
            .map(|c| qi(&c.name))
            .collect::<Vec<_>>()
            .join(", ");

        // Indexes/triggers defined on the table — they die with DROP TABLE.
        let extras: Vec<String> = sqlx::query(
            "SELECT sql FROM sqlite_master WHERE tbl_name = ? AND type IN ('index','trigger') AND sql IS NOT NULL;",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?
        .iter()
        .map(|r| r.try_get::<String, _>("sql").unwrap_or_default())
        .filter(|s| !s.trim().is_empty())
        .collect();

        // Single connection: PRAGMA foreign_keys is per-connection, and the
        // whole rebuild must be one transaction on the same connection.
        let mut conn = self.pool.acquire().await.map_err(AppError::from)?;
        sqlx::query("PRAGMA foreign_keys = OFF;")
            .execute(&mut *conn)
            .await
            .map_err(AppError::from)?;
        sqlx::query(&format!("DROP TABLE IF EXISTS {};", qi(&tmp_name)))
            .execute(&mut *conn)
            .await
            .map_err(AppError::from)?;

        let mut tx = conn.begin().await.map_err(AppError::from)?;

        let outcome: AppResult<String> = async {
            sqlx::query(&new_ddl)
                .execute(&mut *tx)
                .await
                .map_err(AppError::from)?;
            sqlx::query(&format!(
                "INSERT INTO {} ({}) SELECT {} FROM {};",
                qi(&tmp_name),
                col_list,
                col_list,
                qi(table)
            ))
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
            sqlx::query(&format!("DROP TABLE {};", qi(table)))
                .execute(&mut *tx)
                .await
                .map_err(AppError::from)?;
            sqlx::query(&format!(
                "ALTER TABLE {} RENAME TO {};",
                qi(&tmp_name),
                qi(table)
            ))
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
            for extra in &extras {
                sqlx::query(extra)
                    .execute(&mut *tx)
                    .await
                    .map_err(AppError::from)?;
            }

            // Gate: refuse to commit if existing rows violate the new FK.
            let bad = sqlx::query(&format!("PRAGMA foreign_key_check({});", qi(table)))
                .fetch_all(&mut *tx)
                .await
                .map_err(AppError::from)?;
            if !bad.is_empty() {
                return Err(AppError::database(format!(
                    "Cannot add foreign key: {} existing row(s) in '{}' violate it",
                    bad.len(),
                    table
                )));
            }
            Ok(format!(
                "Rebuilt table \"{}\" with FK {} → {}.{} ({} index/trigger object(s) restored)",
                table,
                spec.column,
                spec.ref_table,
                spec.ref_column,
                extras.len()
            ))
        }
        .await;

        match outcome {
            Ok(msg) => {
                tx.commit().await.map_err(AppError::from)?;
                // best effort: re-enable enforcement on this connection
                let _ = sqlx::query("PRAGMA foreign_keys = ON;")
                    .execute(&mut *conn)
                    .await;
                Ok(msg)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                let _ = sqlx::query("PRAGMA foreign_keys = ON;")
                    .execute(&mut *conn)
                    .await;
                Err(e)
            }
        }
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}

/// Rewrite `CREATE TABLE x (...)` to use `new_name`, preserving the body.
/// Drops any `IF NOT EXISTS` / schema qualifier in the name position.
fn retarget_create_table(ddl: &str, new_name: &str) -> AppResult<String> {
    let open = ddl
        .find('(')
        .ok_or_else(|| AppError::database("Unparseable CREATE TABLE: no column list"))?;
    let head = ddl[..open].to_uppercase();
    let prefix = if head.contains("IF NOT EXISTS") {
        "CREATE TABLE IF NOT EXISTS"
    } else {
        "CREATE TABLE"
    };
    Ok(format!("{} {}{}", prefix, qi(new_name), &ddl[open..]))
}

/// Insert a table-level constraint just before the matching `)` of the
/// column list (paren-depth aware, so defaults like `(0)` don't confuse it).
fn inject_constraint(ddl: &str, clause: &str) -> AppResult<String> {
    let start = ddl
        .find('(')
        .ok_or_else(|| AppError::database("Unparseable CREATE TABLE: no column list"))?;
    let mut depth = 0i32;
    let mut end = None;
    for (i, ch) in ddl.char_indices().skip(start) {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end =
        end.ok_or_else(|| AppError::database("Unparseable CREATE TABLE: unbalanced parens"))?;
    Ok(format!(
        "{},\n  {}{}",
        ddl[..end].trim_end_matches(|c: char| c == ',' || c.is_whitespace()),
        clause,
        &ddl[end..]
    ))
}

#[cfg(test)]
mod tests;
