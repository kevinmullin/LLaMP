use llamp_core::{LINE_H, TEXT_PAD};
use llamp_ffi::{
    llamp_group_begin_drag, llamp_group_drag, llamp_group_end_drag, llamp_group_reset,
    llamp_group_set_frame, llamp_lyrics_client_rect, llamp_lyrics_line_rect, llamp_lyrics_mark_rect,
    llamp_lyrics_min_size, llamp_lyrics_privacy_note, llamp_lyrics_propose_size,
    llamp_lyrics_sidecar_note, llamp_lyrics_text_origin, LlampFrame,
};
use std::ffi::CStr;

#[test]
fn privacy_note_lists_the_four_fields() {
    let note = unsafe { CStr::from_ptr(llamp_lyrics_privacy_note()) }
        .to_str()
        .expect("utf8");
    assert!(note.contains("artist"));
    assert!(note.contains("title"));
    assert!(note.contains("album"));
    assert!(note.contains("duration"));
    assert!(note.contains("LRCLIB"));
}

#[test]
fn sidecar_note_says_the_sidecar_was_not_written() {
    let note = unsafe { CStr::from_ptr(llamp_lyrics_sidecar_note()) }
        .to_str()
        .expect("utf8");
    assert_eq!(note, "The sidecar was not written.");
}

#[test]
fn text_and_chrome_stay_on_integer_pixels() {
    let min = llamp_lyrics_min_size();
    assert_eq!(min.width, 275);
    assert_eq!(min.height, 116);
    let size = llamp_lyrics_propose_size(400, 240);
    let client = llamp_lyrics_client_rect(size.width as i32, size.height as i32);
    assert_eq!(client.x, 8);
    assert_eq!(client.y, 14);
    for scale in [1, 2, 4] {
        let origin = llamp_lyrics_text_origin(scale);
        assert_eq!(origin.x % scale, 0);
        assert_eq!(origin.y % scale, 0);
        let line = llamp_lyrics_line_rect(2, LINE_H, scale);
        assert_eq!(line.h, LINE_H * scale);
        assert_eq!(line.w, (client.w - 2 * TEXT_PAD) * scale);
        let mark = llamp_lyrics_mark_rect(scale);
        assert_eq!(mark.x % scale, 0);
        assert_eq!(mark.y % scale, 0);
        assert_eq!(mark.w % scale, 0);
    }
}

#[test]
fn sixth_window_is_in_the_group() {
    llamp_group_reset();
    llamp_group_set_frame(0, frame(0, 0, 275, 116));
    llamp_group_set_frame(1, frame(8, 126, 275, 116));
    llamp_group_set_frame(2, frame(283, 8, 275, 116));
    llamp_group_set_frame(3, frame(291, 140, 275, 116));
    llamp_group_set_frame(4, frame(8, 256, 275, 200));
    llamp_group_set_frame(5, frame(291, 256, 275, 200));
    for (which, dx, dy) in [(1, -8, -10), (2, -8, -8), (3, -8, -24), (4, -8, -24), (5, -8, -24)] {
        llamp_group_begin_drag();
        llamp_group_drag(which, dx, dy);
        llamp_group_end_drag();
    }
    llamp_group_begin_drag();
    let moved = llamp_group_drag(0, 3, 4);
    assert_eq!(moved.lyrics_docked, 1);
    assert_eq!(moved.main.x, 3);
    assert_eq!(moved.lyrics.x, 278);
}

fn frame(x: i32, y: i32, w: i32, h: i32) -> LlampFrame {
    LlampFrame { x, y, w, h }
}
