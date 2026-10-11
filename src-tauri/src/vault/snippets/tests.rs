use super::*;

fn snippet(id: &str, name: &str, sql: &str) -> Snippet {
    Snippet {
        id: id.to_string(),
        name: name.to_string(),
        sql: sql.to_string(),
        created_at: "2026-10-08".to_string(),
    }
}

#[test]
fn snippets_crud() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("snippets.json");

    save_snippet_to(&file, snippet("s1", "All users", "SELECT * FROM users;")).unwrap();
    save_snippet_to(&file, snippet("s2", "Count", "SELECT COUNT(*) FROM users;")).unwrap();

    let loaded = load_snippets_from(&file);
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].name, "All users");

    // update same id replaces
    save_snippet_to(&file, snippet("s1", "All users", "SELECT id FROM users;")).unwrap();
    let loaded = load_snippets_from(&file);
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].sql, "SELECT id FROM users;");

    delete_snippet_from(&file, "s2").unwrap();
    let loaded = load_snippets_from(&file);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, "s1");
}
