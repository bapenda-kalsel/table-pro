use super::*;
use crate::errors::AppError;
use serde::Deserialize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Item {
    id: String,
    n: u32,
}

#[test]
fn missing_file_loads_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    let items: Vec<Item> = load_list(&dir.path().join("none.json")).unwrap();
    assert!(items.is_empty());
}

#[test]
fn corrupt_file_is_a_hard_error_not_empty() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.json");
    std::fs::write(&file, "{ not json").unwrap();

    let err = load_list::<Item>(&file).unwrap_err();
    match err {
        AppError::StorageCorrupt { path, .. } => assert!(path.ends_with("bad.json")),
        other => panic!("expected storage_corrupt, got {:?}", other),
    }
}

#[test]
fn save_does_not_destroy_corrupt_file_on_load_failure() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.json");
    std::fs::write(&file, "{ not json").unwrap();

    let res = update_list::<Item, _>(&file, |items| {
        items.push(Item {
            id: "x".into(),
            n: 1,
        });
        Ok(())
    });
    assert!(res.is_err());
    // Original (corrupt) content is untouched — user can recover it.
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{ not json");
}

#[test]
fn update_list_roundtrips_and_dedupes_by_caller_logic() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("items.json");

    for i in 0..3 {
        update_list::<Item, _>(&file, |items| {
            items.push(Item {
                id: format!("i{}", i),
                n: i as u32,
            });
            Ok(())
        })
        .unwrap();
    }
    let items: Vec<Item> = load_list(&file).unwrap();
    assert_eq!(items.len(), 3);

    // Replace-in-place by id.
    update_list::<Item, _>(&file, |items| {
        let pos = items.iter().position(|i| i.id == "i1").unwrap();
        items[pos].n = 99;
        Ok(())
    })
    .unwrap();
    let items: Vec<Item> = load_list(&file).unwrap();
    assert_eq!(items[1].n, 99);
}

#[cfg(unix)]
#[test]
fn written_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("secret.json");
    write_list_atomic(
        &file,
        &[Item {
            id: "a".into(),
            n: 1,
        }],
    )
    .unwrap();
    let mode = std::fs::metadata(&file).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}
