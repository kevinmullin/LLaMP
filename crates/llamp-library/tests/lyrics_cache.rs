//! 8d — opt-in, cache, sidecar offset. No network in this crate.

use std::fs;
use std::path::{Path, PathBuf};

use llamp_library::{
    parse_lrc, Library, LrclibConsent, LyricKind, LyricQuery, LyricSource, OffsetSave, Storage,
};

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("llamp-lyric-cache-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("dir");
    root
}

fn wav(path: &Path) {
    let frames = [0i16; 8];
    let data_bytes = (frames.len() * 2) as u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&8000u32.to_le_bytes());
    bytes.extend_from_slice(&(8000u32 * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in frames {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, bytes).expect("wav");
}

fn query() -> LyricQuery {
    LyricQuery {
        artist: "Artist".into(),
        title: "Title".into(),
        album: "Album".into(),
        duration_seconds: 200,
    }
}

#[test]
fn opt_in_starts_unknown_and_is_remembered() {
    let root = scratch("consent");
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    assert_eq!(lib.lyrics_consent().expect("read"), LrclibConsent::Unknown);
    lib.set_lyrics_consent(LrclibConsent::Decline).expect("decline");
    assert_eq!(lib.lyrics_consent().expect("declined"), LrclibConsent::Decline);
    lib.set_lyrics_consent(LrclibConsent::Accept).expect("accept");
    let lib = Library::open(&root.join("library.sqlite")).expect("reopen");
    assert_eq!(lib.lyrics_consent().expect("reopen"), LrclibConsent::Accept);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cache_round_trips_and_a_fresh_miss_is_stored() {
    let root = scratch("cache");
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let path = music.join("song.wav");
    wav(&path);
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    lib.grant_folder(&music).expect("grant");
    let q = query();
    lib.cache_lyrics(
        Some(&path),
        &q,
        Some("[00:01.00]Cached\n"),
        LyricSource::Remote,
        Some(LyricKind::Synced),
    )
    .expect("store");
    let hit = lib.cached_lyrics(Some(&path), &q).expect("hit").expect("doc");
    assert_eq!(hit.source, LyricSource::Cache);
    assert_eq!(hit.lines[0].text, "Cached");

    lib.cache_lyrics(None, &q, None, LyricSource::Remote, None)
        .expect("miss");
    assert!(lib.cached_miss_fresh(None, &q).expect("fresh miss"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn saving_offset_writes_the_sidecar_when_the_directory_is_writable() {
    let root = scratch("offset");
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let path = music.join("song.wav");
    wav(&path);
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    lib.grant_folder(&music).expect("grant");
    let doc = parse_lrc("[00:01.00]Hi\n").expect("doc");
    match lib.save_offset(&path, &doc, 200, None).expect("save") {
        OffsetSave::Sidecar(sidecar) => {
            let text = fs::read_to_string(&sidecar).expect("read");
            assert!(text.contains("[offset:+200]"), "{text}");
            let again = parse_lrc(&text).expect("round-trip");
            assert_eq!(again.lines[0].start_ms, 1200);
        }
        OffsetSave::Sqlite => panic!("writable music dir must get a sidecar"),
    }
    assert_eq!(
        lib.storage_of(&path).expect("storage").as_deref(),
        Some(Storage::Referenced.as_str())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn saving_offset_uses_sqlite_when_the_directory_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let root = scratch("offset-ro");
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let path = music.join("song.wav");
    wav(&path);
    let before = fs::read(&path).expect("wav");
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    lib.grant_folder(&music).expect("grant");
    let doc = parse_lrc("[00:01.00]Hi\n").expect("doc");
    let mut perm = fs::metadata(&music).expect("meta").permissions();
    perm.set_mode(0o555);
    fs::set_permissions(&music, perm).expect("ro");
    let result = lib.save_offset(&path, &doc, 200, None);
    let mut restore = fs::metadata(&music).expect("meta").permissions();
    restore.set_mode(0o755);
    fs::set_permissions(&music, restore).expect("rw");
    match result.expect("save") {
        OffsetSave::Sqlite => {}
        OffsetSave::Sidecar(sidecar) => panic!("unwritable dir must not write {sidecar:?}"),
    }
    assert_eq!(lib.stored_offset(&path).expect("row"), Some(200));
    assert!(!music.join("song.lrc").exists(), "no sidecar when the dir is not writable");
    assert_eq!(fs::read(&path).expect("same wav"), before, "audio must not be copied or rewritten");
    assert!(!root.join("song.wav").exists());
    assert!(!root.join("song.lrc").exists());
    assert_eq!(
        lib.storage_of(&path).expect("storage").as_deref(),
        Some(Storage::Referenced.as_str())
    );
    let _ = fs::remove_dir_all(&root);
}
