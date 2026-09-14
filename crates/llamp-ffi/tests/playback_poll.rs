use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn poll_does_not_wait_and_seek_does_not_need_a_channel() {
    llamp_ffi::llamp_session_configure(44_100, 44_100 * 10, 2, c"tone".as_ptr());
    let start = llamp_ffi::llamp_playback_poll();
    assert_eq!(start.position_frames, 0);
    assert_eq!(start.sample_rate, 44_100);
    assert_eq!(start.transport, 0);
    assert_eq!(&start.title[..5], b"tone\0");

    llamp_ffi::llamp_transport_seek_by(44_100);
    let seek = llamp_ffi::llamp_playback_poll();
    assert_eq!(seek.position_frames, 44_100);

    llamp_ffi::llamp_transport_toggle_play();
    let playing = llamp_ffi::llamp_playback_poll();
    assert_eq!(playing.transport, 1);

    let stop = AtomicBool::new(false);
    thread::scope(|scope| {
        scope.spawn(|| {
            while !stop.load(Ordering::Relaxed) {
                llamp_ffi::llamp_transport_seek_by(1);
            }
        });
        let began = Instant::now();
        for _ in 0..60 {
            let _ = llamp_ffi::llamp_playback_poll();
        }
        assert!(began.elapsed() < Duration::from_millis(200), "poll waited");
        stop.store(true, Ordering::Relaxed);
    });
}
