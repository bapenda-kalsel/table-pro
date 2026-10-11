use super::*;
use crate::models::EnvironmentTag;

fn tcp_profile(driver: DatabaseDriverType) -> ConnectionProfile {
    ConnectionProfile {
        id: "t".into(),
        name: "t".into(),
        driver,
        environment: EnvironmentTag::Local,
        host: Some("db".into()),
        port: None,
        database: Some("app".into()),
        username: Some("u".into()),
        file_path: None,
        ssl_mode: Some("verify_full".into()),
        ssl_ca_cert_path: None,
        ssh: None,
        created_at: None,
    }
}

#[test]
fn tls_mode_parsing_accepts_aliases_and_rejects_garbage() {
    assert_eq!(TlsMode::parse("verify_full").unwrap(), TlsMode::VerifyFull);
    assert_eq!(TlsMode::parse("verify-full").unwrap(), TlsMode::VerifyFull);
    assert_eq!(
        TlsMode::parse("VerifyIdentity").unwrap(),
        TlsMode::VerifyFull
    );
    assert_eq!(TlsMode::parse("verify_ca").unwrap(), TlsMode::VerifyCa);
    assert_eq!(TlsMode::parse("require").unwrap(), TlsMode::Required);
    assert_eq!(TlsMode::parse("prefer").unwrap(), TlsMode::Prefer);
    assert_eq!(TlsMode::parse("disable").unwrap(), TlsMode::Disabled);
    assert!(matches!(
        TlsMode::parse("bogus").unwrap_err(),
        AppError::TlsConfig { .. }
    ));
}

#[test]
fn legacy_tcp_profile_without_ssl_mode_requires_explicit_choice() {
    let mut p = tcp_profile(DatabaseDriverType::MySQL);
    p.ssl_mode = None;
    let err = validate(&p).unwrap_err();
    assert!(matches!(err, AppError::TlsConfig { .. }));

    // Empty string is the same "never configured" state.
    p.ssl_mode = Some("".into());
    assert!(matches!(
        validate(&p).unwrap_err(),
        AppError::TlsConfig { .. }
    ));

    p.ssl_mode = Some("verify_full".into());
    validate(&p).unwrap();
}

#[test]
fn sqlite_profile_needs_only_a_file_path() {
    let mut p = tcp_profile(DatabaseDriverType::SQLite);
    p.file_path = Some(":memory:".into());
    p.ssl_mode = None; // TLS is meaningless for SQLite — not required.
    validate(&p).unwrap();

    p.file_path = Some("".into());
    assert!(matches!(
        validate(&p).unwrap_err(),
        AppError::Validation { .. }
    ));
}

#[test]
fn tcp_profile_requires_database_and_username() {
    let mut p = tcp_profile(DatabaseDriverType::PostgreSQL);
    p.database = None;
    assert!(matches!(
        validate(&p).unwrap_err(),
        AppError::Validation { .. }
    ));
    p.database = Some("d".into());
    p.username = Some(" ".into());
    assert!(matches!(
        validate(&p).unwrap_err(),
        AppError::Validation { .. }
    ));
}
