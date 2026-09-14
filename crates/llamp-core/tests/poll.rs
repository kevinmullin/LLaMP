use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use llamp_core::Session;

#[test]
fn note_played_is_an_atomic_add_the_callback_can_make() {
    let session = Session::new();
    session.configure(48_000, 48_000, 2, b"t");
    session.note_played(128);
    assert_eq!(session.poll().position_frames, 128);
    session.seek_by(-64);
    assert_eq!(session.poll().position_frames, 64);
}

#[test]
fn seek_slider_is_a_fraction_of_duration() {
    let session = Session::new();
    session.configure(44_100, 10_000, 2, b"t");
    session.set_slider(15, 250);
    assert_eq!(session.poll().position_frames, 2_500);
    session.set_slider(16, 1000);
    assert_eq!(session.poll().volume_ppm, 1000);
}

#[test]
fn retain_references_writes_storage_referenced() {
    let root = std::env::temp_dir().join(format!("llamp-refs-{}", std::process::id()));
    let files = root.join("files");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&files).expect("dir");
    for i in 0..1000 {
        fs::write(files.join(format!("track-{i:04}")), b"").expect("file");
    }
    let session = Session::new();
    assert_eq!(session.retain_references(&files).expect("refs"), 1000);
    assert_eq!(session.reference_count(), 1000);
    let lib = llamp_library::Library::open(&root.join("library.sqlite")).expect("reopen");
    assert_eq!(
        lib.storage_of(&files.join("track-0000"))
            .expect("storage")
            .as_deref(),
        Some("referenced")
    );
    let _ = fs::remove_dir_all(&files);
    assert_eq!(session.reference_count(), 1000);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn poll_returns_while_another_thread_publishes() {
    let session = Session::new();
    session.configure(44_100, 1_000_000, 2, b"t");
    let stop = AtomicBool::new(false);
    thread::scope(|scope| {
        scope.spawn(|| {
            while !stop.load(Ordering::Relaxed) {
                session.note_played(1);
            }
        });
        let began = Instant::now();
        for _ in 0..1000 {
            let _ = session.poll();
        }
        assert!(began.elapsed() < Duration::from_millis(500));
        stop.store(true, Ordering::Relaxed);
    });
}
