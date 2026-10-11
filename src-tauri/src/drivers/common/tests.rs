use super::*;
use crate::models::{RowUpdatePayload, StagedCommitPayload};
use serde_json::json;

#[test]
fn dialect_quoting() {
    assert_eq!(Dialect::Ansi.qi("users"), "\"users\"");
    assert_eq!(Dialect::Ansi.qi("a\"b"), "\"a\"\"b\"");
    assert_eq!(Dialect::Backtick.qi("users"), "`users`");
    assert_eq!(Dialect::Backtick.qi("a`b"), "`a``b`");
}

#[test]
fn param_styles() {
    assert_eq!(ParamStyle::Question.ph(3), "?");
    assert_eq!(ParamStyle::DollarN.ph(3), "$3");
}

#[test]
fn staged_sql_ansi_vs_backtick() {
    let payload = StagedCommitPayload {
        connection_id: "c".into(),
        table_name: "users".into(),
        primary_keys: vec!["id".into()],
        inserts: vec![json!({"name": "O'Brien"})],
        updates: vec![RowUpdatePayload {
            primary_key_values: json!({"id": 1}),
            updated_fields: json!({"name": "x"}),
        }],
        deletes: vec![json!({"id": 2})],
    };
    let ansi = preview_staged_sql(Dialect::Ansi, &payload);
    assert!(ansi.generated_sql[0].contains("INSERT INTO \"users\""));
    assert!(ansi.generated_sql[0].contains("'O''Brien'"));

    let bq = preview_staged_sql(Dialect::Backtick, &payload);
    assert!(bq.generated_sql[1].contains("UPDATE `users` SET `name` = 'x'"));
    assert!(bq.generated_sql[2].contains("DELETE FROM `users` WHERE `id` = 2"));
}

#[test]
fn table_data_query_builds_safely() {
    let q = table_data_query(
        Dialect::Backtick,
        "orders",
        50,
        100,
        Some("status = 'paid'"),
        Some("created_at"),
        Some("desc"),
    );
    assert_eq!(
        q,
        "SELECT * FROM `orders` WHERE status = 'paid' ORDER BY `created_at` DESC LIMIT 50 OFFSET 100"
    );
    // injection in sort_dir collapses to ASC
    let bad = table_data_query(Dialect::Ansi, "t", 10, 0, None, Some("x"), Some("ASC; DROP"));
    assert!(bad.contains("ORDER BY \"x\" ASC"));
}

#[test]
fn export_formats() {
    let headers = vec!["id".to_string(), "name".to_string()];
    let mut r1 = serde_json::Map::new();
    r1.insert("id".into(), json!(1));
    r1.insert("name".into(), json!("Al"));
    let mut r2 = serde_json::Map::new();
    r2.insert("id".into(), json!(2));
    r2.insert("name".into(), serde_json::Value::Null);
    let rows = vec![r1, r2];

    let csv = export_rows_bytes("csv", "t", None, &headers, &rows, Dialect::Ansi).unwrap();
    let csv_s = String::from_utf8(csv).unwrap();
    assert!(csv_s.starts_with("id,name"));
    assert!(csv_s.contains("2,"));

    let json_b = export_rows_bytes("json", "t", None, &headers, &rows, Dialect::Ansi).unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_slice(&json_b).unwrap();
    assert_eq!(arr.len(), 2);

    let sql_b = export_rows_bytes(
        "sql",
        "t",
        Some("CREATE TABLE t (id INT)"),
        &headers,
        &rows,
        Dialect::Backtick,
    )
    .unwrap();
    let dump = String::from_utf8(sql_b).unwrap();
    assert!(dump.contains("CREATE TABLE t (id INT);"));
    assert!(dump.contains("INSERT INTO `t` (`id`, `name`) VALUES (1, 'Al');"));

    let xlsx_b = export_rows_bytes("xlsx", "t", None, &headers, &rows, Dialect::Ansi).unwrap();
    assert_eq!(&xlsx_b[0..2], &[0x50, 0x4B]); // ZIP magic

    assert!(export_rows_bytes("xml", "t", None, &headers, &rows, Dialect::Ansi).is_err());
}

#[test]
fn import_ddl_and_insert() {
    let ddl = import_table_ddl(Dialect::Backtick, "income", &["a".into(), "b_1".into()]).unwrap();
    assert!(ddl.contains("CREATE TABLE IF NOT EXISTS `income` (`a` TEXT, `b_1` TEXT)"));
    assert!(import_table_ddl(Dialect::Ansi, "t", &["bad header!".into()]).is_err());
    assert!(import_table_ddl(Dialect::Ansi, "t", &[]).is_err());

    let ins = import_insert_sql(Dialect::Ansi, ParamStyle::DollarN, "t", &["a".into(), "b".into()]);
    assert!(ins.contains("VALUES ($1, $2)"));
    let ins2 = import_insert_sql(Dialect::Backtick, ParamStyle::Question, "t", &["a".into()]);
    assert!(ins2.contains("VALUES (?)"));
}

#[test]
fn qi_path_qualified_names() {
    assert_eq!(Dialect::Ansi.qi_path("public.users"), "\"public\".\"users\"");
    assert_eq!(Dialect::Backtick.qi_path("db.orders"), "`db`.`orders`");
    assert_eq!(Dialect::Ansi.qi_path("plain"), "\"plain\"");
}

#[test]
fn sort_dir_whitelist() {
    assert_eq!(sanitize_sort_dir(Some("desc")), "DESC");
    assert_eq!(sanitize_sort_dir(Some("ASC")), "ASC");
    assert_eq!(sanitize_sort_dir(Some("DESC; DROP TABLE x")), "ASC");
    assert_eq!(sanitize_sort_dir(None), "ASC");
    assert_eq!(sanitize_sort_dir(Some("  ")), "ASC");
}

#[test]
fn literal_escaping_edges() {
    assert_eq!(sql_literal(&serde_json::Value::Null), "NULL");
    assert_eq!(sql_literal(&json!("it's a 'test'")), "'it''s a ''test'''");
    assert_eq!(sql_literal(&json!(42)), "42");
    assert_eq!(sql_literal(&json!(1.5)), "1.5");
    assert_eq!(sql_literal(&json!(true)), "1");
    // arrays/objects serialize then quote
    assert_eq!(sql_literal(&json!({"a":1})), "'{\"a\":1}'");
}

#[test]
fn empty_insert_row_uses_default_values() {
    let payload = StagedCommitPayload {
        connection_id: "c".into(),
        table_name: "t".into(),
        primary_keys: vec!["id".into()],
        inserts: vec![json!({"id": "", "name": ""})], // all blank
        updates: vec![],
        deletes: vec![],
    };
    let diff = preview_staged_sql(Dialect::Ansi, &payload);
    assert_eq!(diff.generated_sql[0], "INSERT INTO \"t\" DEFAULT VALUES;");
}

#[test]
fn count_query_includes_filter() {
    assert_eq!(
        count_query(Dialect::Ansi, "t", Some("x > 1")),
        "SELECT COUNT(*) AS n FROM \"t\" WHERE x > 1"
    );
    assert_eq!(
        count_query(Dialect::Ansi, "t", None),
        "SELECT COUNT(*) AS n FROM \"t\""
    );
}

#[test]
fn add_fk_sql_generation() {
    let s = add_fk_sql(
        Dialect::Backtick,
        "fk_orders_user",
        "orders",
        "user_id",
        "users",
        "id",
        "cascade",
        "bogus-injection",
    );
    assert_eq!(
        s,
        "ALTER TABLE `orders` ADD CONSTRAINT `fk_orders_user` FOREIGN KEY (`user_id`) REFERENCES `users`(`id`) ON DELETE CASCADE ON UPDATE NO ACTION;"
    );

    let a = add_fk_sql(
        Dialect::Ansi,
        "fk_o_u",
        "orders",
        "user_id",
        "users",
        "id",
        "SET NULL",
        "SET DEFAULT",
    );
    assert!(a.contains("FOREIGN KEY (\"user_id\") REFERENCES \"users\"(\"id\")"));
    assert!(a.contains("ON DELETE SET NULL ON UPDATE SET DEFAULT"));
}

#[test]
fn dump_content_layout() {
    let mut row = serde_json::Map::new();
    row.insert("id".into(), json!(1));
    let dumps = vec![(
        "CREATE TABLE `t` (`id` INT)".to_string(),
        "t".to_string(),
        vec![row],
    )];
    let extras = vec!["CREATE TRIGGER trg AFTER INSERT ON t BEGIN END".to_string()];
    let s = dump_content(Dialect::Backtick, &dumps, &extras);
    assert!(s.contains("BEGIN TRANSACTION"));
    assert!(s.contains("CREATE TABLE `t` (`id` INT);"));
    assert!(s.contains("INSERT INTO `t` (`id`) VALUES (1);"));
    assert!(s.contains("CREATE TRIGGER"));
    assert!(s.ends_with("COMMIT;\n"));
}
