use llamp_core::{
    client_rect as vis_client, cursor_for, line_rect, mark_rect, text_origin, DockGroup, Pane,
    Session, LINE_H, MARK_H, MARK_W, TEXT_PAD,
};
use llamp_library::{parse_lrc, LyricKind};

#[test]
fn chrome_and_text_share_integer_origins_at_each_scale() {
    for scale in [1, 2, 4] {
        let (x, y) = text_origin(scale);
        assert_eq!(x % scale, 0, "scale {scale} x={x}");
        assert_eq!(y % scale, 0, "scale {scale} y={y}");
        let client = vis_client(275, 116, false);
        let line = line_rect(0, 0, client.w, scale);
        assert_eq!(line.x, x);
        assert_eq!(line.y, y);
        assert_eq!(line.h, LINE_H * scale);
        assert_eq!(line.w % scale, 0);
        let scrolled = line_rect(3, LINE_H, client.w, scale);
        assert_eq!(scrolled.y, y + (3 * LINE_H - LINE_H) * scale);
        let resized = vis_client(400, 240, false);
        let wide = line_rect(0, 0, resized.w, scale);
        assert_eq!(wide.w, (resized.w - 2 * TEXT_PAD) * scale);
        assert_eq!(wide.x % scale, 0);
        assert_eq!(wide.y % scale, 0);
    }
}

#[test]
fn mark_is_integer_scaled_in_the_client_hole() {
    for scale in [1, 2, 4] {
        let mark = mark_rect(275, 116, scale);
        assert_eq!(mark.w, MARK_W * scale);
        assert_eq!(mark.h, MARK_H * scale);
        assert_eq!(mark.x % scale, 0);
        assert_eq!(mark.y % scale, 0);
    }
}

#[test]
fn six_windows_snap_as_one_group() {
    let mut group = DockGroup::new();
    group.set_frame(Pane::Main, 0, 0, 275, 116);
    group.set_frame(Pane::Eq, 8, 126, 275, 116);
    group.set_frame(Pane::Playlist, 283, 8, 275, 116);
    group.set_frame(Pane::Browser, 291, 140, 275, 116);
    group.set_frame(Pane::Vis, 8, 256, 275, 200);
    group.set_frame(Pane::Lyrics, 291, 256, 275, 200);
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
    group.drag(Pane::Vis, -8, -24);
    group.end_drag();
    group.begin_drag();
    group.drag(Pane::Lyrics, -8, -24);
    group.end_drag();
    group.begin_drag();
    let moved = group.drag(Pane::Main, 3, 4);
    assert!(moved.docked(Pane::Lyrics));
    assert!(moved.docked(Pane::Vis));
    assert_eq!(moved.frame(Pane::Main).x, 3);
    assert_eq!(moved.frame(Pane::Vis).x, 3);
    assert_eq!(moved.frame(Pane::Lyrics).x, 278);
}

#[test]
fn pause_and_device_reopen_keep_the_active_line() {
    let doc = parse_lrc("[00:00.50]One\n[00:01.00]Two\n").expect("lrc");
    assert_eq!(doc.kind, LyricKind::Synced);
    let session = Session::new();
    session.configure(44_100, 44_100 * 4, 2, b"t");
    session.toggle_play();
    session.note_played(44_100);
    let playing = cursor_for(&doc, session.poll().position_frames, session.poll().sample_rate, 0);
    assert_eq!(playing.line, Some(1));
    session.toggle_play();
    session.note_played(44_100);
    let paused = cursor_for(&doc, session.poll().position_frames, session.poll().sample_rate, 0);
    assert_eq!(paused.line, Some(1), "pause must freeze the line");
    assert_eq!(session.clock_ms(), 1000);
    session.reopen_device(48_000, true);
    assert_eq!(session.events().underruns.load(std::sync::atomic::Ordering::Relaxed), 1);
    let after = cursor_for(&doc, session.poll().position_frames, session.poll().sample_rate, 0);
    assert_eq!(after.line, Some(1), "a device-rate reopen must not drift the line");
    assert!((session.clock_ms() - 1000).unsigned_abs() <= 16);
}
