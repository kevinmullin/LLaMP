//! A database already at user_version 1 must become v2 without losing rows.

use std::fs;

use rusqlite::Connection;

#[test]
fn v1_database_upgrades_without_losing_rows() {
    let root = std::env::temp_dir().join(format!("llamp-migrate-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("dir");
    let db = root.join("library.sqlite");
    {
        let conn = Connection::open(&db).expect("open v1");
        conn.execute_batch(include_str!("../migrations/001.sql"))
            .expect("v1 schema");
        conn.pragma_update(None, "user_version", 1).expect("version");
        conn.execute(
            "INSERT INTO tracks (storage, path, missing) VALUES ('referenced', '/music/a.wav', 0)",
            [],
        )
        .expect("row a");
        conn.execute(
            "INSERT INTO tracks (storage, path, missing) VALUES ('referenced', '/music/b.wav', 1)",
            [],
        )
        .expect("row b");
    }

    let lib = llamp_library::Library::open(&db).expect("upgrade");
    assert_eq!(lib.user_version().expect("version"), 3);
    assert_eq!(lib.count_referenced().expect("count"), 2);
    assert_eq!(
        lib.storage_of(std::path::Path::new("/music/a.wav"))
            .expect("a")
            .as_deref(),
        Some("referenced")
    );
    assert_eq!(lib.missing_of(std::path::Path::new("/music/b.wav")).expect("b"), Some(true));
    let _ = fs::remove_dir_all(&root);
}
