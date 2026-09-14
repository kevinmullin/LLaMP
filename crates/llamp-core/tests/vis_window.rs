use llamp_core::{client_rect, propose_size, DockGroup, Pane, CHROME_LEFT, CHROME_TOP, VIS_MIN_H, VIS_MIN_W};

#[test]
fn chrome_owns_the_border_and_the_gpu_hole_is_the_client() {
    let client = client_rect(275, 116, false);
    assert_eq!(client.x, CHROME_LEFT);
    assert_eq!(client.y, CHROME_TOP);
    assert_eq!(client.w, 275 - 16);
    assert_eq!(client.h, 116 - 22);
    let full = client_rect(800, 600, true);
    assert_eq!((full.x, full.y), (0, 0));
    assert_eq!((full.w, full.h), (800, 600));
}

#[test]
fn vis_size_is_free_integer_not_a_playlist_step() {
    assert_eq!(propose_size(274, 115), (VIS_MIN_W, VIS_MIN_H));
    assert_eq!(propose_size(400, 300), (400, 300));
    assert_eq!(propose_size(310, 150), (310, 150));
}

#[test]
fn five_windows_snap_as_one_group() {
    let mut group = DockGroup::new();
    group.set_frame(Pane::Main, 0, 0, 275, 116);
    group.set_frame(Pane::Eq, 8, 126, 275, 116);
    group.set_frame(Pane::Playlist, 283, 8, 275, 116);
    group.set_frame(Pane::Browser, 291, 140, 275, 116);
    group.set_frame(Pane::Vis, 8, 256, 275, 200);
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
    let moved = group.drag(Pane::Main, 3, 4);
    assert!(moved.docked(Pane::Eq));
    assert!(moved.docked(Pane::Playlist));
    assert!(moved.docked(Pane::Browser));
    assert!(moved.docked(Pane::Vis));
    assert_eq!(moved.frame(Pane::Main).x, 3);
    assert_eq!(moved.frame(Pane::Vis).x, 3);
}
