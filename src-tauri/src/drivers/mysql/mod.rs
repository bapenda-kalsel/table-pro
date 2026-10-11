use super::common::{self, Dialect, ParamStyle};
use super::DatabaseDriver;
use crate::models::{
    DatabaseStats, ExplainResult, ExplainStep, ExportResult, ForeignKeyInfo, FullTableSchema,
    GlobalSearchResult, IndexInfo, QueryResultData, StagedCommitPayload, StagedDiffSummary,
    TableColumnMetadata, TableInfo, TriggerInfo,
};
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions, MySqlRow};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Verbs whose statements return rows (everything else -> affected-rows result).
const ROW_RETURNING_VERBS: &[&str] = &[
    "select", "show", "explain", "describe", "desc", "with", "values", "table",
];

pub struct MysqlDriver {
    pool: MySqlPool,
    database: String,
    host: String,
    port: u16,
}

fn qi(ident: &str) -> String {
    Dialect::Backtick.qi(ident)
}

/// Quote a possibly schema-qualified name (`db`.`table`).
fn qi_path(name: &str) -> String {
    Dialect::Backtick.qi_path(name)
}

fn to_hex(b: &[u8]) -> String {
    const MAX: usize = 256;
    let hex: String = b.iter().take(MAX).map(|x| format!("{:02x}", x)).collect();
    if b.len() > MAX {
        format!("0x{}…({} bytes)", hex, b.len())
    } else {
        format!("0x{}", hex)
    }
}

impl MysqlDriver {
    pub async fn new(
        host: &str,
        port: u16,
        database: &str,
        username: &str,
        password: &str,
    ) -> Result<Self, String> {
        let opts = MySqlConnectOptions::new()
            .host(host)
            .port(port)
            .username(username)
            .password(password)
            .database(database);

        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(opts)
            .await
            .map_err(|e| format!("MySQL connection failed: {}", e))?;

        Ok(Self {
            pool,
            database: database.to_string(),
            host: host.to_string(),
            port,
        })
    }

    fn value_to_json(row: &MySqlRow, i: usize) -> Value {
        let raw = match row.try_get_raw(i) {
            Ok(v) => v,
            Err(_) => return Value::Null,
        };
        if raw.is_null() {
            return Value::Null;
        }

        let type_name = row
            .columns()
            .get(i)
            .map(|c| c.type_info().name().to_uppercase())
            .unwrap_or_default();

        match type_name.as_str() {
            t if t.contains("UNSIGNED") && t.contains("INT") => row
                .try_get::<u64, _>(i)
                .map(Value::from)
                .or_else(|_| row.try_get::<i64, _>(i).map(Value::from))
                .unwrap_or(Value::Null),
            "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "INTEGER" | "BIGINT" | "YEAR"
            | "BIT" | "BOOL" | "BOOLEAN" => row
                .try_get::<i64, _>(i)
                .map(Value::from)
                .unwrap_or(Value::Null),
            "FLOAT" | "DOUBLE" | "DOUBLE PRECISION" | "REAL" => row
                .try_get::<f64, _>(i)
                .map(|v| json!(v))
                .unwrap_or(Value::Null),
            "DECIMAL" | "NEWDECIMAL" | "NUMERIC" => row
                .try_get::<rust_decimal::Decimal, _>(i)
                .map(|d| json!(d.to_string()))
                .or_else(|_| row.try_get::<f64, _>(i).map(|f| json!(f)))
                .unwrap_or(Value::Null),
            "DATE" => row
                .try_get::<chrono::NaiveDate, _>(i)
                .map(|v| json!(v.to_string()))
                .unwrap_or(Value::Null),
            "TIME" => row
                .try_get::<chrono::NaiveTime, _>(i)
                .map(|v| json!(v.to_string()))
                .unwrap_or(Value::Null),
            "DATETIME" | "TIMESTAMP" => row
                .try_get::<chrono::NaiveDateTime, _>(i)
                .map(|v| json!(v.to_string()))
                .unwrap_or(Value::Null),
            "JSON" => row
                .try_get::<serde_json::Value, _>(i)
                .unwrap_or(Value::Null),
            "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BINARY" | "VARBINARY"
            | "GEOMETRY" => row
                .try_get::<Vec<u8>, _>(i)
                .map(|b| json!(to_hex(&b)))
                .unwrap_or(Value::Null),
            _ => row
                .try_get::<String, _>(i)
                .map(Value::from)
                .unwrap_or(Value::Null),
        }
    }

    fn row_to_json(row: &MySqlRow) -> Vec<Value> {
        (0..row.columns().len())
            .map(|i| Self::value_to_json(row, i))
            .collect()
    }

    fn row_to_map(row: &MySqlRow) -> Map<String, Value> {
        let mut m = Map::new();
        for (i, c) in row.columns().iter().enumerate() {
            m.insert(c.name().to_string(), Self::value_to_json(row, i));
        }
        m
    }

    fn col_types(row: &MySqlRow) -> Vec<String> {
        row.columns()
            .iter()
            .map(|c| c.type_info().name().to_uppercase())
            .collect()
    }
}

#[async_trait]
impl DatabaseDriver for MysqlDriver {
    async fn ping(&self) -> Result<f64, String> {
        let start = Instant::now();
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    }

    async fn list_tables(&self) -> Result<Vec<TableInfo>, String> {
        let rows = sqlx::query(
            "SELECT TABLE_NAME, TABLE_TYPE, TABLE_ROWS
             FROM information_schema.TABLES
             WHERE TABLE_SCHEMA = DATABASE()
             ORDER BY TABLE_NAME;",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(rows
            .iter()
            .map(|r| {
                let t: String = r.try_get("TABLE_TYPE").unwrap_or_default();
                TableInfo {
                    name: r.try_get("TABLE_NAME").unwrap_or_default(),
                    table_type: if t == "BASE TABLE" { "table" } else { "view" }.to_string(),
                    row_count_estimate: r
                        .try_get::<i64, _>("TABLE_ROWS")
                        .ok()
                        .filter(|n| *n >= 0),
                }
            })
            .collect())
    }

    async fn get_table_schema(&self, table: &str) -> Result<Vec<TableColumnMetadata>, String> {
        let rows = sqlx::query(
            "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_KEY, COLUMN_DEFAULT, COLUMN_COMMENT
             FROM information_schema.COLUMNS
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ?
             ORDER BY ORDINAL_POSITION;",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(rows
            .iter()
            .map(|r| TableColumnMetadata {
                name: r.try_get("COLUMN_NAME").unwrap_or_default(),
                data_type: r.try_get("COLUMN_TYPE").unwrap_or_default(),
                is_nullable: r
                    .try_get::<String, _>("IS_NULLABLE")
                    .map(|v| v == "YES")
                    .unwrap_or(true),
                is_primary_key: r
                    .try_get::<String, _>("COLUMN_KEY")
                    .map(|v| v == "PRI")
                    .unwrap_or(false),
                default_value: r.try_get("COLUMN_DEFAULT").ok().flatten(),
                comment: r
                    .try_get::<String, _>("COLUMN_COMMENT")
                    .ok()
                    .filter(|s| !s.is_empty()),
            })
            .collect())
    }

    async fn get_full_schema(&self, table: &str) -> Result<FullTableSchema, String> {
        let columns = self.get_table_schema(table).await?;

        // Indexes
        let idx_rows = sqlx::query(
            "SELECT INDEX_NAME, NON_UNIQUE, COLUMN_NAME
             FROM information_schema.STATISTICS
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ?
             ORDER BY INDEX_NAME, SEQ_IN_INDEX;",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut idx_map: BTreeMap<String, IndexInfo> = BTreeMap::new();
        for r in &idx_rows {
            let name: String = r.try_get("INDEX_NAME").unwrap_or_default();
            let non_unique: i64 = r.try_get("NON_UNIQUE").unwrap_or(1);
            let col: String = r.try_get("COLUMN_NAME").unwrap_or_default();
            let entry = idx_map.entry(name.clone()).or_insert_with(|| IndexInfo {
                name: name.clone(),
                columns: vec![],
                is_unique: non_unique == 0,
                origin: if name == "PRIMARY" {
                    "pk".to_string()
                } else if non_unique == 0 {
                    "u".to_string()
                } else {
                    "c".to_string()
                },
                is_partial: false,
            });
            entry.columns.push(col);
        }

        // Foreign keys
        let fk_rows = sqlx::query(
            "SELECT k.CONSTRAINT_NAME, k.COLUMN_NAME, k.REFERENCED_TABLE_NAME,
                    k.REFERENCED_COLUMN_NAME, rc.UPDATE_RULE, rc.DELETE_RULE
             FROM information_schema.KEY_COLUMN_USAGE k
             JOIN information_schema.REFERENTIAL_CONSTRAINTS rc
               ON rc.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA
              AND rc.CONSTRAINT_NAME = k.CONSTRAINT_NAME
             WHERE k.TABLE_SCHEMA = DATABASE() AND k.TABLE_NAME = ?
               AND k.REFERENCED_TABLE_NAME IS NOT NULL;",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let foreign_keys = fk_rows
            .iter()
            .enumerate()
            .map(|(i, r)| ForeignKeyInfo {
                id: i as i64,
                from_column: r.try_get("COLUMN_NAME").unwrap_or_default(),
                referenced_table: r.try_get("REFERENCED_TABLE_NAME").unwrap_or_default(),
                to_column: r.try_get("REFERENCED_COLUMN_NAME").ok(),
                on_update: r
                    .try_get::<String, _>("UPDATE_RULE")
                    .unwrap_or_else(|_| "RESTRICT".to_string()),
                on_delete: r
                    .try_get::<String, _>("DELETE_RULE")
                    .unwrap_or_else(|_| "RESTRICT".to_string()),
            })
            .collect();

        // Triggers
        let trg_rows = sqlx::query(
            "SELECT TRIGGER_NAME, EVENT_MANIPULATION, ACTION_TIMING, ACTION_STATEMENT
             FROM information_schema.TRIGGERS
             WHERE TRIGGER_SCHEMA = DATABASE() AND EVENT_OBJECT_TABLE = ?;",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let triggers = trg_rows
            .iter()
            .map(|r| {
                let name: String = r.try_get("TRIGGER_NAME").unwrap_or_default();
                let event: String = r.try_get("EVENT_MANIPULATION").unwrap_or_default();
                let timing: String = r.try_get("ACTION_TIMING").unwrap_or_default();
                let body: String = r.try_get("ACTION_STATEMENT").unwrap_or_default();
                TriggerInfo {
                    sql: format!(
                        "CREATE TRIGGER {} {} {} ON {} FOR EACH ROW {}",
                        name,
                        timing,
                        event,
                        qi(table),
                        body
                    ),
                    name,
                    table_name: table.to_string(),
                }
            })
            .collect();

        let ddl = self.get_table_ddl(table).await.ok();

        Ok(FullTableSchema {
            columns,
            indexes: idx_map.into_values().collect(),
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
    ) -> Result<QueryResultData, String> {
        let start = Instant::now();
        let data_sql = common::table_data_query(
            Dialect::Backtick,
            table,
            limit,
            offset,
            filter,
            sort_by,
            sort_dir,
        );
        let count_sql = common::count_query(Dialect::Backtick, table, filter);

        let total_rows: i64 = sqlx::query(&count_sql)
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get("n"))
            .unwrap_or(0);

        let rows = sqlx::query(&data_sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let (columns, column_types) = match rows.first() {
            Some(r) => (
                r.columns().iter().map(|c| c.name().to_string()).collect(),
                Self::col_types(r),
            ),
            None => {
                let schema = self.get_table_schema(table).await.unwrap_or_default();
                (
                    schema.iter().map(|c| c.name.clone()).collect(),
                    schema.iter().map(|c| c.data_type.clone()).collect(),
                )
            }
        };

        Ok(QueryResultData {
            columns,
            column_types,
            rows: rows.iter().map(Self::row_to_json).collect(),
            execution_time_ms: start.elapsed().as_secs_f64() * 1000.0,
            affected_rows: 0,
            total_rows: total_rows.max(0) as u64,
        })
    }

    async fn execute_query(&self, sql: &str) -> Result<QueryResultData, String> {
        let start = Instant::now();
        let verb = crate::models::detect_statement_type(sql).to_lowercase();

        if ROW_RETURNING_VERBS.contains(&verb.as_str()) {
            let rows = sqlx::query(sql)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
            let (columns, column_types) = match rows.first() {
                Some(r) => (
                    r.columns().iter().map(|c| c.name().to_string()).collect(),
                    Self::col_types(r),
                ),
                None => (vec![], vec![]),
            };
            Ok(QueryResultData {
                columns,
                column_types,
                total_rows: rows.len() as u64,
                rows: rows.iter().map(Self::row_to_json).collect(),
                execution_time_ms: start.elapsed().as_secs_f64() * 1000.0,
                affected_rows: 0,
            })
        } else {
            let res = sqlx::query(sql)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
            Ok(QueryResultData {
                columns: vec!["Affected Rows".to_string()],
                column_types: vec!["INTEGER".to_string()],
                rows: vec![vec![json!(res.rows_affected())]],
                execution_time_ms: start.elapsed().as_secs_f64() * 1000.0,
                affected_rows: res.rows_affected(),
                total_rows: 1,
            })
        }
    }

    async fn explain_query(&self, sql: &str) -> Result<ExplainResult, String> {
        let explain_sql = format!("EXPLAIN {}", sql.trim_end_matches(';'));
        let rows = sqlx::query(&explain_sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let mut steps = Vec::new();
        let mut has_full_scan = false;
        let mut uses_index = false;
        let mut temp_btree = false;
        let mut warnings = Vec::new();

        for (i, r) in rows.iter().enumerate() {
            let m = Self::row_to_map(r);
            let get = |k: &str| -> String {
                m.get(k)
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        Value::Null => String::new(),
                        o => o.to_string(),
                    })
                    .unwrap_or_default()
            };
            let table = get("table");
            let access = get("type");
            let key = get("key");
            let est_rows = get("rows");
            let extra = get("Extra");

            if access.eq_ignore_ascii_case("ALL") {
                has_full_scan = true;
                warnings.push(format!("Full table scan on '{}'", table));
            }
            if !key.is_empty() && key != "null" {
                uses_index = true;
            }
            if extra.contains("Using filesort") || extra.contains("Using temporary") {
                temp_btree = true;
            }

            steps.push(ExplainStep {
                id: i as i64,
                parent: 0,
                detail: format!(
                    "{} {} access={} key={} rows≈{} {}",
                    get("select_type"),
                    table,
                    access,
                    if key.is_empty() { "-" } else { &key },
                    est_rows,
                    extra
                ),
                children: vec![],
            });
        }

        if temp_btree {
            warnings.push("Uses temporary table / filesort — consider an index to support ORDER BY/GROUP BY".to_string());
        }

        Ok(ExplainResult {
            steps,
            has_full_scan,
            uses_index,
            uses_temp_btree: temp_btree,
            warnings,
        })
    }

    async fn preview_staged_changes(
        &self,
        payload: &StagedCommitPayload,
    ) -> Result<StagedDiffSummary, String> {
        Ok(common::preview_staged_sql(Dialect::Backtick, payload))
    }

    async fn commit_staged_changes(
        &self,
        payload: StagedCommitPayload,
    ) -> Result<QueryResultData, String> {
        let diff = common::preview_staged_sql(Dialect::Backtick, &payload);
        let start = Instant::now();

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Transaction begin failed: {}", e))?;

        let mut total_affected = 0;
        for sql in &diff.generated_sql {
            let res = sqlx::query(sql)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed executing '{}': {}", sql, e))?;
            total_affected += res.rows_affected();
        }

        tx.commit()
            .await
            .map_err(|e| format!("Transaction commit failed: {}", e))?;

        Ok(QueryResultData {
            columns: vec!["Committed Statements".to_string(), "Total Affected".to_string()],
            column_types: vec!["INTEGER".to_string(), "INTEGER".to_string()],
            rows: vec![vec![
                json!(diff.generated_sql.len()),
                json!(total_affected),
            ]],
            execution_time_ms: start.elapsed().as_secs_f64() * 1000.0,
            affected_rows: total_affected,
            total_rows: 1,
        })
    }

    async fn get_table_ddl(&self, table: &str) -> Result<String, String> {
        let row = sqlx::query(&format!("SHOW CREATE TABLE {}", qi_path(table)))
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        // Column 1 is "Create Table"/"Create View"
        row.try_get::<String, _>(1)
            .map_err(|e| format!("No DDL for '{}': {}", table, e))
    }

    async fn run_maintenance(&self, action: &str, table: Option<&str>) -> Result<String, String> {
        // Resolve target list: given table or every base table in this schema.
        let targets: Vec<String> = match table {
            Some(t) => vec![t.to_string()],
            None => self
                .list_tables()
                .await?
                .iter()
                .filter(|t| t.table_type == "table")
                .map(|t| t.name.clone())
                .collect(),
        };
        if targets.is_empty() {
            return Err("No tables to operate on".to_string());
        }
        let joined = targets.iter().map(|t| qi(t)).collect::<Vec<_>>().join(", ");

        match action.to_lowercase().as_str() {
            "vacuum" => {
                // MySQL equivalent of VACUUM: OPTIMIZE TABLE rebuilds + frees space.
                sqlx::query(&format!("OPTIMIZE TABLE {}", joined))
                    .execute(&self.pool)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("OPTIMIZE TABLE completed on {} table(s).", targets.len()))
            }
            "integrity_check" => {
                let rows = sqlx::query(&format!("CHECK TABLE {}", joined))
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut msgs = Vec::new();
                for r in &rows {
                    let m = Self::row_to_map(r);
                    let t = m.get("Table").and_then(|v| v.as_str()).unwrap_or("?");
                    let txt = m.get("Msg_text").and_then(|v| v.as_str()).unwrap_or("");
                    msgs.push(format!("{}: {}", t, txt));
                }
                Ok(format!("CHECK TABLE result:\n{}", msgs.join("\n")))
            }
            "analyze" => {
                sqlx::query(&format!("ANALYZE TABLE {}", joined))
                    .execute(&self.pool)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("ANALYZE TABLE completed on {} table(s).", targets.len()))
            }
            "reindex" => {
                sqlx::query(&format!("OPTIMIZE TABLE {}", joined))
                    .execute(&self.pool)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("Tables rebuilt (index reorganization) on {} table(s).", targets.len()))
            }
            other => Err(format!("Unsupported maintenance action '{}'", other)),
        }
    }

    async fn search_text_in_database(&self, keyword: &str) -> Result<Vec<GlobalSearchResult>, String> {
        let tables = self.list_tables().await?;
        let mut results = Vec::new();

        for t in tables.iter().filter(|t| t.table_type == "table") {
            let cols = self.get_table_schema(&t.name).await.unwrap_or_default();
            let text_cols: Vec<&TableColumnMetadata> = cols
                .iter()
                .filter(|c| {
                    let d = c.data_type.to_lowercase();
                    d.contains("char") || d.contains("text") || d.contains("enum") || d.contains("json")
                })
                .collect();

            for c in text_cols {
                let sql = format!(
                    "SELECT * FROM {} WHERE {} LIKE CONCAT('%', ?, '%') LIMIT 5;",
                    qi(&t.name),
                    qi(&c.name),
                );
                if let Ok(rows) = sqlx::query(&sql).bind(keyword).fetch_all(&self.pool).await {
                    if !rows.is_empty() {
                        let samples: Vec<Value> = rows
                            .iter()
                            .take(3)
                            .map(|r| Value::Object(Self::row_to_map(r)))
                            .collect();
                        results.push(GlobalSearchResult {
                            table_name: t.name.clone(),
                            column_name: c.name.clone(),
                            match_count: rows.len(),
                            sample_rows: samples,
                        });
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
    ) -> Result<ExportResult, String> {
        let mut query = format!("SELECT * FROM {}", qi_path(table));
        if let Some(f) = filter {
            if !f.trim().is_empty() {
                query.push_str(&format!(" WHERE {}", f));
            }
        }
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Export query failed: {}", e))?;

        let headers: Vec<String> = if let Some(first) = rows.first() {
            first.columns().iter().map(|c| c.name().to_string()).collect()
        } else {
            self.get_table_schema(table)
                .await
                .unwrap_or_default()
                .iter()
                .map(|c| c.name.clone())
                .collect()
        };

        let maps: Vec<Map<String, Value>> = rows.iter().map(Self::row_to_map).collect();
        let ddl = self.get_table_ddl(table).await.ok();

        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let safe_table: String = table
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();

        let bytes = common::export_rows_bytes(
            format,
            table,
            ddl.as_deref(),
            &headers,
            &maps,
            Dialect::Backtick,
        )?;

        let mut path = dest_dir.to_path_buf();
        path.push(format!(
            "{}-{}.{}",
            safe_table,
            ts,
            match format {
                "csv" => "csv",
                "json" => "json",
                "sql" => "sql",
                "xlsx" => "xlsx",
                _ => "txt",
            }
        ));
        std::fs::write(&path, &bytes).map_err(|e| format!("Failed writing export: {}", e))?;

        Ok(ExportResult {
            file_path: path.to_string_lossy().to_string(),
            row_count: rows.len() as u64,
            byte_size: bytes.len() as u64,
            format: format.to_string(),
        })
    }

    async fn import_csv(&self, table: &str, file_path: &str) -> Result<u64, String> {
        let mut rdr = csv::Reader::from_path(file_path)
            .map_err(|e| format!("Cannot open CSV '{}': {}", file_path, e))?;
        let headers: Vec<String> = rdr
            .headers()
            .map_err(|e| e.to_string())?
            .iter()
            .map(|h| h.trim().to_string())
            .collect();

        let create_sql = common::import_table_ddl(Dialect::Backtick, table, &headers)?;
        sqlx::query(&create_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed creating table: {}", e))?;

        let insert_sql =
            common::import_insert_sql(Dialect::Backtick, ParamStyle::Question, table, &headers);

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Transaction begin failed: {}", e))?;

        let mut count: u64 = 0;
        for record in rdr.records() {
            let rec = record.map_err(|e| format!("CSV parse error: {}", e))?;
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
                .map_err(|e| format!("Import failed at row {}: {}", count + 1, e))?;
            count += 1;
        }

        tx.commit()
            .await
            .map_err(|e| format!("Transaction commit failed: {}", e))?;
        Ok(count)
    }

    async fn backup_database(&self, dest_dir: &std::path::Path) -> Result<String, String> {
        // MySQL has no in-SQL file backup: produce a logical .sql dump.
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let base = format!("{}-{}-{}", self.database, self.host, self.port)
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>();

        let mut table_dumps = Vec::new();
        let mut extras = Vec::new();
        for t in self.list_tables().await? {
            let ddl = self.get_table_ddl(&t.name).await.unwrap_or_default();
            if t.table_type == "table" {
                let rows = sqlx::query(&format!("SELECT * FROM {}", qi(&t.name)))
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| e.to_string())?;
                let maps: Vec<Map<String, Value>> = rows.iter().map(Self::row_to_map).collect();
                table_dumps.push((ddl, t.name.clone(), maps));
            } else {
                extras.push(ddl); // view DDL
            }
        }
        // triggers
        let trgs = sqlx::query(
            "SELECT TRIGGER_NAME, EVENT_MANIPULATION, ACTION_TIMING, ACTION_STATEMENT, EVENT_OBJECT_TABLE
             FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA = DATABASE();",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        for r in &trgs {
            extras.push(format!(
                "CREATE TRIGGER {} {} {} ON {} FOR EACH ROW {}",
                r.try_get::<String, _>("TRIGGER_NAME").unwrap_or_default(),
                r.try_get::<String, _>("ACTION_TIMING").unwrap_or_default(),
                r.try_get::<String, _>("EVENT_MANIPULATION").unwrap_or_default(),
                qi(&r.try_get::<String, _>("EVENT_OBJECT_TABLE").unwrap_or_default()),
                r.try_get::<String, _>("ACTION_STATEMENT").unwrap_or_default(),
            ));
        }

        let content = common::dump_content(Dialect::Backtick, &table_dumps, &extras);
        let mut path = dest_dir.to_path_buf();
        path.push(format!("{}-{}.sql", base, ts));
        std::fs::write(&path, content).map_err(|e| format!("Backup dump failed: {}", e))?;
        Ok(path.to_string_lossy().to_string())
    }

    async fn server_version(&self) -> Result<String, String> {
        let row = sqlx::query("SELECT VERSION() AS v;")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let v: String = row.try_get("v").unwrap_or_default();
        Ok(format!("MySQL {}", v))
    }

    async fn get_database_stats(&self) -> Result<DatabaseStats, String> {
        let row = sqlx::query(
            "SELECT
                SUM(TABLE_TYPE = 'BASE TABLE') AS tables,
                SUM(TABLE_TYPE = 'VIEW') AS views,
                COALESCE(SUM(TABLE_ROWS), 0) AS rows_est,
                COALESCE(SUM(DATA_LENGTH + INDEX_LENGTH), 0) AS bytes
             FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE();",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let idx: i64 = sqlx::query(
            "SELECT COUNT(*) FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = DATABASE();",
        )
        .fetch_one(&self.pool)
        .await
        .and_then(|r| r.try_get(0))
        .unwrap_or(0);
        let trg: i64 = sqlx::query(
            "SELECT COUNT(*) FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA = DATABASE();",
        )
        .fetch_one(&self.pool)
        .await
        .and_then(|r| r.try_get(0))
        .unwrap_or(0);

        let engine: String = sqlx::query("SELECT @@default_storage_engine AS v;")
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get("v"))
            .unwrap_or_default();
        let charset: String = sqlx::query("SELECT @@character_set_database AS v;")
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get("v"))
            .unwrap_or_default();
        let page_size: i64 = sqlx::query("SELECT @@innodb_page_size AS v;")
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get("v"))
            .unwrap_or(16384);

        let bytes: i64 = row.try_get("bytes").unwrap_or(0);
        let version = self.server_version().await.unwrap_or_else(|_| "MySQL".into());

        Ok(DatabaseStats {
            driver_name: "MySQL".to_string(),
            version,
            file_path: Some(format!("{}:{}/{}", self.host, self.port, self.database)),
            file_size_bytes: bytes.max(0) as u64,
            page_count: if page_size > 0 { (bytes / page_size).max(0) as u64 } else { 0 },
            page_size: page_size.max(0) as u64,
            freelist_pages: 0,
            journal_mode: engine,
            encoding: charset,
            table_count: row.try_get::<i64, _>("tables").unwrap_or(0).max(0) as u64,
            view_count: row.try_get::<i64, _>("views").unwrap_or(0).max(0) as u64,
            index_count: idx.max(0) as u64,
            trigger_count: trg.max(0) as u64,
            total_rows: row.try_get::<i64, _>("rows_est").unwrap_or(0).max(0) as u64,
        })
    }

    /// MySQL supports `ALTER TABLE ... ADD CONSTRAINT ... FOREIGN KEY` natively.
    async fn add_foreign_key(
        &self,
        constraint_name: &str,
        table: &str,
        column: &str,
        ref_table: &str,
        ref_column: &str,
        on_delete: &str,
        on_update: &str,
    ) -> Result<String, String> {
        let sql = common::add_fk_sql(
            Dialect::Backtick,
            constraint_name,
            table,
            column,
            ref_table,
            ref_column,
            on_delete,
            on_update,
        );
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("ADD FOREIGN KEY failed: {}", e))?;
        Ok(format!("Executed: {}", sql))
    }
}

#[cfg(test)]
mod tests;
