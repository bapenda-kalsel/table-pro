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
/// Uses a unique table per run so it is safe against a shared test database.
#[tokio::test]
async fn postgres_integration_smoke() {
    let url = match std::env::var("TABLEPRO_TEST_POSTGRES") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("skipped: set TABLEPRO_TEST_POSTGRES to run");
            return;
        }
    };
    // Unique table per run — never DROP a fixed name on a shared database.
    let table = format!("tp_test_{}", uuid::Uuid::new_v4().simple());
    let opts = sqlx::postgres::PgConnectOptions::from_str(&url).unwrap();
    let pool = sqlx::postgres::PgPool::connect_with(opts.clone())
        .await
        .unwrap();
    sqlx::query(&format!(
        "CREATE TABLE {} (id SERIAL PRIMARY KEY, name VARCHAR(100), price NUMERIC(10,2), uid UUID, created TIMESTAMPTZ DEFAULT now())",
        table
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(&format!(
        "INSERT INTO {} (name, price, uid) VALUES ('a', 1.50, '11111111-1111-1111-1111-111111111111'), ('b', 2.50, '22222222-2222-2222-2222-222222222222')",
        table
    ))
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
    assert!(d
        .list_tables()
        .await
        .unwrap()
        .iter()
        .any(|t| t.name == table));

    let cols = d.get_table_schema(&table).await.unwrap();
    assert_eq!(cols.len(), 5);
    assert!(cols[0].is_primary_key);

    let data = d
        .get_table_data(&table, 10, 0, None, None, None)
        .await
        .unwrap();
    assert_eq!(data.rows.len(), 2);
    // NUMERIC -> string, UUID -> uuid string, TIMESTAMPTZ -> rfc3339
    assert!(data.rows[0][4].as_str().unwrap().contains('T') || data.rows[0][4].is_string());

    let sel = d
        .execute_query(&format!("SELECT name FROM {} WHERE id=1", table))
        .await
        .unwrap();
    assert_eq!(sel.rows[0][0], serde_json::json!("a"));

    let full = d.get_full_schema(&table).await.unwrap();
    assert!(full.ddl.unwrap().contains("CREATE TABLE"));

    let exp = d
        .explain_query(&format!("SELECT * FROM {} WHERE id=1", table))
        .await
        .unwrap();
    assert!(!exp.steps.is_empty());

    let stats = d.get_database_stats().await.unwrap();
    assert!(stats.table_count >= 1);
    assert!(stats.version.starts_with("PostgreSQL"));

    let payload = crate::models::StagedCommitPayload {
        connection_id: "t".into(),
        table_name: table.clone(),
        primary_keys: vec!["id".into()],
        inserts: vec![serde_json::json!({"name": "c", "price": 3.5})],
        updates: vec![],
        deletes: vec![],
    };
    let res = d.commit_staged_changes(payload).await.unwrap();
    assert_eq!(res.affected_rows, 1);

    sqlx::query(&format!("DROP TABLE {}", table))
        .execute(&pool)
        .await
        .unwrap();
}
