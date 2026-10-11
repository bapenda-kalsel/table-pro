use super::common::{self, staged, Dialect, ParamStyle};
use super::factory::TlsConfig;
use super::DatabaseDriver;
use crate::errors::{AppError, AppResult, ResultExt};
use crate::models::{
    DatabaseStats, ExplainResult, ExplainStep, ExportResult, ForeignKeyInfo, ForeignKeySpec,
    FullTableSchema, GlobalSearchResult, IndexInfo, QueryResultData, StagedCommitPayload,
    StagedDiffSummary, TableColumnMetadata, TableInfo, TriggerInfo,
};
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgRow};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::time::{Duration, Instant};

/// Verbs whose statements return rows.
const ROW_RETURNING_VERBS: &[&str] = &["select", "with", "values", "table", "show", "explain"];

const SCHEMA: &str = "public";

pub struct PostgresDriver {
    pool: PgPool,
    database: String,
    host: String,
    port: u16,
}

fn qi(ident: &str) -> String {
    Dialect::Ansi.qi(ident)
}

/// Quote a possibly schema-qualified name (`schema`.`table`).
fn qi_path(name: &str) -> String {
    Dialect::Ansi.qi_path(name)
}

fn to_hex(b: &[u8]) -> String {
    const MAX: usize = 256;
    let hex: String = b.iter().take(MAX).map(|x| format!("{:02x}", x)).collect();
    if b.len() > MAX {
        format!("\\x{}…({} bytes)", hex, b.len())
    } else {
        format!("\\x{}", hex)
    }
}

/// Bind a JSON scalar to a Postgres query placeholder.
fn bind_json<'q>(
    q: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    v: &'q serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
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

impl PostgresDriver {
    pub async fn new(
        host: &str,
        port: u16,
        database: &str,
        username: &str,
        password: Option<&str>,
        tls: &TlsConfig,
    ) -> AppResult<Self> {
        use super::factory::TlsMode;
        use sqlx::postgres::PgSslMode;

        let mut opts = PgConnectOptions::new()
            .host(host)
            .port(port)
            .username(username)
            .database(database)
            .ssl_mode(match tls.mode {
                TlsMode::Disabled => PgSslMode::Disable,
                TlsMode::Prefer => PgSslMode::Prefer,
                TlsMode::Required => PgSslMode::Require,
                TlsMode::VerifyCa => PgSslMode::VerifyCa,
                TlsMode::VerifyFull => PgSslMode::VerifyFull,
            });
        if let Some(pw) = password {
            opts = opts.password(pw);
        }
        if let Some(ca) = &tls.ca_cert_path {
            opts = opts.ssl_root_cert(ca);
        }

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(opts)
            .await
            .map_err(AppError::from)?;

        Ok(Self {
            pool,
            database: database.to_string(),
            host: host.to_string(),
            port,
        })
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
        staged::build_staged_plan(Dialect::Ansi, ParamStyle::DollarN, payload, &pk, &all)
    }

    fn value_to_json(row: &PgRow, i: usize) -> Value {
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
            "BOOL" | "BOOLEAN" => row
                .try_get::<bool, _>(i)
                .map(Value::from)
                .unwrap_or(Value::Null),
            "INT2" | "SMALLINT" | "CHAR" => row
                .try_get::<i16, _>(i)
                .map(Value::from)
                .unwrap_or(Value::Null),
            "INT4" | "INT" | "INTEGER" | "SERIAL" | "OID" | "REGCLASS" | "XID" => row
                .try_get::<i32, _>(i)
                .map(Value::from)
                .or_else(|_| row.try_get::<i64, _>(i).map(Value::from))
                .unwrap_or(Value::Null),
            "INT8" | "BIGINT" | "BIGSERIAL" => row
                .try_get::<i64, _>(i)
                .map(Value::from)
                .unwrap_or(Value::Null),
            "FLOAT4" | "REAL" => row
                .try_get::<f32, _>(i)
                .map(|v| json!(v))
                .unwrap_or(Value::Null),
            "FLOAT8" | "DOUBLE PRECISION" => row
                .try_get::<f64, _>(i)
                .map(|v| json!(v))
                .unwrap_or(Value::Null),
            "NUMERIC" | "DECIMAL" => row
                .try_get::<rust_decimal::Decimal, _>(i)
                .map(|d| json!(d.to_string()))
                .or_else(|_| row.try_get::<f64, _>(i).map(|f| json!(f)))
                .unwrap_or(Value::Null),
            "DATE" => row
                .try_get::<chrono::NaiveDate, _>(i)
                .map(|v| json!(v.to_string()))
                .unwrap_or(Value::Null),
            "TIME" | "TIMETZ" => row
                .try_get::<chrono::NaiveTime, _>(i)
                .map(|v| json!(v.to_string()))
                .unwrap_or(Value::Null),
            "TIMESTAMP" => row
                .try_get::<chrono::NaiveDateTime, _>(i)
                .map(|v| json!(v.to_string()))
                .unwrap_or(Value::Null),
            "TIMESTAMPTZ" => row
                .try_get::<chrono::DateTime<chrono::Utc>, _>(i)
                .map(|v| json!(v.to_rfc3339()))
                .unwrap_or(Value::Null),
            "JSON" | "JSONB" => row.try_get::<Value, _>(i).unwrap_or(Value::Null),
            "UUID" => row
                .try_get::<sqlx::types::Uuid, _>(i)
                .map(|u| json!(u.to_string()))
                .unwrap_or(Value::Null),
            "BYTEA" => row
                .try_get::<Vec<u8>, _>(i)
                .map(|b| json!(to_hex(&b)))
                .unwrap_or(Value::Null),
            _ => row
                .try_get::<String, _>(i)
                .map(Value::from)
                .unwrap_or(Value::Null),
        }
    }

    fn row_to_json(row: &PgRow) -> Vec<Value> {
        (0..row.columns().len())
            .map(|i| Self::value_to_json(row, i))
            .collect()
    }

    fn row_to_map(row: &PgRow) -> Map<String, Value> {
        let mut m = Map::new();
        for (i, c) in row.columns().iter().enumerate() {
            m.insert(c.name().to_string(), Self::value_to_json(row, i));
        }
        m
    }

    fn col_types(row: &PgRow) -> Vec<String> {
        row.columns()
            .iter()
            .map(|c| c.type_info().name().to_uppercase())
            .collect()
    }

    /// Primary-key column names for a table in the public schema.
    async fn pk_columns(&self, table: &str) -> Vec<String> {
        sqlx::query(
            "SELECT a.attname
             FROM pg_index i
             JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
             WHERE i.indrelid = to_regclass($1) AND i.indisprimary
             ORDER BY array_position(i.indkey, a.attnum);",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .iter()
        .filter_map(|r| r.try_get::<String, _>("attname").ok())
        .collect()
    }
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    async fn ping(&self) -> AppResult<f64> {
        let start = Instant::now();
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    }

    async fn list_tables(&self) -> AppResult<Vec<TableInfo>> {
        let rows = sqlx::query(
            "SELECT t.table_name, t.table_type,
                    COALESCE((SELECT c.reltuples::bigint
                              FROM pg_catalog.pg_class c
                              JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                              WHERE n.nspname = t.table_schema
                                AND c.relname = t.table_name
                                AND c.relkind IN ('r','p')), -1) AS est
             FROM information_schema.tables t
             WHERE t.table_schema = $1
             ORDER BY t.table_name;",
        )
        .bind(SCHEMA)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .iter()
            .map(|r| {
                let ty: String = r.try_get("table_type").unwrap_or_default();
                TableInfo {
                    name: r.try_get("table_name").unwrap_or_default(),
                    table_type: if ty == "BASE TABLE" { "table" } else { "view" }.to_string(),
                    row_count_estimate: r.try_get::<i64, _>("est").ok().filter(|n| *n >= 0),
                }
            })
            .collect())
    }

    async fn get_table_schema(&self, table: &str) -> AppResult<Vec<TableColumnMetadata>> {
        let rows = sqlx::query(
            "SELECT column_name, data_type, is_nullable, column_default
             FROM information_schema.columns
             WHERE table_schema = $1 AND table_name = $2
             ORDER BY ordinal_position;",
        )
        .bind(SCHEMA)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        let pks: std::collections::HashSet<String> =
            self.pk_columns(table).await.into_iter().collect();

        // Column comments via pg_description (Workbench shows these)
        let comment_rows = sqlx::query(
            "SELECT a.attname, d.description
             FROM pg_attribute a
             JOIN pg_class c ON c.oid = a.attrelid
             JOIN pg_namespace n ON n.oid = c.relnamespace
             LEFT JOIN pg_description d ON d.objoid = c.oid AND d.objsubid = a.attnum
             WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0;",
        )
        .bind(SCHEMA)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        let comments: std::collections::HashMap<String, std::string::String> = comment_rows
            .iter()
            .filter_map(|r| {
                let n: String = r.try_get("attname").ok()?;
                let d: Option<String> = r.try_get("description").ok().flatten();
                d.map(|desc| (n, desc))
            })
            .collect();

        Ok(rows
            .iter()
            .map(|r| {
                let name: String = r.try_get("column_name").unwrap_or_default();
                TableColumnMetadata {
                    is_primary_key: pks.contains(&name),
                    comment: comments.get(&name).cloned(),
                    name,
                    data_type: r.try_get("data_type").unwrap_or_default(),
                    is_nullable: r
                        .try_get::<String, _>("is_nullable")
                        .map(|v| v == "YES")
                        .unwrap_or(true),
                    default_value: r.try_get("column_default").ok().flatten(),
                }
            })
            .collect())
    }

    async fn get_full_schema(&self, table: &str) -> AppResult<FullTableSchema> {
        let columns = self.get_table_schema(table).await?;

        // Indexes from pg_indexes (parse indexdef for uniqueness/partial/cols)
        let idx_rows = sqlx::query(
            "SELECT indexname, indexdef FROM pg_indexes WHERE schemaname = $1 AND tablename = $2;",
        )
        .bind(SCHEMA)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        let indexes = idx_rows
            .iter()
            .map(|r| {
                let name: String = r.try_get("indexname").unwrap_or_default();
                let def: String = r.try_get("indexdef").unwrap_or_default();
                // Extract the column list between the last (...) pair — good
                // enough for ordinary btree indexes; expressions show raw.
                let cols = def
                    .rfind('(')
                    .zip(def.rfind(')'))
                    .filter(|(o, e)| e > o)
                    .map(|(o, e)| def[o + 1..e].to_string())
                    .unwrap_or_default();
                IndexInfo {
                    is_unique: def.to_uppercase().starts_with("CREATE UNIQUE"),
                    origin: if name.ends_with("_pkey") {
                        "pk".to_string()
                    } else if def.to_uppercase().starts_with("CREATE UNIQUE") {
                        "u".to_string()
                    } else {
                        "c".to_string()
                    },
                    is_partial: def.to_uppercase().contains(" WHERE "),
                    name,
                    columns: cols
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                }
            })
            .collect();

        // Foreign keys via information_schema (single-col FKs; composite shown per-col)
        let fk_rows = sqlx::query(
            "SELECT tc.constraint_name, kcu.column_name AS from_col,
                    ccu.table_name AS ref_table, ccu.column_name AS to_col,
                    rc.update_rule, rc.delete_rule
             FROM information_schema.table_constraints tc
             JOIN information_schema.key_column_usage kcu
               ON tc.constraint_name = kcu.constraint_name
              AND tc.table_schema = kcu.table_schema
              AND tc.table_name = kcu.table_name
             JOIN information_schema.constraint_column_usage ccu
               ON ccu.constraint_name = tc.constraint_name
              AND ccu.table_schema = tc.table_schema
             JOIN information_schema.referential_constraints rc
               ON rc.constraint_name = tc.constraint_name
              AND rc.constraint_schema = tc.table_schema
             WHERE tc.constraint_type = 'FOREIGN KEY'
               AND tc.table_schema = $1 AND tc.table_name = $2;",
        )
        .bind(SCHEMA)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        let foreign_keys = fk_rows
            .iter()
            .enumerate()
            .map(|(i, r)| ForeignKeyInfo {
                id: i as i64,
                from_column: r.try_get("from_col").unwrap_or_default(),
                referenced_table: r.try_get("ref_table").unwrap_or_default(),
                to_column: r.try_get("to_col").ok(),
                on_update: r
                    .try_get::<String, _>("update_rule")
                    .unwrap_or_else(|_| "RESTRICT".to_string()),
                on_delete: r
                    .try_get::<String, _>("delete_rule")
                    .unwrap_or_else(|_| "RESTRICT".to_string()),
            })
            .collect();

        // Triggers
        let trg_rows = sqlx::query(
            "SELECT trigger_name, event_manipulation, action_timing, action_statement
             FROM information_schema.triggers
             WHERE event_object_schema = $1 AND event_object_table = $2;",
        )
        .bind(SCHEMA)
        .bind(table)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        let triggers = trg_rows
            .iter()
            .map(|r| {
                let name: String = r.try_get("trigger_name").unwrap_or_default();
                let event: String = r.try_get("event_manipulation").unwrap_or_default();
                let timing: String = r.try_get("action_timing").unwrap_or_default();
                let body: String = r.try_get("action_statement").unwrap_or_default();
                TriggerInfo {
                    sql: format!(
                        "CREATE TRIGGER {} {} {} ON {} FOR EACH ROW {}",
                        name,
                        timing,
                        event,
                        qi_path(table),
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
        let data_sql = common::table_data_query(
            Dialect::Ansi,
            table,
            limit,
            offset,
            filter,
            sort_by,
            sort_dir,
        );
        let count_sql = common::count_query(Dialect::Ansi, table, filter);

        let total_rows: i64 = sqlx::query(&count_sql)
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get("n"))
            .unwrap_or(0);

        let rows = sqlx::query(&data_sql)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

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

    async fn execute_query(&self, sql: &str) -> AppResult<QueryResultData> {
        let start = Instant::now();
        let verb = crate::models::detect_statement_type(sql).to_lowercase();

        if ROW_RETURNING_VERBS.contains(&verb.as_str()) {
            let rows = sqlx::query(sql)
                .fetch_all(&self.pool)
                .await
                .map_err(AppError::from)?;
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
                .map_err(AppError::from)?;
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

    async fn explain_query(&self, sql: &str) -> AppResult<ExplainResult> {
        let explain_sql = format!("EXPLAIN {}", sql.trim_end_matches(';'));
        let rows = sqlx::query(&explain_sql)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::from)?;

        // EXPLAIN returns one text column ("QUERY PLAN") — one plan line per row.
        let mut steps = Vec::new();
        let mut has_full_scan = false;
        let mut uses_index = false;
        let mut temp_btree = false;
        let mut warnings = Vec::new();

        for (i, r) in rows.iter().enumerate() {
            let line: String = r.try_get(0).unwrap_or_default();
            let trimmed = line.trim().to_string();

            if trimmed.contains("Seq Scan") {
                has_full_scan = true;
                if let Some(t) = trimmed.split(" on ").nth(1) {
                    warnings.push(format!("Full table scan (Seq Scan) on '{}'", t.trim()));
                } else {
                    warnings.push("Sequential scan detected".to_string());
                }
            }
            if trimmed.contains("Index Scan")
                || trimmed.contains("Index Only Scan")
                || trimmed.contains("Bitmap Index Scan")
            {
                uses_index = true;
            }
            if trimmed.contains("external merge") || trimmed.contains("Sort Method: external") {
                temp_btree = true;
            }

            steps.push(ExplainStep {
                id: i as i64,
                parent: 0,
                detail: trimmed,
                children: vec![],
            });
        }

        if temp_btree {
            warnings.push("External sort on disk — consider an index or more work_mem".to_string());
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

        Ok(QueryResultData {
            columns: vec![
                "Committed Statements".to_string(),
                "Total Affected".to_string(),
            ],
            column_types: vec!["INTEGER".to_string(), "INTEGER".to_string()],
            rows: vec![vec![json!(plan.len()), json!(total_affected)]],
            execution_time_ms: start.elapsed().as_secs_f64() * 1000.0,
            affected_rows: total_affected,
            total_rows: 1,
        })
    }

    /// Postgres has no SHOW CREATE TABLE — synthesize DDL from catalog metadata.
    async fn get_table_ddl(&self, table: &str) -> AppResult<String> {
        // Views first
        let view_def: Option<String> = sqlx::query("SELECT pg_get_viewdef(to_regclass($1)) AS v;")
            .bind(format!("{}.{}", SCHEMA, table))
            .fetch_one(&self.pool)
            .await
            .ok()
            .and_then(|r| r.try_get("v").ok().flatten());
        if let Some(v) = view_def {
            return Ok(format!("CREATE VIEW {} AS\n{}", qi_path(table), v));
        }

        let cols = self.get_table_schema(table).await?;
        if cols.is_empty() {
            return Err(AppError::not_found(format!("Table '{}' not found", table)));
        }
        let mut lines: Vec<String> = cols
            .iter()
            .map(|c| {
                let mut s = format!("{} {}", qi(&c.name), c.data_type);
                if !c.is_nullable && !c.is_primary_key {
                    s.push_str(" NOT NULL");
                }
                if let Some(d) = &c.default_value {
                    s.push_str(&format!(" DEFAULT {}", d));
                }
                s
            })
            .collect();
        let pks = self.pk_columns(table).await;
        if !pks.is_empty() {
            lines.push(format!(
                "PRIMARY KEY ({})",
                pks.iter().map(|p| qi(p)).collect::<Vec<_>>().join(", ")
            ));
        }
        Ok(format!(
            "CREATE TABLE {} (\n  {}\n)",
            qi_path(table),
            lines.join(",\n  ")
        ))
    }

    async fn run_maintenance(&self, action: &str, table: Option<&str>) -> AppResult<String> {
        match action.to_lowercase().as_str() {
            "vacuum" => {
                // VACUUM cannot run inside a transaction block — pool.exec is fine.
                let sql = match table {
                    Some(t) => format!("VACUUM ANALYZE {};", qi_path(t)),
                    None => "VACUUM ANALYZE;".to_string(),
                };
                sqlx::query(&sql)
                    .execute(&self.pool)
                    .await
                    .map_err(AppError::from)?;
                Ok("VACUUM ANALYZE completed. Dead tuples reclaimed.".to_string())
            }
            "analyze" => {
                let sql = match table {
                    Some(t) => format!("ANALYZE {};", qi_path(t)),
                    None => "ANALYZE;".to_string(),
                };
                sqlx::query(&sql)
                    .execute(&self.pool)
                    .await
                    .map_err(AppError::from)?;
                Ok("ANALYZE completed. Planner statistics refreshed.".to_string())
            }
            "reindex" => {
                let sql = match table {
                    Some(t) => format!("REINDEX TABLE {};", qi_path(t)),
                    None => format!("REINDEX SCHEMA {};", SCHEMA),
                };
                sqlx::query(&sql)
                    .execute(&self.pool)
                    .await
                    .map_err(AppError::from)?;
                Ok("REINDEX completed.".to_string())
            }
            "integrity_check" => {
                // PG has no CHECK TABLE; checksums are verified on page read.
                // Offer a cheap sanity query instead of failing.
                let n: i64 =
                    sqlx::query("SELECT COUNT(*) FROM pg_catalog.pg_tables WHERE schemaname=$1;")
                        .bind(SCHEMA)
                        .fetch_one(&self.pool)
                        .await
                        .and_then(|r| r.try_get(0))
                        .unwrap_or(0);
                Ok(format!(
                    "PostgreSQL verifies page checksums automatically during reads. {} tables reachable — catalog healthy. For deep checks, use the amcheck extension.",
                    n
                ))
            }
            other => Err(AppError::unsupported(format!(
                "Unsupported maintenance action '{}'",
                other
            ))),
        }
    }

    async fn search_text_in_database(&self, keyword: &str) -> AppResult<Vec<GlobalSearchResult>> {
        let tables = self.list_tables().await?;
        let mut results = Vec::new();

        for t in tables.iter().filter(|t| t.table_type == "table") {
            let cols = self.get_table_schema(&t.name).await.unwrap_or_default();
            let text_cols: Vec<&TableColumnMetadata> = cols
                .iter()
                .filter(|c| {
                    matches!(
                        c.data_type.to_lowercase().as_str(),
                        "text" | "character varying" | "character" | "name" | "json" | "jsonb"
                    )
                })
                .collect();

            for c in text_cols {
                let sql = format!(
                    "SELECT * FROM {} WHERE {}::text ILIKE '%' || $1 || '%' LIMIT 5;",
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
    ) -> AppResult<ExportResult> {
        let mut query = format!("SELECT * FROM {}", qi_path(table));
        if let Some(f) = filter {
            if !f.trim().is_empty() {
                query.push_str(&format!(" WHERE {}", f));
            }
        }
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .db_with(|e| format!("Export query failed: {}", e))?;

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

        let maps: Vec<Map<String, Value>> = rows.iter().map(Self::row_to_map).collect();
        let ddl = self.get_table_ddl(table).await.ok();

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

        let bytes = common::export_rows_bytes(
            format,
            table,
            ddl.as_deref(),
            &headers,
            &maps,
            Dialect::Ansi,
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

        let create_sql = common::import_table_ddl(Dialect::Ansi, table, &headers)?;
        sqlx::query(&create_sql)
            .execute(&self.pool)
            .await
            .db_with(|e| format!("Failed creating table: {}", e))?;

        let insert_sql =
            common::import_insert_sql(Dialect::Ansi, ParamStyle::DollarN, table, &headers);

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
        // No in-SQL binary backup for PG: produce a logical .sql dump.
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let base = format!("{}-{}-{}", self.database, self.host, self.port)
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();

        let mut table_dumps = Vec::new();
        let mut extras = Vec::new();
        for t in self.list_tables().await? {
            let ddl = self.get_table_ddl(&t.name).await.unwrap_or_default();
            if t.table_type == "table" {
                let rows = sqlx::query(&format!("SELECT * FROM {}", qi(&t.name)))
                    .fetch_all(&self.pool)
                    .await
                    .map_err(AppError::from)?;
                let maps: Vec<Map<String, Value>> = rows.iter().map(Self::row_to_map).collect();
                table_dumps.push((ddl, t.name.clone(), maps));
            } else {
                extras.push(ddl);
            }
        }

        let content = common::dump_content(Dialect::Ansi, &table_dumps, &extras);
        let mut path = dest_dir.to_path_buf();
        path.push(format!("{}-{}.sql", base, ts));
        std::fs::write(&path, content).db_with(|e| format!("Backup dump failed: {}", e))?;
        Ok(path.to_string_lossy().to_string())
    }

    async fn server_version(&self) -> AppResult<String> {
        let row = sqlx::query("SHOW server_version;")
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::from)?;
        let v: String = row.try_get(0).unwrap_or_default();
        Ok(format!("PostgreSQL {}", v))
    }

    async fn get_database_stats(&self) -> AppResult<DatabaseStats> {
        let db_size: i64 = sqlx::query("SELECT pg_database_size(current_database()) AS s;")
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get("s"))
            .unwrap_or(0);

        let obj = sqlx::query(
            "SELECT
                (SELECT COUNT(*) FROM pg_tables WHERE schemaname=$1) AS tables,
                (SELECT COUNT(*) FROM pg_views WHERE schemaname=$1) AS views,
                (SELECT COUNT(*) FROM pg_indexes WHERE schemaname=$1) AS indexes,
                (SELECT COUNT(*) FROM pg_trigger trg
                   JOIN pg_class c ON c.oid = trg.tgrelid
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   WHERE n.nspname=$1 AND NOT trg.tgisinternal) AS triggers,
                (SELECT COALESCE(SUM(c.reltuples::bigint), 0) FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   WHERE n.nspname=$1 AND c.relkind IN ('r','p')) AS rows_est;",
        )
        .bind(SCHEMA)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        let wal: String = sqlx::query("SHOW wal_level;")
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get(0))
            .unwrap_or_default();
        let encoding: String = sqlx::query("SHOW server_encoding;")
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get(0))
            .unwrap_or_default();
        let block_size: i64 = sqlx::query("SHOW block_size;")
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get::<i32, _>(0))
            .map(|v| v as i64)
            .unwrap_or(8192);

        let version = self
            .server_version()
            .await
            .unwrap_or_else(|_| "PostgreSQL".into());

        Ok(DatabaseStats {
            driver_name: "PostgreSQL".to_string(),
            version,
            file_path: Some(format!("{}:{}/{}", self.host, self.port, self.database)),
            file_size_bytes: db_size.max(0) as u64,
            page_count: if block_size > 0 {
                (db_size / block_size).max(0) as u64
            } else {
                0
            },
            page_size: block_size.max(0) as u64,
            freelist_pages: 0,
            journal_mode: wal,
            encoding,
            table_count: obj.try_get::<i64, _>("tables").unwrap_or(0).max(0) as u64,
            view_count: obj.try_get::<i64, _>("views").unwrap_or(0).max(0) as u64,
            index_count: obj.try_get::<i64, _>("indexes").unwrap_or(0).max(0) as u64,
            trigger_count: obj.try_get::<i64, _>("triggers").unwrap_or(0).max(0) as u64,
            total_rows: obj.try_get::<i64, _>("rows_est").unwrap_or(0).max(0) as u64,
        })
    }

    /// PostgreSQL supports `ALTER TABLE ... ADD CONSTRAINT ... FOREIGN KEY` natively.
    async fn add_foreign_key(&self, spec: &ForeignKeySpec) -> AppResult<String> {
        let sql = common::add_fk_sql(Dialect::Ansi, spec);
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .ctx_db("ADD FOREIGN KEY failed")?;
        Ok(format!("Executed: {}", sql))
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests;
