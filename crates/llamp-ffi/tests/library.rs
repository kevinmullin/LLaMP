use std::ffi::{CStr, CString};
use std::fs;
use std::path::PathBuf;

fn wav(path: &std::path::Path) {
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
fn grant_search_enqueue_keeps_the_granted_path() {
    let root = std::env::temp_dir().join(format!("llamp-ffi-enq-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let music = root.join("music");
    fs::create_dir_all(&music).expect("music");
    let track = music.join("song.wav");
    wav(&track);
    let before = fs::read(&track).expect("bytes");
    let db = root.join("library.sqlite");
    let db_c = CString::new(db.to_string_lossy().as_ref()).expect("db");
    let dir_c = CString::new(music.to_string_lossy().as_ref()).expect("dir");
    let q = CString::new("song").expect("q");
    assert_eq!(
        llamp_ffi::llamp_library_open(db_c.as_ptr()),
        llamp_ffi::LLAMP_OK
    );
    assert_eq!(llamp_ffi::llamp_library_grant(dir_c.as_ptr()), 1);
    assert_eq!(llamp_ffi::llamp_library_search(q.as_ptr()), 1);
    let mut buf = [0i8; 512];
    assert_eq!(
        llamp_ffi::llamp_library_hit_path(0, buf.as_mut_ptr(), buf.len()),
        llamp_ffi::LLAMP_OK
    );
    let hit = unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    assert_eq!(PathBuf::from(&hit), track);
    assert_eq!(llamp_ffi::llamp_library_enqueue_hit(0), llamp_ffi::LLAMP_OK);
    assert_eq!(fs::read(&track).expect("still there"), before);
    assert!(!root.join("Application Support").join("song.wav").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn playlist_list_font_promotes_the_visible_set() {
    let fixture = CString::new("FIXTURE").expect("ascii");
    let mixed = CString::new("F日").expect("mixed");
    let one = [fixture.as_ptr()];
    let two = [fixture.as_ptr(), mixed.as_ptr()];
    assert_eq!(llamp_ffi::llamp_playlist_list_font(one.as_ptr(), 1), 0);
    assert_eq!(llamp_ffi::llamp_playlist_list_font(two.as_ptr(), 2), 1);
    assert_eq!(llamp_ffi::llamp_playlist_row_font(mixed.as_ptr()), 1);
}
