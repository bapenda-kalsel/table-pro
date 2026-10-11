use super::super::DatabaseDriver;
use super::{qi, qi_path, to_hex};
use std::str::FromStr;

#[test]
fn mysql_identifier_quoting() {
    assert_eq!(qi("users"), "`users`");
    assert_eq!(qi("a`b"), "`a``b`");
    assert_eq!(qi_path("shop.orders"), "`shop`.`orders`");
    assert_eq!(qi_path("plain"), "`plain`");
}

#[test]
fn blob_hex_encoding() {
    assert_eq!(to_hex(&[0xDE, 0xAD, 0xBE, 0xEF]), "0xdeadbeef");
    // >256 bytes gets truncated with a marker
    let big = vec![0xABu8; 300];
    let out = to_hex(&big);
    assert!(out.ends_with("(300 bytes)"));
}

/// Live MySQL smoke test — skipped unless TABLEPRO_TEST_MYSQL is set
/// (e.g. `mysql://root:secret@127.0.0.1:3306/testdb`). Covers connect,
/// list_tables, schema introspection, data read, explain and stats.
#[tokio::test]
async fn mysql_integration_smoke() {
    let url = match std::env::var("TABLEPRO_TEST_MYSQL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("skipped: set TABLEPRO_TEST_MYSQL to run");
            return;
        }
    };
    let opts = sqlx::mysql::MySqlConnectOptions::from_str(&url).unwrap();
    let pool = sqlx::mysql::MySqlPool::connect_with(opts).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS tp_test").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE tp_test (id INT PRIMARY KEY AUTO_INCREMENT, name VARCHAR(100), val DECIMAL(10,2))")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tp_test (name, val) VALUES ('a', 1.5), ('b', 2.5)")
        .execute(&pool)
        .await
        .unwrap();
    drop(pool);

    let opts = sqlx::mysql::MySqlConnectOptions::from_str(&url).unwrap();
    let d = super::MysqlDriver {
        pool: sqlx::mysql::MySqlPoolOptions::new()
            .connect_with(opts.clone())
            .await
            .unwrap(),
        database: opts.get_database().unwrap_or("").to_string(),
        host: opts.get_host().to_string(),
        port: opts.get_port(),
    };

    assert!(d.ping().await.unwrap() >= 0.0);
    assert!(d.list_tables().await.unwrap().iter().any(|t| t.name == "tp_test"));
    let cols = d.get_table_schema("tp_test").await.unwrap();
    assert_eq!(cols.len(), 3);
    assert!(cols[0].is_primary_key);

    let data = d.get_table_data("tp_test", 10, 0, None, None, None).await.unwrap();
    assert_eq!(data.rows.len(), 2);
    assert_eq!(data.total_rows, 2);

    let sel = d.execute_query("SELECT name FROM tp_test WHERE id=1").await.unwrap();
    assert_eq!(sel.rows[0][0], serde_json::json!("a"));

    let full = d.get_full_schema("tp_test").await.unwrap();
    assert!(full.ddl.unwrap().contains("CREATE TABLE"));

    let exp = d.explain_query("SELECT * FROM tp_test WHERE id=1").await.unwrap();
    assert!(!exp.steps.is_empty());

    let stats = d.get_database_stats().await.unwrap();
    assert!(stats.table_count >= 1);
    assert!(stats.version.starts_with("MySQL") || stats.version.starts_with("MariaDB"));

    // staged commit roundtrip
    let payload = crate::models::StagedCommitPayload {
        connection_id: "t".into(),
        table_name: "tp_test".into(),
        primary_keys: vec!["id".into()],
        inserts: vec![serde_json::json!({"name": "c", "val": 3.5})],
        updates: vec![],
        deletes: vec![],
    };
    let res = d.commit_staged_changes(payload).await.unwrap();
    assert_eq!(res.affected_rows, 1);

    let cleanup = sqlx::mysql::MySqlPool::connect_with(opts).await.unwrap();
    sqlx::query("DROP TABLE tp_test").execute(&cleanup).await.unwrap();
}
