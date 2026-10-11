use super::*;
use crate::drivers::sqlite::SqliteDriver;
use crate::models::EnvironmentTag;
use crate::services::connections::LiveConnection;

#[test]
fn read_only_batches_are_read() {
    let cases = [
        "SELECT * FROM users WHERE id = 1",
        "select 1",
        "SELECT a.*, b.x FROM a JOIN b ON a.id=b.id LIMIT 10",
        "WITH x AS (SELECT 1 AS n) SELECT * FROM x",
        "SHOW TABLES",
        "EXPLAIN SELECT * FROM t",
        "SELECT COUNT(*) FROM a; SELECT MAX(id) FROM b",
    ];
    for sql in cases {
        for eng in [
            DatabaseDriverType::SQLite,
            DatabaseDriverType::MySQL,
            DatabaseDriverType::PostgreSQL,
        ] {
            assert_eq!(
                analyze_sql(sql, &eng),
                SqlRisk::Read,
                "expected Read for {:?} on {:?}",
                sql,
                eng
            );
        }
    }
}

#[test]
fn writes_and_unparseable_are_not_read() {
    let cases = [
        "INSERT INTO t VALUES (1)",
        "UPDATE t SET x = 1",
        "DELETE FROM t",
        "CREATE TABLE t (id INT)",
        "DROP TABLE t",
        "ALTER TABLE t ADD COLUMN x INT",
        "TRUNCATE TABLE t",
        "SELECT * FROM t; DELETE FROM t",
        "EXPLAIN ANALYZE SELECT * FROM t",
        "PRAGMA journal_mode=WAL",
        "GRANT ALL ON t TO u",
        "VACUUM",
    ];
    for sql in cases {
        for eng in [
            DatabaseDriverType::SQLite,
            DatabaseDriverType::MySQL,
            DatabaseDriverType::PostgreSQL,
        ] {
            let r = analyze_sql(sql, &eng);
            assert_ne!(
                r,
                SqlRisk::Read,
                "expected non-Read for {:?} on {:?}",
                sql,
                eng
            );
        }
    }
}

#[test]
fn unparseable_or_dialect_specific_is_unknown() {
    assert_eq!(
        analyze_sql("this is not sql at all", &DatabaseDriverType::SQLite),
        SqlRisk::Unknown
    );
    assert_eq!(
        analyze_sql(
            "SELECT * FROM t INTO OUTFILE '/tmp/x'",
            &DatabaseDriverType::SQLite
        ),
        SqlRisk::Unknown
    );
}

async fn prod_conn(generation: u64) -> LiveConnection {
    // Driver never touched in these tests.
    let drv = SqliteDriver::new(":memory:")
        .await
        .expect("in-memory sqlite");
    LiveConnection::new(
        "c1".into(),
        Box::new(drv),
        DatabaseDriverType::SQLite,
        "Prod".into(),
        EnvironmentTag::Production,
        generation,
        None,
    )
}

#[tokio::test]
async fn approval_is_one_shot_and_bound() {
    let store = ApprovalStore::new();
    let conn = prod_conn(7).await;

    // First call on production → challenge.
    let err = store
        .require(&conn, "sig-x", None, "do it?".to_string())
        .unwrap_err();
    let token = match err {
        AppError::ConfirmationRequired { challenge } => challenge.token,
        other => panic!("expected ConfirmationRequired, got {:?}", other),
    };

    // Wrong request signature with the valid token → invalid.
    assert!(store
        .require(&conn, "sig-y", Some(&token), "x".into())
        .is_err());

    // Correct replay → Ok, once.
    store
        .require(&conn, "sig-x", Some(&token), "x".into())
        .unwrap();

    // Same token cannot be replayed again.
    assert!(store
        .require(&conn, "sig-x", Some(&token), "x".into())
        .is_err());

    // Reconnect (new generation) invalidates outstanding tokens.
    let conn2 = prod_conn(8).await;
    let err2 = store
        .require(&conn2, "sig-x", None, "y".into())
        .unwrap_err();
    let token2 = match err2 {
        AppError::ConfirmationRequired { challenge } => challenge.token,
        _ => panic!(),
    };
    let conn3 = prod_conn(9).await;
    assert!(store
        .require(&conn3, "sig-x", Some(&token2), "z".into())
        .is_err());
}

#[tokio::test]
async fn non_production_is_noop() {
    let store = ApprovalStore::new();
    let mut conn = prod_conn(1).await;
    conn.environment = crate::models::EnvironmentTag::Local;
    store.require(&conn, "sig", None, "s".into()).unwrap();
}
