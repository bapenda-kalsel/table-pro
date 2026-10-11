use super::*;
use crate::models::{RowUpdatePayload, StagedCommitPayload};
use serde_json::json;
use sqlx::Executor;

/// Driver bound to a shared-cache in-memory DB, seeded with two tables.
async fn seeded() -> SqliteDriver {
    let d = SqliteDriver::new(":memory:").await.unwrap();
    d.pool
        .execute(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                email TEXT UNIQUE,
                balance REAL,
                is_active INTEGER DEFAULT 1
            );",
        )
        .await
        .unwrap();
    d.pool
        .execute(
            "INSERT INTO users (name, email, balance, is_active) VALUES
             ('Alice', 'alice@x.com', 10.5, 1),
             ('Bob', 'bob@x.com', 20.0, 0),
             ('Carol', 'carol@x.com', NULL, 1);",
        )
        .await
        .unwrap();
    d.pool
        .execute("CREATE INDEX idx_users_email ON users(email);")
        .await
        .unwrap();
    d.pool
        .execute(
            "CREATE TABLE projects (
                id INTEGER PRIMARY KEY,
                user_id INTEGER REFERENCES users(id)
            );",
        )
        .await
        .unwrap();
    d.pool
        .execute(
            "CREATE TRIGGER trg_users AFTER INSERT ON users BEGIN SELECT 1; END;",
        )
        .await
        .unwrap();
    d
}

fn seed_table() -> StagedCommitPayload {
    StagedCommitPayload {
        connection_id: "test".to_string(),
        table_name: "users".to_string(),
        primary_keys: vec!["id".to_string()],
        inserts: vec![json!({"name": "Dave", "email": "dave@x.com", "balance": 5.0})],
        updates: vec![RowUpdatePayload {
            primary_key_values: json!({"id": 1}),
            updated_fields: json!({"name": "Alice2"}),
        }],
        deletes: vec![json!({"id": 3})],
    }
}

#[tokio::test]
async fn ping_returns_latency() {
    let d = seeded().await;
    assert!(d.ping().await.unwrap() >= 0.0);
}

#[tokio::test]
async fn list_tables_shows_tables_and_counts() {
    let d = seeded().await;
    let tables = d.list_tables().await.unwrap();
    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"users"));
    assert!(names.contains(&"projects"));
    let users = tables.iter().find(|t| t.name == "users").unwrap();
    assert_eq!(users.row_count_estimate, Some(3));
    assert_eq!(users.table_type, "table");
}

#[tokio::test]
async fn schema_reports_pk_nullable_defaults() {
    let d = seeded().await;
    let cols = d.get_table_schema("users").await.unwrap();
    assert_eq!(cols.len(), 5);
    let id = &cols[0];
    assert_eq!(id.name, "id");
    assert!(id.is_primary_key);
    let is_active = cols.iter().find(|c| c.name == "is_active").unwrap();
    assert_eq!(is_active.default_value.as_deref(), Some("1"));
    let balance = cols.iter().find(|c| c.name == "balance").unwrap();
    assert!(balance.is_nullable);
}

#[tokio::test]
async fn full_schema_includes_indexes_fks_triggers() {
    let d = seeded().await;
    let s = d.get_full_schema("users").await.unwrap();
    assert!(s.indexes.iter().any(|i| i.name == "idx_users_email" && i.columns == vec!["email"]));
    assert!(s.triggers.iter().any(|t| t.name == "trg_users"));
    assert!(s.ddl.unwrap().contains("CREATE TABLE"));

    let p = d.get_full_schema("projects").await.unwrap();
    assert_eq!(p.foreign_keys.len(), 1);
    assert_eq!(p.foreign_keys[0].from_column, "user_id");
    assert_eq!(p.foreign_keys[0].referenced_table, "users");
}

#[tokio::test]
async fn table_data_pagination_filter_sort() {
    let d = seeded().await;
    // page size 2 -> first page
    let p1 = d.get_table_data("users", 2, 0, None, None, None).await.unwrap();
    assert_eq!(p1.rows.len(), 2);
    assert_eq!(p1.total_rows, 3);
    // second page
    let p2 = d.get_table_data("users", 2, 2, None, None, None).await.unwrap();
    assert_eq!(p2.rows.len(), 1);
    // filter
    let f = d
        .get_table_data("users", 100, 0, Some("is_active = 1"), None, None)
        .await
        .unwrap();
    assert_eq!(f.rows.len(), 2);
    assert_eq!(f.total_rows, 2);
    // sort desc by balance
    let s = d
        .get_table_data("users", 100, 0, None, Some("balance"), Some("DESC"))
        .await
        .unwrap();
    assert_eq!(s.rows[0][1], json!("Bob"));
}

#[tokio::test]
async fn sort_dir_injection_is_sanitized() {
    let d = seeded().await;
    // A malicious sort_dir collapses to ASC instead of executing.
    let res = d
        .get_table_data("users", 10, 0, None, Some("name"), Some("DESC; DROP TABLE users"))
        .await
        .unwrap();
    assert_eq!(res.rows.len(), 3);
    // table still exists
    assert!(d.get_table_data("users", 1, 0, None, None, None).await.is_ok());
}

#[tokio::test]
async fn empty_table_still_returns_headers() {
    let d = seeded().await;
    d.pool.execute("CREATE TABLE empty_t (a INTEGER, b TEXT);").await.unwrap();
    let res = d.get_table_data("empty_t", 10, 0, None, None, None).await.unwrap();
    assert_eq!(res.rows.len(), 0);
    assert_eq!(res.columns, vec!["a", "b"]);
}

#[tokio::test]
async fn execute_query_handles_select_and_writes() {
    let d = seeded().await;
    let sel = d.execute_query("SELECT name FROM users WHERE id = 1").await.unwrap();
    assert_eq!(sel.rows[0][0], json!("Alice"));

    let ins = d
        .execute_query("INSERT INTO users (name, email) VALUES ('Zed', 'zed@x.com')")
        .await
        .unwrap();
    assert_eq!(ins.affected_rows, 1);

    let err = d.execute_query("SELECT * FROM nope").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn explain_detects_index_use_and_full_scans() {
    let d = seeded().await;
    // indexed column -> uses index
    let indexed = d
        .explain_query("SELECT * FROM users WHERE email = 'a@x.com'")
        .await
        .unwrap();
    assert!(indexed.uses_index);
    assert!(!indexed.has_full_scan);

    // non-indexed column -> full scan warning
    let scan = d
        .explain_query("SELECT * FROM users WHERE balance > 1")
        .await
        .unwrap();
    assert!(scan.has_full_scan);
    assert!(scan.warnings.iter().any(|w| w.contains("Full table scan")));
    assert!(!scan.steps.is_empty());
}

#[tokio::test]
async fn staged_changes_preview_and_commit() {
    let d = seeded().await;
    let payload = seed_table();

    let diff = d.preview_staged_changes(&payload).await.unwrap();
    assert_eq!(diff.insert_count, 1);
    assert_eq!(diff.update_count, 1);
    assert_eq!(diff.delete_count, 1);
    assert_eq!(diff.generated_sql.len(), 3);
    assert!(diff.generated_sql[0].starts_with("INSERT INTO \"users\""));
    assert!(diff.generated_sql[1].starts_with("UPDATE \"users\""));
    assert!(diff.generated_sql[2].starts_with("DELETE FROM \"users\""));
    // literal escaping check
    assert!(diff.generated_sql[0].contains("'Dave'"));

    let res = d.commit_staged_changes(payload).await.unwrap();
    assert_eq!(res.affected_rows, 3);

    // verify data actually changed
    let data = d.get_table_data("users", 100, 0, None, Some("id"), Some("ASC")).await.unwrap();
    assert_eq!(data.rows.len(), 3); // 3 - 1 deleted + 1 inserted
    assert_eq!(data.rows[0][1], json!("Alice2")); // updated
    assert_eq!(data.rows[2][1], json!("Dave")); // inserted
}

#[tokio::test]
async fn staged_insert_skips_empty_strings_for_defaults() {
    let d = seeded().await;
    let mut payload = seed_table();
    payload.updates.clear();
    payload.deletes.clear();
    // empty strings should be dropped so AUTOINCREMENT/DEFAULT apply
    payload.inserts = vec![json!({"id": "", "name": "Eve", "email": "eve@x.com"})];
    let diff = d.preview_staged_changes(&payload).await.unwrap();
    assert!(!diff.generated_sql[0].contains("\"id\""));
    d.commit_staged_changes(payload).await.unwrap();
    let data = d.get_table_data("users", 100, 0, Some("name = 'Eve'"), None, None).await.unwrap();
    assert_eq!(data.rows.len(), 1);
}

#[tokio::test]
async fn ddl_and_version() {
    let d = seeded().await;
    let ddl = d.get_table_ddl("users").await.unwrap();
    assert!(ddl.contains("CREATE TABLE users"));
    assert!(d.server_version().await.unwrap().starts_with("SQLite"));
}

#[tokio::test]
async fn maintenance_actions() {
    let d = seeded().await;
    assert!(d.run_maintenance("vacuum", None).await.unwrap().contains("VACUUM"));
    assert!(d.run_maintenance("integrity_check", None).await.unwrap().contains("ok"));
    assert!(d.run_maintenance("analyze", None).await.is_ok());
    assert!(d.run_maintenance("reindex", None).await.is_ok());
    assert!(d.run_maintenance("bogus", None).await.is_err());
}

#[tokio::test]
async fn global_text_search() {
    let d = seeded().await;
    let res = d.search_text_in_database("alice@x.com").await.unwrap();
    assert!(res.iter().any(|r| r.table_name == "users" && r.column_name == "email"));

    let none = d.search_text_in_database("zzz-no-match").await.unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn export_csv_json_sql_xlsx() {
    let d = seeded().await;
    let dir = tempfile::tempdir().unwrap();

    // CSV
    let csv_res = d.export_table("users", "csv", None, dir.path()).await.unwrap();
    let csv_text = std::fs::read_to_string(&csv_res.file_path).unwrap();
    assert!(csv_text.lines().next().unwrap().contains("name"));
    assert_eq!(csv_res.row_count, 3);

    // JSON
    let json_res = d.export_table("users", "json", None, dir.path()).await.unwrap();
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(&json_res.file_path).unwrap()).unwrap();
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[0]["name"], json!("Alice"));

    // SQL dump: DDL + INSERTs
    let sql_res = d.export_table("users", "sql", None, dir.path()).await.unwrap();
    let dump = std::fs::read_to_string(&sql_res.file_path).unwrap();
    assert!(dump.contains("CREATE TABLE users"));
    assert_eq!(dump.matches("INSERT INTO \"users\"").count(), 3);

    // XLSX: valid ZIP magic bytes "PK"
    let xlsx_res = d.export_table("users", "xlsx", None, dir.path()).await.unwrap();
    let bytes = std::fs::read(&xlsx_res.file_path).unwrap();
    assert_eq!(&bytes[0..2], &[0x50, 0x4B]);
    assert_eq!(xlsx_res.row_count, 3);

    // filter respected: only 1 row exported
    let filtered = d
        .export_table("users", "csv", Some("id = 1"), dir.path())
        .await
        .unwrap();
    let text = std::fs::read_to_string(&filtered.file_path).unwrap();
    assert_eq!(filtered.row_count, 1);
    assert_eq!(text.lines().count(), 2); // header + 1 row

    // unsupported format errors
    assert!(d.export_table("users", "xml", None, dir.path()).await.is_err());
}

#[tokio::test]
async fn import_csv_creates_table_and_rows() {
    let d = seeded().await;
    let dir = tempfile::tempdir().unwrap();
    let csv_path = dir.path().join("import.csv");
    std::fs::write(&csv_path, "id,name\n1,Widget\n2,Gadget\n").unwrap();

    let count = d
        .import_csv("gadgets", csv_path.to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(count, 2);

    let data = d.get_table_data("gadgets", 10, 0, None, None, None).await.unwrap();
    assert_eq!(data.rows.len(), 2);
    assert_eq!(data.columns, vec!["id", "name"]);

    // import into existing table appends
    let csv2 = dir.path().join("import2.csv");
    std::fs::write(&csv2, "name,email,balance,is_active\nNew,new@x.com,9.9,1\n").unwrap();
    let n2 = d.import_csv("users", csv2.to_str().unwrap()).await.unwrap();
    assert_eq!(n2, 1);

    // bad header rejected
    let bad = dir.path().join("bad.csv");
    std::fs::write(&bad, "bad header!,x\n1,2\n").unwrap();
    assert!(d.import_csv("x", bad.to_str().unwrap()).await.is_err());
}

#[tokio::test]
async fn backup_produces_file() {
    // File-backed DB (the real-world case for backups).
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("source.db");
    let d = SqliteDriver::new(db_path.to_str().unwrap()).await.unwrap();
    d.pool
        .execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT);")
        .await
        .unwrap();
    d.pool
        .execute("INSERT INTO t (v) VALUES ('x'), ('y');")
        .await
        .unwrap();

    let out_dir = dir.path().join("backups");
    std::fs::create_dir_all(&out_dir).unwrap();
    let path = d.backup_database(&out_dir).await.unwrap();
    if !std::path::Path::new(&path).exists() {
        let listing: Vec<_> = std::fs::read_dir(&out_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        panic!("backup file {} missing; dir contains {:?}", path, listing);
    }
    // SQLite file magic: "SQLite format 3\0"
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[0..6], b"SQLite");
}

#[tokio::test]
async fn database_stats_reported() {
    let d = seeded().await;
    let s = d.get_database_stats().await.unwrap();
    assert_eq!(s.driver_name, "SQLite");
    assert!(s.version.starts_with("SQLite"));
    assert_eq!(s.table_count, 2); // users + projects (sqlite_sequence filtered out)
    assert_eq!(s.view_count, 0);
    assert!(s.index_count >= 1); // idx_users_email (+ sqlite_autoindex for UNIQUE)
    assert_eq!(s.trigger_count, 1);
    assert_eq!(s.total_rows, 3);
    assert!(s.page_count > 0);
    assert!(s.page_size > 0);
    assert!(s.file_size_bytes > 0);
    assert!(s.file_path.is_none()); // :memory:
    assert!(!s.journal_mode.is_empty());
}

#[tokio::test]
async fn backup_memory_db_falls_back_to_sql_dump() {
    // Shared-cache :memory: pools can't VACUUM INTO, so we get a logical
    // .sql dump instead — still a complete, restorable backup.
    let d = seeded().await;
    let dir = tempfile::tempdir().unwrap();
    let path = d.backup_database(dir.path()).await.unwrap();
    assert!(std::path::Path::new(&path).exists(), "{}", path);
    assert!(path.ends_with(".sql"));
    let dump = std::fs::read_to_string(&path).unwrap();
    assert!(dump.contains("CREATE TABLE users"));
    assert!(dump.contains("INSERT INTO \"users\""));
    assert!(dump.contains("CREATE TRIGGER"));
    assert!(dump.contains("CREATE INDEX"));
}

#[test]
fn retarget_and_inject_constraint() {
    let ddl = "CREATE TABLE \"orders\" (\n  \"id\" INTEGER PRIMARY KEY,\n  \"total\" REAL DEFAULT (0)\n)";
    let retargeted = retarget_create_table(ddl, "orders__new").unwrap();
    assert!(retargeted.starts_with("CREATE TABLE \"orders__new\"("));

    // inject after DEFAULT (0) — nested parens must not confuse the matcher
    let out = inject_constraint(
        &retargeted,
        "CONSTRAINT \"fk_o\" FOREIGN KEY (\"u\") REFERENCES \"users\"(\"id\")",
    )
    .unwrap();
    assert!(out.contains("DEFAULT (0),\n  CONSTRAINT \"fk_o\" FOREIGN KEY"));
    assert!(out.trim_end().ends_with(')'));
}

#[tokio::test]
async fn add_foreign_key_rebuilds_table() {
    let d = seeded().await;

    // projects already has user_id -> users(id) inline; create a fresh
    // table WITHOUT one, then add the FK via the visual-relation path.
    d.pool
        .execute("CREATE TABLE tasks (id INTEGER PRIMARY KEY, user_id INTEGER);")
        .await
        .unwrap();
    d.pool
        .execute("INSERT INTO tasks (user_id) VALUES (1);")
        .await
        .unwrap();
    d.pool
        .execute("CREATE INDEX idx_tasks_user ON tasks(user_id);")
        .await
        .unwrap();

    let msg = d
        .add_foreign_key("fk_tasks_user", "tasks", "user_id", "users", "id", "CASCADE", "NO ACTION")
        .await
        .unwrap();
    assert!(msg.contains("tasks"));

    // FK is now visible in the schema
    let full = d.get_full_schema("tasks").await.unwrap();
    assert_eq!(full.foreign_keys.len(), 1);
    assert_eq!(full.foreign_keys[0].referenced_table, "users");
    assert_eq!(full.foreign_keys[0].from_column, "user_id");
    assert_eq!(full.foreign_keys[0].on_delete, "CASCADE");

    // data survived the rebuild
    let data = d.get_table_data("tasks", 10, 0, None, None, None).await.unwrap();
    assert_eq!(data.rows.len(), 1);

    // the index was restored
    assert!(full.indexes.iter().any(|i| i.name == "idx_tasks_user"));
}

#[tokio::test]
async fn add_foreign_key_rejects_orphan_rows() {
    let d = seeded().await;
    d.pool
        .execute("CREATE TABLE orphan_tasks (id INTEGER PRIMARY KEY, user_id INTEGER);")
        .await
        .unwrap();
    // 999 references a non-existent users.id
    d.pool
        .execute("INSERT INTO orphan_tasks (user_id) VALUES (999);")
        .await
        .unwrap();

    let err = d
        .add_foreign_key("fk_ot", "orphan_tasks", "user_id", "users", "id", "NO ACTION", "NO ACTION")
        .await;
    assert!(err.is_err());
    assert!(err.err().unwrap().contains("violate"));

    // rollback kept the table and its row intact
    let data = d.get_table_data("orphan_tasks", 10, 0, None, None, None).await.unwrap();
    assert_eq!(data.rows.len(), 1);
    let full = d.get_full_schema("orphan_tasks").await.unwrap();
    assert!(full.foreign_keys.is_empty());
}
