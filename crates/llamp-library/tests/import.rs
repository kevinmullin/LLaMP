//! Grant, search, tag round-trip, missing-on-delete, M3U8, artwork eviction.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use llamp_library::{Library, Storage, TrackTags};

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("llamp-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("dir");
    root
}

fn wav(path: &Path) {
    // Stereo PCM16, a few frames. lofty 0.25.2 writes ID3v2 onto this container.
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

#[test]
fn grant_does_not_copy_and_fts_finds_non_latin_names() {
    let root = scratch("grant");
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let cyr = music.join("boris.wav");
    let cjk = music.join("zhou.wav");
    wav(&cyr);
    wav(&cjk);
    let before = fs::read(&cyr).expect("bytes");

    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    assert_eq!(lib.grant_folder(&music).expect("grant"), 2);
    assert_eq!(fs::read(&cyr).expect("still there"), before, "grant must not copy or rewrite audio");
    lib.write_tags(
        &cyr,
        &TrackTags {
            title: "Track".into(),
            artist: "Борис".into(),
            album: "Album".into(),
            genre: "Genre".into(),
        },
    )
    .expect("cyr tags");
    lib.write_tags(
        &cjk,
        &TrackTags {
            title: "歌".into(),
            artist: "周杰伦".into(),
            album: "范特西".into(),
            genre: "流行".into(),
        },
    )
    .expect("cjk tags");

    let app_copy = root.join("Application Support").join("boris.wav");
    assert!(!app_copy.exists(), "music must not be copied under an app folder");
    assert_eq!(
        lib.storage_of(&cyr).expect("storage").as_deref(),
        Some(Storage::Referenced.as_str())
    );

    let found = lib.search("Борис").expect("cyr search");
    assert!(found.iter().any(|hit| hit.path == cyr), "unicode61 must find Борис, got {found:?}");
    let found = lib.search("周杰伦").expect("cjk search");
    assert!(found.iter().any(|hit| hit.path == cjk), "unicode61 must find 周杰伦, got {found:?}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn grant_search_enqueue_tag_survives_reopen_on_the_granted_file() {
    let root = scratch("enqueue");
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let path = music.join("song.wav");
    wav(&path);
    let before = fs::read(&path).expect("bytes");
    let db = root.join("library.sqlite");
    {
        let lib = Library::open(&db).expect("open");
        lib.grant_folder(&music).expect("grant");
        let hits = lib.search("song").expect("fts");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, path, "search must return the granted path");
        lib.enqueue(&hits[0].path).expect("enqueue");
        assert_eq!(lib.enqueued_paths().expect("queue"), vec![path.clone()]);
        assert_eq!(fs::read(&path).expect("still there"), before, "enqueue must not copy or rewrite audio");
        assert!(!root.join("Application Support").join("song.wav").exists());
        lib.write_tags(
            &path,
            &TrackTags {
                title: "Title".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                genre: "Genre".into(),
            },
        )
        .expect("write");
    }
    let lib = Library::open(&db).expect("relaunch");
    let tags = lib.read_tags(&path).expect("read back");
    assert_eq!(tags.title, "Title");
    assert_eq!(tags.artist, "Artist");
    assert_eq!(lib.enqueued_paths().expect("queue after relaunch"), vec![path.clone()]);
    assert!(path.is_file(), "the file on disk is the granted file, not a copy");
    assert_eq!(
        lib.storage_of(&path).expect("storage").as_deref(),
        Some(Storage::Referenced.as_str())
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn tag_write_survives_reopen_on_the_granted_file() {
    let root = scratch("tags");
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let path = music.join("song.wav");
    wav(&path);
    let db = root.join("library.sqlite");
    {
        let lib = Library::open(&db).expect("open");
        lib.grant_folder(&music).expect("grant");
        lib.write_tags(
            &path,
            &TrackTags {
                title: "Title".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                genre: "Genre".into(),
            },
        )
        .expect("write");
    }
    let lib = Library::open(&db).expect("reopen");
    let tags = lib.read_tags(&path).expect("read back");
    assert_eq!(tags.title, "Title");
    assert_eq!(tags.artist, "Artist");
    assert!(path.is_file(), "the tag must be on the granted file, not a copy");
    assert_eq!(
        lib.storage_of(&path).expect("storage").as_deref(),
        Some("referenced")
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn deleted_file_stays_missing_and_remove_does_not_delete_audio() {
    let root = scratch("missing");
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let stay = music.join("stay.wav");
    let gone = music.join("gone.wav");
    wav(&stay);
    wav(&gone);
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    lib.grant_folder(&music).expect("grant");
    fs::remove_file(&gone).expect("finder delete");
    lib.rescan_path(&gone).expect("incremental");
    assert_eq!(lib.missing_of(&gone).expect("missing"), Some(true));
    assert_eq!(lib.count_referenced().expect("count"), 2, "a missing file must not drop the row");
    lib.remove_from_library(&stay).expect("drop row");
    assert!(stay.is_file(), "remove from library must not delete the user's file");
    assert_eq!(lib.storage_of(&stay).expect("gone row"), None);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn watch_picks_up_a_new_file_without_rescanning_the_grant() {
    let root = scratch("watch");
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let first = music.join("first.wav");
    wav(&first);
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    lib.grant_folder(&music).expect("grant");
    let id_before = lib.track_id(&first).expect("id");
    let added = music.join("added.wav");
    wav(&added);
    let deadline = Instant::now() + Duration::from_secs(3);
    while lib.storage_of(&added).expect("poll").is_none() {
        if Instant::now() > deadline {
            panic!("watcher did not insert the new file");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(lib.track_id(&first).expect("same id"), id_before, "a new file must not rewrite the existing row");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn m3u8_round_trip_keeps_order_and_paths_as_written() {
    let root = scratch("m3u8");
    let list = root.join("list.m3u8");
    fs::write(
        &list,
        "#EXTM3U\n#EXTINF:1,First\n/abs/one.mp3\nrel/two.mp3\n",
    )
    .expect("playlist");
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    lib.import_m3u8(&list).expect("import");
    let exported = root.join("out.m3u8");
    lib.export_m3u8(&exported).expect("export");
    let text = fs::read_to_string(&exported).expect("read");
    let paths: Vec<&str> = text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(paths, ["/abs/one.mp3", "rel/two.mp3"]);
    assert!(!text.contains("Application Support"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn artwork_eviction_deletes_the_cache_file_not_the_audio() {
    let root = scratch("art");
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let audio = music.join("song.wav");
    wav(&audio);
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    lib.grant_folder(&music).expect("grant");
    lib.set_artwork_cap(100);
    let first = lib.store_artwork(&audio, &vec![1u8; 80]).expect("first");
    let other = music.join("other.wav");
    wav(&other);
    lib.insert(&other, Storage::Referenced).expect("other");
    let second = lib.store_artwork(&other, &vec![2u8; 80]).expect("second");
    assert!(!first.exists(), "lru must delete the cache file");
    assert!(second.exists(), "newer cache file stays");
    assert!(audio.is_file(), "eviction must not delete audio");
    let _ = fs::remove_dir_all(&root);
}
