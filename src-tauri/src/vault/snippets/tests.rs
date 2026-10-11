use super::super::json_file;
use super::*;

fn snippet(id: &str, name: &str, sql: &str) -> Snippet {
    Snippet {
        id: id.to_string(),
        name: name.to_string(),
        sql: sql.to_string(),
        created_at: "2026-10-08".to_string(),
    }
}

fn save_to(file: &std::path::Path, s: Snippet) {
    json_file::update_list(file, move |items: &mut Vec<Snippet>| {
        if let Some(pos) = items.iter().position(|x| x.id == s.id) {
            items[pos] = s.clone();
        } else {
            items.push(s.clone());
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn snippets_crud() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("snippets.json");

    save_to(&file, snippet("s1", "All users", "SELECT * FROM users;"));
    save_to(&file, snippet("s2", "Count", "SELECT COUNT(*) FROM users;"));

    let loaded: Vec<Snippet> = json_file::load_list(&file).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].name, "All users");

    // update same id replaces
    save_to(&file, snippet("s1", "All users", "SELECT id FROM users;"));
    let loaded: Vec<Snippet> = json_file::load_list(&file).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].sql, "SELECT id FROM users;");

    json_file::update_list::<Snippet, _>(&file, |items| {
        items.retain(|s| s.id != "s2");
        Ok(())
    })
    .unwrap();
    let loaded: Vec<Snippet> = json_file::load_list(&file).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, "s1");
}

#[test]
fn corrupt_snippets_file_is_an_error_not_empty() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("snippets.json");
    std::fs::write(&file, "[{broken").unwrap();

    let err = json_file::load_list::<Snippet>(&file).unwrap_err();
    assert!(matches!(err, crate::errors::AppError::StorageCorrupt { .. }));
}
