//! Playlist resize, virtual window, and per-list font choice.
//! The shell hit-test must use the same window; this test locks the math.

use std::fs;

use llamp_core::{glyph_font, list_font, row_font, DockGroup, Pane, PlaylistWindow, RowFont};

#[test]
fn resize_rejects_a_mid_step_drag() {
    let mut window = PlaylistWindow::new();
    assert_eq!(window.size(), (275, 116));
    window.propose_size(300, 145).expect("25 and 29");
    assert_eq!(window.size(), (300, 145));
    let err = window.propose_size(310, 145).expect_err("10 px is not a 25 px step");
    assert!(err.contains("25"), "{err}");
    assert_eq!(window.size(), (300, 145), "a rejected drag must not apply");
    let err = window.propose_size(300, 150).expect_err("5 px is not a 29 px step");
    assert!(err.contains("29"), "{err}");
    assert!(window.propose_size(274, 116).is_err());
}

#[test]
fn ten_thousand_rows_expose_only_the_visible_window() {
    let window = PlaylistWindow::new();
    let range = window.visible_range(10_000, 0);
    assert!(range.end <= range.start + 20, "viewport must not include every row: {range:?}");
    assert!(range.end < 10_000);
    assert_eq!(window.hit_row(14, 0, 10_000), Some(0));
    assert_eq!(window.hit_row(21, 40, 10_000), Some(41));
    assert_eq!(window.hit_row(14, 9990, 10_000), Some(9990));
    assert_eq!(window.hit_row(7, 40, 10_000), None);
    assert_eq!(window.rows_touched(10_000, 0), range.end - range.start);
}

#[test]
fn a_missing_glyph_in_the_visible_set_promotes_the_list() {
    let ascii = |ch: char| (' '..='~').contains(&ch);
    assert_eq!(row_font("FIXTURE", ascii), RowFont::Bitmap);
    assert_eq!(glyph_font('F', ascii), RowFont::Bitmap);
    assert_eq!(glyph_font('日', ascii), RowFont::CoreText);
    assert_eq!(row_font("F日", ascii), RowFont::CoreText);
    assert_eq!(list_font(["FIXTURE"], ascii), RowFont::Bitmap);
    assert_eq!(list_font(["FIXTURE", "F日"], ascii), RowFont::CoreText);
    assert_ne!(list_font(["FIXTURE", "F日"], ascii), RowFont::Bitmap);
}

#[test]
fn remove_does_not_delete_the_file() {
    let root = std::env::temp_dir().join(format!("llamp-pl-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("dir");
    let path = root.join("keep.wav");
    fs::write(&path, b"x").expect("file");
    let mut window = PlaylistWindow::new();
    window.enqueue(path.to_string_lossy().as_ref());
    window.enqueue("second");
    window.reorder(1, 0);
    assert_eq!(window.entry(0), "second");
    window.remove(1).expect("remove");
    assert!(path.is_file(), "playlist remove must not delete the file");
    assert_eq!(window.len(), 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn enqueue_hit_keeps_the_granted_path() {
    let mut window = PlaylistWindow::new();
    let path = std::path::PathBuf::from("/music/granted.wav");
    window.enqueue_hit(&llamp_library::SearchHit { path: path.clone() });
    assert_eq!(window.entry(0), path.to_string_lossy().as_ref());
}

#[test]
fn four_windows_snap_as_one_group() {
    let mut group = DockGroup::new();
    group.set_frame(Pane::Main, 0, 0, 275, 116);
    group.set_frame(Pane::Eq, 8, 126, 275, 116);
    group.set_frame(Pane::Playlist, 283, 8, 275, 116);
    group.set_frame(Pane::Browser, 291, 140, 275, 116);
    group.begin_drag();
    group.drag(Pane::Eq, -8, -10);
    group.end_drag();
    group.begin_drag();
    group.drag(Pane::Playlist, -8, -8);
    group.end_drag();
    group.begin_drag();
    group.drag(Pane::Browser, -8, -24);
    group.end_drag();
    group.begin_drag();
    let moved = group.drag(Pane::Main, 3, 4);
    assert!(moved.docked(Pane::Eq));
    assert!(moved.docked(Pane::Playlist));
    assert!(moved.docked(Pane::Browser));
    assert_eq!(moved.frame(Pane::Main).x, 3);
    assert_eq!(moved.frame(Pane::Eq).x, 3);
    assert_eq!(moved.frame(Pane::Playlist).x, 278);
    assert_eq!(moved.frame(Pane::Browser).x, 278);
}

#[test]
fn browser_text_is_never_the_bitmap_font() {
    assert_eq!(llamp_core::browser_row_font("FIXTURE"), RowFont::CoreText);
    assert_eq!(llamp_core::browser_row_font("周杰伦"), RowFont::CoreText);
}
