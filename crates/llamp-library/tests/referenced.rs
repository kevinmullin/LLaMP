use std::fs;

use llamp_library::{Library, Storage};

#[test]
fn one_thousand_rows_are_storage_referenced() {
    let root = std::env::temp_dir().join(format!("llamp-lib-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let files = root.join("files");
    fs::create_dir_all(&files).expect("dir");
    for i in 0..1000 {
        fs::write(files.join(format!("track-{i:04}")), b"").expect("file");
    }
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    // Open applies 001 then 002. v1 is only a mid-migration fixture (migrate_v1.rs).
    assert_eq!(lib.user_version().expect("version"), 2);
    assert_eq!(lib.insert_referenced_dir(&files).expect("insert"), 1000);
    assert_eq!(lib.count_referenced().expect("count"), 1000);
    let sample = files.join("track-0000");
    assert_eq!(
        lib.storage_of(&sample).expect("storage").as_deref(),
        Some(Storage::Referenced.as_str())
    );
    lib.insert(&root.join("managed.wav"), Storage::Managed).expect("managed");
    assert_eq!(lib.count_referenced().expect("count after managed"), 1000);
    let _ = fs::remove_dir_all(&files);
    assert_eq!(lib.count_referenced().expect("count after delete"), 1000);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn bundled_sqlite_has_fts5() {
    let root = std::env::temp_dir().join(format!("llamp-lib-fts-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("dir");
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    let _ = lib;
    let conn = rusqlite::Connection::open(root.join("library.sqlite")).expect("conn");
    conn.execute_batch("CREATE VIRTUAL TABLE fts_probe USING fts5(x); DROP TABLE fts_probe;")
        .expect("FTS5 is compiled into the bundled SQLite");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn storage_rejects_a_third_value() {
    let root = std::env::temp_dir().join(format!("llamp-lib-bad-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("dir");
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    let err = lib
        .insert(&root.join("x"), Storage::Referenced)
        .and_then(|_| {
            // The CHECK is on the column. A raw third value must fail.
            // Re-open through the same file with rusqlite in this test crate is not needed:
            // insert() only writes the two legal strings. Prove the CHECK exists via SQL.
            Ok(())
        });
    assert!(err.is_ok());
    let conn = rusqlite::Connection::open(root.join("library.sqlite")).expect("conn");
    let failed = conn.execute(
        "INSERT INTO tracks (storage, path, missing) VALUES ('copied', 'p', 0)",
        [],
    );
    assert!(failed.is_err(), "tracks.storage allows only referenced and managed");
    let _ = fs::remove_dir_all(&root);
}
