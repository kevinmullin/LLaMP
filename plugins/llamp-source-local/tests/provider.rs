//! The local library is a MediaSource. Browse, search, and resolve go through the trait.

use std::fs;
use std::path::Path;

use llamp_plugin_api::{MediaSource, SourceFlags};
use llamp_source_local::LocalSource;

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
fn browse_search_resolve_are_the_trait() {
    let root = std::env::temp_dir().join(format!("llamp-local-src-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let track = music.join("song.wav");
    wav(&track);
    let source = LocalSource::open(&root.join("library.sqlite")).expect("open");
    source.grant_folder(&music).expect("grant");
    let as_source: &dyn MediaSource = &source;
    let browsed = as_source.browse().expect("browse");
    assert_eq!(browsed.len(), 1);
    assert_eq!(browsed[0].label, track.to_string_lossy());
    let hits = as_source.search("song").expect("search");
    assert_eq!(hits.len(), 1);
    let resolved = as_source.resolve(&hits[0].id).expect("resolve");
    assert_eq!(resolved.flags, SourceFlags::local_file());
    assert!(resolved.flags.produces_pcm);
    assert!(resolved.flags.supports_eq);
    let _ = fs::remove_dir_all(&root);
}
