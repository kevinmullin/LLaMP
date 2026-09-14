//! Kill is off the audio thread. The allocator hook still passes during the kill.

use std::sync::atomic::{AtomicU64, Ordering};

use llamp_audio::graph::Callback;
use llamp_audio::realtime;
use llamp_plugin_host::vis_budget::{FakeClock, VisBudget};

llamp_audio::install_realtime_hook!();

#[test]
fn sleep_past_budget_is_killed_and_the_callback_stays_clean() {
    let mut callback = Callback::new(256, 48_000);
    let pcm = vec![0.25f32; 512 * 8];
    let last = AtomicU64::new(0);
    let underruns = AtomicU64::new(0);
    callback.push_pcm(&pcm);
    let mut out = [0f32; 512];
    realtime::assert_section_clean(|| {
        callback.fill(&mut out);
        last.store(callback.played_frames(), Ordering::Relaxed);
        underruns.store(callback.underruns(), Ordering::Relaxed);
        assert!(last.load(Ordering::Relaxed) > 0);
    });

    let clock = FakeClock::new(50);
    let mut budget = VisBudget::new();
    for _ in 0..3 {
        budget.run(&clock, || {});
    }
    assert!(budget.killed, "three 50 ms draws must trip the budget");

    callback.push_pcm(&pcm);
    realtime::assert_section_clean(|| {
        callback.fill(&mut out);
        assert!(
            callback.played_frames() > last.load(Ordering::Relaxed),
            "audio must keep publishing after the kill"
        );
        assert_eq!(
            callback.underruns(),
            underruns.load(Ordering::Relaxed),
            "the kill must not starve the ring"
        );
    });
}
