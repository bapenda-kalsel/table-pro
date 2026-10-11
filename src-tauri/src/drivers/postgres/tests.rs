use super::super::DatabaseDriver;
use super::{qi, qi_path, to_hex};
use std::str::FromStr;

#[test]
fn pg_identifier_quoting() {
    assert_eq!(qi("users"), "\"users\"");
    assert_eq!(qi("a\"b"), "\"a\"\"b\"");
    assert_eq!(qi_path("public.orders"), "\"public\".\"orders\"");
    assert_eq!(qi_path("plain"), "\"plain\"");
}

#[test]
fn bytea_hex_encoding() {
    assert_eq!(to_hex(&[0xDE, 0xAD]), "\\xdead");
    let big = vec![0xABu8; 300];
    assert!(to_hex(&big).ends_with("(300 bytes)"));
}

/// Live PostgreSQL smoke test — skipped unless TABLEPRO_TEST_POSTGRES is set
/// (e.g. `postgres://postgres:secret@127.0.0.1:5432/postgres`).
#[tokio::test]
async fn postgres_integration_smoke() {
    let url = match std::env::var("TABLEPRO_TEST_POSTGRES") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("skipped: set TABLEPRO_TEST_POSTGRES to run");
            return;
        }
    };
    let opts = sqlx::postgres::PgConnectOptions::from_str(&url).unwrap();
    let pool = sqlx::postgres::PgPool::connect_with(opts.clone()).await.unwrap();
    sqlx::query("DROP TABLE IF EXISTS tp_test").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE tp_test (id SERIAL PRIMARY KEY, name VARCHAR(100), price NUMERIC(10,2), uid UUID, created TIMESTAMPTZ DEFAULT now())")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tp_test (name, price, uid) VALUES ('a', 1.50, '11111111-1111-1111-1111-111111111111'), ('b', 2.50, '22222222-2222-2222-2222-222222222222')")
        .execute(&pool)
        .await
        .unwrap();

    let d = super::PostgresDriver {
        pool: sqlx::postgres::PgPoolOptions::new()
            .connect_with(opts.clone())
            .await
            .unwrap(),
        database: opts.get_database().unwrap_or("postgres").to_string(),
        host: opts.get_host().to_string(),
        port: opts.get_port(),
    };

    assert!(d.ping().await.unwrap() >= 0.0);
    assert!(d.list_tables().await.unwrap().iter().any(|t| t.name == "tp_test"));

    let cols = d.get_table_schema("tp_test").await.unwrap();
    assert_eq!(cols.len(), 5);
    assert!(cols[0].is_primary_key);

    let data = d.get_table_data("tp_test", 10, 0, None, None, None).await.unwrap();
    assert_eq!(data.rows.len(), 2);
    // NUMERIC -> string, UUID -> uuid string, TIMESTAMPTZ -> rfc3339
    assert!(data.rows[0][4].as_str().unwrap().contains('T') || data.rows[0][4].is_string());

    let sel = d.execute_query("SELECT name FROM tp_test WHERE id=1").await.unwrap();
    assert_eq!(sel.rows[0][0], serde_json::json!("a"));

    let full = d.get_full_schema("tp_test").await.unwrap();
    assert!(full.ddl.unwrap().contains("CREATE TABLE"));

    let exp = d.explain_query("SELECT * FROM tp_test WHERE id=1").await.unwrap();
    assert!(!exp.steps.is_empty());
    assert!(exp.uses_index); // PK lookup should use the index

    let stats = d.get_database_stats().await.unwrap();
    assert!(stats.table_count >= 1);
    assert!(stats.version.starts_with("PostgreSQL"));

    let payload = crate::models::StagedCommitPayload {
        connection_id: "t".into(),
        table_name: "tp_test".into(),
        primary_keys: vec!["id".into()],
        inserts: vec![serde_json::json!({"name": "c", "price": 3.5})],
        updates: vec![],
        deletes: vec![],
    };
    let res = d.commit_staged_changes(payload).await.unwrap();
    assert_eq!(res.affected_rows, 1);

    sqlx::query("DROP TABLE tp_test").execute(&pool).await.unwrap();
}
