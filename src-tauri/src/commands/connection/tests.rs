use super::*;
use crate::models::EnvironmentTag;

fn profile(driver: DatabaseDriverType) -> ConnectionProfile {
    ConnectionProfile {
        id: "test-conn".into(),
        name: "Test".into(),
        driver,
        environment: EnvironmentTag::Local,
        host: Some("127.0.0.1".into()),
        port: Some(1), // nothing listens -> only validation paths are asserted
        database: Some("db".into()),
        username: Some("u".into()),
        file_path: Some(":memory:".into()),
        ssl_mode: None,
        created_at: None,
    }
}

#[tokio::test]
async fn build_driver_sqlite_memory_works() {
    let d = build_driver(&profile(DatabaseDriverType::SQLite), None)
        .await
        .unwrap();
    assert!(d.ping().await.unwrap() >= 0.0);
}

#[tokio::test]
async fn build_driver_mysql_requires_database_and_user() {
    let mut p = profile(DatabaseDriverType::MySQL);
    p.database = None;
    let err = build_driver(&p, None).await.err().unwrap();
    assert!(err.contains("database"));

    let mut p2 = profile(DatabaseDriverType::MySQL);
    p2.username = Some("".into());
    let err2 = build_driver(&p2, None).await.err().unwrap();
    assert!(err2.contains("username"));
}

#[tokio::test]
async fn build_driver_postgres_requires_database() {
    let mut p = profile(DatabaseDriverType::PostgreSQL);
    p.database = Some("".into());
    let err = build_driver(&p, None).await.err().unwrap();
    assert!(err.contains("database"));
}

#[tokio::test]
async fn build_driver_mysql_default_host_and_port() {
    // With all required fields set, validation passes and the failure is a
    // connection error (nothing listening), not a validation error.
    let mut p = profile(DatabaseDriverType::MySQL);
    p.host = None;
    p.port = None;
    let err = build_driver(&p, None).await.err().unwrap();
    assert!(err.contains("MySQL connection failed"));
}
