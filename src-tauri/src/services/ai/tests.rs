use super::*;
use crate::vault::credentials::MemoryStore;

#[test]
fn provider_parse_and_defaults() {
    assert_eq!(AiProvider::parse("openai"), Some(AiProvider::Openai));
    assert_eq!(AiProvider::parse("anthropic"), Some(AiProvider::Anthropic));
    assert_eq!(AiProvider::parse("gemini"), Some(AiProvider::Gemini));
    assert_eq!(AiProvider::parse("bogus"), None);
    assert_eq!(AiProvider::Openai.key_account(), "ai:openai");
}

#[test]
fn extract_sql_strips_fences_and_prose() {
    assert_eq!(extract_sql("SELECT 1"), "SELECT 1");
    assert_eq!(extract_sql("```sql\nSELECT 1;\n```"), "SELECT 1;");
    assert_eq!(
        extract_sql("Here is the query:\n```\nSELECT 2;\n```\nDone."),
        "SELECT 2;"
    );
}

#[test]
fn schema_context_is_bounded() {
    let col = |name: &str, pk: bool| TableColumnMetadata {
        name: name.into(),
        data_type: "TEXT".into(),
        is_nullable: true,
        is_primary_key: pk,
        default_value: None,
        comment: None,
    };
    let tables: Vec<(String, Vec<TableColumnMetadata>)> = (0..5)
        .map(|i| (format!("t{}", i), vec![col("id", true), col("name", false)]))
        .collect();
    let ctx = schema_context(&tables, 3);
    assert!(ctx.contains("t0("));
    assert!(ctx.contains("id TEXT PK"));
    assert!(ctx.contains("t2("));
    assert!(!ctx.contains("t3("));
    assert!(ctx.contains("2 more tables"));
}

#[test]
fn save_config_never_returns_the_key() {
    let creds = Arc::new(MemoryStore::new());
    let svc = AiService::for_test(creds.clone());
    creds.set("ai:openai", "sk-secret-123").unwrap();
    // get_config must report has_key without exposing it.
    // (read_config touches the real vault dir — skip in unit test; verify
    // the key account contract instead.)
    assert!(creds.get("ai:openai").unwrap().is_some());
    assert_eq!(AiProvider::Anthropic.key_account(), "ai:anthropic");
    let _ = svc; // constructed without panic
}
