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

    let loaded = load_profiles_from(&file);
    assert_eq!(loaded.len(), 2);

    // save same id replaces instead of duplicating
    save_profile_to(&file, profile("c1", "Renamed")).unwrap();
    let loaded = load_profiles_from(&file);
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].name, "Renamed");
}
