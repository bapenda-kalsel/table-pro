use super::super::json_file;
use super::*;
use crate::models::{DatabaseDriverType, EnvironmentTag};

fn profile(id: &str, name: &str) -> ConnectionProfile {
    ConnectionProfile {
        id: id.to_string(),
        name: name.to_string(),
        driver: DatabaseDriverType::SQLite,
        environment: EnvironmentTag::Local,
        host: None,
        port: None,
        database: None,
        username: None,
        file_path: Some(":memory:".to_string()),
        ssl_mode: None,
        ssl_ca_cert_path: None,
        ssh: None,
        created_at: None,
    }
}

#[test]
fn profiles_save_load_delete() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("connections.json");
    let p = profile("c1", "Test");

    save_profile_to(&file, p.clone()).unwrap();
    save_profile_to(&file, profile("c2", "Second")).unwrap();

    let loaded = load_profiles_from(&file).unwrap();
    assert_eq!(loaded.len(), 2);

    // save same id replaces instead of duplicating
    save_profile_to(&file, profile("c1", "Renamed")).unwrap();
    let loaded = load_profiles_from(&file).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].name, "Renamed");

    delete_profile_to(&file, "c2").unwrap();
    let loaded = load_profiles_from(&file).unwrap();
    assert_eq!(loaded.len(), 1);
}

fn delete_profile_to(path: &std::path::Path, id: &str) -> AppResult<()> {
    json_file::update_list(path, |items: &mut Vec<ConnectionProfile>| {
        items.retain(|p| p.id != id);
        Ok(())
    })
}

#[test]
fn corrupt_profiles_file_fails_loudly() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("connections.json");
    std::fs::write(&file, "[{broken").unwrap();

    let err = load_profiles_from(&file).unwrap_err();
    assert!(matches!(
        err,
        crate::errors::AppError::StorageCorrupt { .. }
    ));

    // A failed load must never become an empty list that a save overwrites.
    let res = save_profile_to(&file, profile("c1", "New"));
    assert!(res.is_err());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "[{broken");
}
