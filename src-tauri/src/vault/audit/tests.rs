use super::*;

fn entry(id: &str, query: &str, ms: f64, is_error: bool) -> SqlLogEntry {
    SqlLogEntry {
        id: id.to_string(),
        timestamp: "2026-10-08 10:00:00.000".to_string(),
        query: query.to_string(),
        duration_ms: ms,
        is_error,
        error_message: if is_error {
            Some("boom".to_string())
        } else {
            None
        },
        connection_id: Some("conn-1".to_string()),
        connection_name: Some("Test DB".to_string()),
        environment: Some("local".to_string()),
        statement_type: Some(crate::models::detect_statement_type(query)),
        affected_rows: None,
    }
}

#[test]
fn audit_append_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("audit.jsonl");

    append_audit_to(&file, &entry("1", "SELECT 1", 1.5, false)).unwrap();
    append_audit_to(&file, &entry("2", "UPDATE t SET x=1", 20.0, false)).unwrap();
    append_audit_to(&file, &entry("3", "SELECT bad", 0.5, true)).unwrap();

    let loaded = load_audit_entries_from(&file);
    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[1].query, "UPDATE t SET x=1");
    assert!(loaded[2].is_error);
    assert_eq!(loaded[0].statement_type.as_deref(), Some("SELECT"));
}

#[test]
fn audit_filters() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("audit.jsonl");
    append_audit_to(&file, &entry("1", "SELECT * FROM users", 1.0, false)).unwrap();
    append_audit_to(&file, &entry("2", "UPDATE users SET x=1", 2.0, false)).unwrap();
    append_audit_to(&file, &entry("3", "SELECT * FROM bad", 0.5, true)).unwrap();

    let loaded = load_audit_entries_from(&file);

    // errors only
    let errs = filter_audit_entries(loaded.clone(), None, None, None, true, None);
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].id, "3");

    // text filter matches query content (case-insensitive)
    let text = filter_audit_entries(loaded.clone(), None, None, Some("USERS"), false, None);
    assert_eq!(text.len(), 2);

    // statement type filter
    let sel = filter_audit_entries(loaded.clone(), None, None, None, false, Some("select"));
    assert_eq!(sel.len(), 2);

    // limit keeps the most recent N
    let limited = filter_audit_entries(loaded.clone(), Some(2), None, None, false, None);
    assert_eq!(limited.len(), 2);
    assert_eq!(limited[0].id, "2");
}

#[test]
fn audit_stats_computed() {
    let entries = vec![
        entry("1", "SELECT 1", 10.0, false),
        entry("2", "UPDATE t SET x=1", 30.0, false),
        entry("3", "DELETE FROM t", 20.0, true),
    ];
    let stats = audit_stats_of(&entries);
    assert_eq!(stats.total, 3);
    assert_eq!(stats.error_count, 1);
    assert!((stats.avg_duration_ms - 20.0).abs() < 0.001);
    assert_eq!(stats.max_duration_ms, 30.0);
    assert_eq!(stats.slowest_query.as_deref(), Some("UPDATE t SET x=1"));
    assert_eq!(stats.by_statement_type.get("SELECT"), Some(&1));
    assert_eq!(stats.by_statement_type.get("DELETE"), Some(&1));
}

#[test]
fn audit_serialize_csv_and_json() {
    let entries = vec![entry("1", "SELECT 'a,b'", 1.0, false)];
    let csv = serialize_audit(&entries, "csv").unwrap();
    assert!(csv.starts_with("id,timestamp,"));
    // the query contains a comma and quote -> must be quoted/escaped
    assert!(csv.contains("\"SELECT 'a,b'\""));

    let json = serialize_audit(&entries, "json").unwrap();
    let parsed: Vec<SqlLogEntry> = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, "1");
}
