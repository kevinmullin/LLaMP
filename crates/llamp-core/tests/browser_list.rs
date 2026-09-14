//! The browser lists the granted library. Rows are CoreText, not text.bmp.

use std::fs;
use std::path::Path;

use llamp_core::{browser_row_font, BrowserList, RowFont};
use llamp_library::Library;

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
fn browser_lists_the_granted_library_in_coretext() {
    let root = std::env::temp_dir().join(format!("llamp-browser-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let track = music.join("song.wav");
    wav(&track);
    let lib = Library::open(&root.join("library.sqlite")).expect("open");
    lib.grant_folder(&music).expect("grant");
    let mut browser = BrowserList::new();
    browser.load_granted(&lib).expect("load");
    assert_eq!(browser.rows(), [track.to_string_lossy().as_ref()]);
    assert_eq!(browser_row_font(&browser.rows()[0]), RowFont::CoreText);
    assert_eq!(browser_row_font("FIXTURE"), RowFont::CoreText);
    let _ = fs::remove_dir_all(&root);
}
