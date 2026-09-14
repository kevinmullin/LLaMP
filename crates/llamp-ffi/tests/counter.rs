use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn publisher_takes_no_lock_and_poll_is_monotonic() {
    let barrier = Arc::new(Barrier::new(2));
    let start = Arc::clone(&barrier);
    let publisher = thread::spawn(move || {
        start.wait();
        for _ in 0..10_000 {
            llamp_ffi::llamp_counter_publish();
        }
    });

    barrier.wait();
    let began = Instant::now();
    let mut previous = llamp_ffi::llamp_counter_poll();
    let mut last = previous;
    for _ in 0..1_000 {
        let value = llamp_ffi::llamp_counter_poll();
        assert!(
            value >= previous,
            "poll went backwards: {previous} -> {value}"
        );
        previous = value;
        last = value;
    }
    publisher.join().expect("publisher thread");
    let after = llamp_ffi::llamp_counter_poll();
    assert!(after >= last);
    assert!(after > 0, "publisher did not increment the counter");
    assert!(
        began.elapsed() < Duration::from_millis(200),
        "poll blocked while the publisher ran"
    );
}
