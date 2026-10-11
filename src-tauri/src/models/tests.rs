use super::detect_statement_type;
use super::*;

#[test]
fn detects_basic_verbs() {
    assert_eq!(detect_statement_type("SELECT * FROM t"), "SELECT");
    assert_eq!(
        detect_statement_type("  insert into t values (1)"),
        "INSERT"
    );
    assert_eq!(detect_statement_type("UPDATE t SET x=1"), "UPDATE");
    assert_eq!(detect_statement_type("delete from t"), "DELETE");
    assert_eq!(detect_statement_type("pragma table_info(t)"), "PRAGMA");
}

#[test]
fn skips_comments() {
    assert_eq!(detect_statement_type("-- a comment\nSELECT 1"), "SELECT");
    assert_eq!(
        detect_statement_type("/* block */ UPDATE t SET x=1"),
        "UPDATE"
    );
    assert_eq!(
        detect_statement_type("/* multi\nline */\n\n  DROP TABLE t"),
        "DROP"
    );
}

#[test]
fn handles_edge_cases() {
    assert_eq!(detect_statement_type(""), "OTHER");
    assert_eq!(detect_statement_type("   "), "OTHER");
    assert_eq!(detect_statement_type("-- only a comment"), "OTHER");
    assert_eq!(
        detect_statement_type("WITH x AS (SELECT 1) SELECT * FROM x"),
        "WITH"
    );
}

// ---- Serde contract tests: the TS mirror in src/lib/types.ts depends on
// these exact wire shapes. If a test here breaks, update both sides.

#[test]
fn connection_profile_roundtrips_and_tolerates_missing_new_fields() {
    // Legacy file written before ssl_mode/created_at existed.
    let legacy = r#"{
        "id": "c1", "name": "Prod", "driver": "postgres",
        "environment": "production", "host": "db.internal", "port": 5432,
        "database": "app", "username": "svc", "file_path": null
    }"#;
    let p: ConnectionProfile = serde_json::from_str(legacy).unwrap();
    assert_eq!(p.driver, DatabaseDriverType::PostgreSQL);
    assert_eq!(p.environment, EnvironmentTag::Production);
    assert!(p.ssl_mode.is_none());
    assert!(p.created_at.is_none());

    // Full roundtrip is stable.
    let json = serde_json::to_string(&p).unwrap();
    let p2: ConnectionProfile = serde_json::from_str(&json).unwrap();
    assert_eq!(p2.id, "c1");
}

#[test]
fn driver_and_environment_serialize_to_lowercase_wire_values() {
    assert_eq!(
        serde_json::to_string(&DatabaseDriverType::SQLite).unwrap(),
        "\"sqlite\""
    );
    assert_eq!(
        serde_json::to_string(&DatabaseDriverType::MySQL).unwrap(),
        "\"mysql\""
    );
    assert_eq!(
        serde_json::to_string(&DatabaseDriverType::PostgreSQL).unwrap(),
        "\"postgres\""
    );
    assert_eq!(
        serde_json::to_string(&EnvironmentTag::Staging).unwrap(),
        "\"staging\""
    );
}

#[test]
fn sql_log_entry_defaults_missing_enrichment_fields() {
    // Entries written before connection_name/environment existed must parse.
    let old = r#"{
        "id": "1", "timestamp": "2026-01-01 00:00:00.000",
        "query": "SELECT 1", "duration_ms": 1.0, "is_error": false,
        "connection_id": null
    }"#;
    let e: SqlLogEntry = serde_json::from_str(old).unwrap();
    assert!(e.connection_name.is_none());
    assert!(e.environment.is_none());
    assert!(e.statement_type.is_none());
    assert!(e.affected_rows.is_none());
}

#[test]
fn staged_payload_wire_shape_uses_snake_case() {
    let payload = StagedCommitPayload {
        connection_id: "c".into(),
        table_name: "users".into(),
        primary_keys: vec!["id".into()],
        inserts: vec![serde_json::json!({"name": "x"})],
        updates: vec![RowUpdatePayload {
            primary_key_values: serde_json::json!({"id": 1}),
            updated_fields: serde_json::json!({"name": "y"}),
        }],
        deletes: vec![serde_json::json!({"id": 2})],
    };
    let v = serde_json::to_value(&payload).unwrap();
    for key in [
        "connection_id",
        "table_name",
        "primary_keys",
        "inserts",
        "updates",
        "deletes",
    ] {
        assert!(v.get(key).is_some(), "missing wire key {key}");
    }
    assert!(v["updates"][0].get("primary_key_values").is_some());
    assert!(v["updates"][0].get("updated_fields").is_some());

    let back: StagedCommitPayload = serde_json::from_value(v).unwrap();
    assert_eq!(back.primary_keys, vec!["id"]);
}

#[test]
fn table_data_query_and_fk_spec_wire_shape() {
    let q: TableDataQuery = serde_json::from_str(
        r#"{"table":"t","limit":10,"offset":0,"filter":null,"sort_by":"id","sort_dir":"desc"}"#,
    )
    .unwrap();
    assert_eq!(q.sort_dir.as_deref(), Some("desc"));

    let spec: ForeignKeySpec = serde_json::from_str(
        r#"{"constraint_name":"fk","table":"a","column":"b","ref_table":"c","ref_column":"d","on_delete":"CASCADE","on_update":"NO ACTION"}"#,
    )
    .unwrap();
    assert_eq!(spec.ref_table, "c");
}
