//! 8a — LRC, enhanced LRC, tags, sidecar. Malformed lines degrade.

use std::fs;
use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::id3::v2::{
    BinaryFrame, FrameId, Id3v2Tag, SyncTextContentType, SynchronizedTextFrame, TimestampFormat,
    UnsynchronizedTextFrame,
};
use lofty::tag::{ItemKey, Tag, TagExt, TagType};
use lofty::TextEncoding;
use llamp_library::{
    parse_lrc, read_embedded, resolve_lyrics, LyricKind, LyricSource, Storage, TrackTags,
};

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("llamp-lyrics-{name}-{}", std::process::id()));
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

#[test]
fn lrc_parses_line_timestamps_and_skips_id_tags() {
    let doc = parse_lrc(
        "[ar:Artist]\n[ti:Title]\n[al:Album]\n[length:02:00]\n[00:01.00]One\n[00:02]Two\n[01:00.123]Three\n",
    )
    .expect("parse");
    assert_eq!(doc.kind, LyricKind::Synced);
    assert_eq!(doc.tags.artist, "Artist");
    assert_eq!(doc.tags.title, "Title");
    assert_eq!(doc.tags.album, "Album");
    assert_eq!(
        doc.lines
            .iter()
            .map(|line| (line.start_ms, line.text.as_str()))
            .collect::<Vec<_>>(),
        [(1000, "One"), (2000, "Two"), (60123, "Three")]
    );
}

#[test]
fn enhanced_lrc_keeps_word_times() {
    let doc = parse_lrc("[00:10.00]Hello <00:10.40>there <00:10.80>friend\n").expect("parse");
    assert_eq!(doc.lines.len(), 1);
    let words: Vec<(u32, &str)> = doc.lines[0]
        .words
        .iter()
        .map(|word| (word.start_ms, word.text.as_str()))
        .collect();
    assert_eq!(words, [(10000, "Hello"), (10400, "there"), (10800, "friend")]);
}

#[test]
fn offset_applies_to_line_and_word_times() {
    let doc = parse_lrc("[offset:+200]\n[00:01.00]Hi <00:01.50>there\n").expect("parse");
    assert_eq!(doc.offset_ms, 200);
    assert_eq!(doc.lines[0].start_ms, 1200);
    assert_eq!(doc.lines[0].words[1].start_ms, 1700);
    let back = parse_lrc("[offset:-50]\n[00:01.00]Hi\n").expect("negative");
    assert_eq!(back.lines[0].start_ms, 950);
}

#[test]
fn two_timestamps_duplicate_the_line() {
    let doc = parse_lrc("[00:01.00][00:05.00]Repeat\n").expect("parse");
    assert_eq!(doc.lines.len(), 2);
    assert_eq!(doc.lines[0].start_ms, 1000);
    assert_eq!(doc.lines[1].start_ms, 5000);
    assert_eq!(doc.lines[0].text, "Repeat");
    assert_eq!(doc.lines[1].text, "Repeat");
}

#[test]
fn malformed_line_is_skipped_and_the_rest_stay() {
    let doc = parse_lrc("[00:01.00]Good\n[not-a-time]Nope\n[00:02.00]Also good\n[99:]\n")
        .expect("degrade");
    assert_eq!(
        doc.lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>(),
        ["Good", "Also good"]
    );
}

#[test]
fn no_timestamp_is_unsynced_plain_text() {
    let doc = parse_lrc("a poem\nwith two lines\n").expect("plain");
    assert_eq!(doc.kind, LyricKind::Unsynced);
    assert_eq!(doc.plain, "a poem\nwith two lines");
    assert!(parse_lrc("[ar:Only]\n").is_none(), "tags only is a miss");
}

#[test]
fn uslt_is_plain_text() {
    let root = scratch("uslt");
    let path = root.join("song.wav");
    wav(&path);
    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(ItemKey::UnsyncLyrics, "hello from uslt".into());
    tag.save_to_path(&path, WriteOptions::default()).expect("uslt");
    let doc = read_embedded(&path).expect("read");
    assert_eq!(doc.kind, LyricKind::Unsynced);
    assert_eq!(doc.plain, "hello from uslt");
    assert_eq!(doc.source, LyricSource::Embedded);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn sylt_wins_over_uslt() {
    let root = scratch("sylt");
    let path = root.join("song.wav");
    wav(&path);
    let sylt = SynchronizedTextFrame::new(
        TextEncoding::UTF8,
        *b"eng",
        TimestampFormat::MS,
        SyncTextContentType::Lyrics,
        None,
        vec![(0, "Hello".into()), (1500, "World".into())],
    );
    let bytes = sylt.as_bytes(WriteOptions::default()).expect("sylt bytes");
    let mut id3 = Id3v2Tag::new();
    id3.insert(
        UnsynchronizedTextFrame::new(TextEncoding::UTF8, *b"eng", String::new(), "ignore me")
            .into(),
    );
    id3.insert(
        BinaryFrame::new(FrameId::Valid("SYLT".into()), bytes).into(),
    );
    id3.save_to_path(&path, WriteOptions::default()).expect("save");
    let doc = read_embedded(&path).expect("sylt");
    assert_eq!(doc.kind, LyricKind::Synced);
    assert_eq!(doc.lines[0].text.trim(), "Hello");
    assert_eq!(doc.lines[1].start_ms, 1500);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn vorbis_lyrics_and_mp4_lyr_are_plain_text() {
    let vorbis = Tag::new(TagType::VorbisComments);
    let mut vorbis = vorbis;
    vorbis.insert_text(ItemKey::Lyrics, "vorbis lyrics".into());
    let from_vorbis = llamp_library::lyrics_from_tag(&vorbis, None).expect("vorbis");
    assert_eq!(from_vorbis.kind, LyricKind::Unsynced);
    assert_eq!(from_vorbis.plain, "vorbis lyrics");

    let mut mp4 = Tag::new(TagType::Mp4Ilst);
    mp4.insert_text(ItemKey::Lyrics, "mp4 lyr".into());
    let from_mp4 = llamp_library::lyrics_from_tag(&mp4, None).expect("mp4");
    assert_eq!(from_mp4.plain, "mp4 lyr");
}

#[test]
fn sidecar_synced_wins_over_embedded_unsynced() {
    let root = scratch("sidecar");
    let path = root.join("song.wav");
    wav(&path);
    let mut tag = Tag::new(TagType::Id3v2);
    tag.insert_text(ItemKey::UnsyncLyrics, "embedded plain".into());
    tag.save_to_path(&path, WriteOptions::default()).expect("uslt");
    fs::write(root.join("song.lrc"), "[00:01.00]From sidecar\n").expect("lrc");
    let doc = resolve_lyrics(&path, None).expect("resolve");
    assert_eq!(doc.source, LyricSource::Sidecar);
    assert_eq!(doc.kind, LyricKind::Synced);
    assert_eq!(doc.lines[0].text, "From sidecar");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn txt_without_a_timestamp_is_not_a_sidecar() {
    let root = scratch("txt");
    let path = root.join("song.wav");
    wav(&path);
    fs::write(root.join("song.txt"), "just a poem\nno times\n").expect("txt");
    assert!(resolve_lyrics(&path, None).is_none());
    fs::write(root.join("song.txt"), "[00:01.00]Looks like LRC\n").expect("lrc-txt");
    let doc = resolve_lyrics(&path, None).expect("txt lrc");
    assert_eq!(doc.source, LyricSource::Sidecar);
    assert_eq!(doc.lines[0].text, "Looks like LRC");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn resolve_does_not_fail_the_track_when_sidecar_is_garbage() {
    let root = scratch("garbage");
    let path = root.join("song.wav");
    wav(&path);
    fs::write(root.join("song.lrc"), "[bad\n").expect("garbage");
    assert!(resolve_lyrics(&path, None).is_none());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_open_still_accepts_the_granted_file() {
    let root = scratch("grant");
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let path = music.join("song.wav");
    wav(&path);
    let lib = llamp_library::Library::open(&root.join("library.sqlite")).expect("open");
    lib.grant_folder(&music).expect("grant");
    lib.write_tags(
        &path,
        &TrackTags {
            title: "Title".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            genre: String::new(),
        },
    )
    .expect("tags");
    assert_eq!(
        lib.storage_of(&path).expect("storage").as_deref(),
        Some(Storage::Referenced.as_str())
    );
    let _ = fs::remove_dir_all(&root);
}
