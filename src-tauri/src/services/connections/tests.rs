use crate::models::{DatabaseDriverType, EnvironmentTag};
use crate::services::connections::ConnectionService;
use crate::vault::credentials::{CredentialStore, MemoryStore};
use std::sync::Arc;

fn sqlite_profile(id: &str, path: &str) -> crate::models::ConnectionProfile {
    crate::models::ConnectionProfile {
        id: id.into(),
        name: format!("Conn {}", id),
        driver: DatabaseDriverType::SQLite,
        environment: EnvironmentTag::Local,
        host: None,
        port: None,
        database: None,
        username: None,
        file_path: Some(path.into()),
        ssl_mode: None,
        ssl_ca_cert_path: None,
        ssh: None,
        created_at: None,
    }
}

fn test_service(dir: &tempfile::TempDir) -> ConnectionService {
    ConnectionService::for_test(
        dir.path().join("connections.json"),
        Arc::new(MemoryStore::new()),
    )
}

#[tokio::test]
async fn connect_sqlite_memory_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let svc = test_service(&dir);
    svc.save_profile(
        sqlite_profile("c1", ":memory:"),
        crate::vault::credentials::SecretIntent::Keep,
    )
    .await
    .unwrap();

    let ms = svc.connect("c1", None).await.unwrap();
    assert!(ms >= 0.0);

    let guard = svc.acquire("c1").await.unwrap();
    let tables = guard.conn.driver.list_tables().await.unwrap();
    assert!(tables.is_empty());
    assert_eq!(guard.conn.connection_name, "Conn c1");
    drop(guard);

    svc.disconnect("c1").await.unwrap();
    // Second disconnect is a no-op.
    svc.disconnect("c1").await.unwrap();
}

#[tokio::test]
async fn acquire_rejects_disconnected_and_closing_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let svc = test_service(&dir);
    svc.save_profile(
        sqlite_profile("c1", ":memory:"),
        crate::vault::credentials::SecretIntent::Keep,
    )
    .await
    .unwrap();

    let err = svc.acquire("c1").await.unwrap_err();
    assert!(matches!(err, crate::errors::AppError::NotConnected { .. }));

    svc.connect("c1", None).await.unwrap();
    svc.disconnect("c1").await.unwrap();
    let err = svc.acquire("c1").await.unwrap_err();
    assert!(matches!(err, crate::errors::AppError::NotConnected { .. }));
}

#[tokio::test]
async fn disconnect_drains_in_flight_operation_before_closing() {
    let dir = tempfile::tempdir().unwrap();
    let svc = test_service(&dir);
    svc.save_profile(
        sqlite_profile("c1", ":memory:"),
        crate::vault::credentials::SecretIntent::Keep,
    )
    .await
    .unwrap();
    svc.connect("c1", None).await.unwrap();

    // Hold a lease, start disconnect on another task — it must wait for
    // the lease to be dropped before completing.
    let guard = svc.acquire("c1").await.unwrap();
    let svc2 = ConnectionService::for_test(
        dir.path().join("connections.json"),
        Arc::new(MemoryStore::new()),
    );
    // NOTE: separate service = separate registry; drain test needs the
    // same service, so simulate via registry clone instead.
    let _ = svc2;
    let disconnect = tokio::spawn({
        let svc = svc;
        async move { svc.disconnect("c1").await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!disconnect.is_finished(), "disconnect must wait for ops");
    drop(guard);
    disconnect.await.unwrap().unwrap();
}

#[tokio::test]
async fn unknown_profile_and_tcp_without_tls_fail_typed() {
    let dir = tempfile::tempdir().unwrap();
    let svc = test_service(&dir);

    let err = svc.connect("nope", None).await.unwrap_err();
    assert!(matches!(err, crate::errors::AppError::NotFound { .. }));

    // TCP profile without ssl_mode => tls_config error at validation.
    let mut p = sqlite_profile("c1", ":memory:");
    p.driver = DatabaseDriverType::MySQL;
    p.database = Some("d".into());
    p.username = Some("u".into());
    let err = svc
        .save_profile(p, crate::vault::credentials::SecretIntent::Keep)
        .await
        .unwrap_err();
    assert!(matches!(err, crate::errors::AppError::TlsConfig { .. }));
}

#[tokio::test]
async fn delete_active_profile_is_busy_not_orphaned() {
    let dir = tempfile::tempdir().unwrap();
    let svc = test_service(&dir);
    svc.save_profile(
        sqlite_profile("c1", ":memory:"),
        crate::vault::credentials::SecretIntent::Keep,
    )
    .await
    .unwrap();
    svc.connect("c1", None).await.unwrap();

    let err = svc.delete_profile("c1").await.unwrap_err();
    assert!(matches!(err, crate::errors::AppError::Busy { .. }));

    svc.disconnect("c1").await.unwrap();
    svc.delete_profile("c1").await.unwrap();
}

#[tokio::test]
async fn credential_intent_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let creds = Arc::new(MemoryStore::new());
    let svc = ConnectionService::for_test(dir.path().join("connections.json"), creds.clone());
    svc.save_profile(
        sqlite_profile("c1", ":memory:"),
        crate::vault::credentials::SecretIntent::Set("pw123".into()),
    )
    .await
    .unwrap();
    assert_eq!(creds.get("c1").unwrap(), Some("pw123".to_string()));

    creds.delete("c1").unwrap();
    assert_eq!(creds.get("c1").unwrap(), None);
}
