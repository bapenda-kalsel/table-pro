use super::detect_statement_type;

#[test]
fn detects_basic_verbs() {
    assert_eq!(detect_statement_type("SELECT * FROM t"), "SELECT");
    assert_eq!(detect_statement_type("  insert into t values (1)"), "INSERT");
    assert_eq!(detect_statement_type("UPDATE t SET x=1"), "UPDATE");
    assert_eq!(detect_statement_type("delete from t"), "DELETE");
    assert_eq!(detect_statement_type("pragma table_info(t)"), "PRAGMA");
}

#[test]
fn skips_comments() {
    assert_eq!(
        detect_statement_type("-- a comment\nSELECT 1"),
        "SELECT"
    );
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
    assert_eq!(detect_statement_type("WITH x AS (SELECT 1) SELECT * FROM x"), "WITH");
}
