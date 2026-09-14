//! The callback and the FFT hop must not allocate, lock, or do I/O.

use llamp_audio::analysis::FftTap;
use llamp_audio::graph::Callback;
use llamp_audio::realtime::{self, Violation};

llamp_audio::install_realtime_hook!();

#[test]
fn hook_fails_when_the_section_allocates() {
    realtime::assert_section_catches(Violation::Alloc, || {
        let _boxed = Box::new(1u32);
    });
}

#[test]
fn hook_fails_when_the_section_locks() {
    let lock = std::sync::Mutex::new(0u32);
    realtime::assert_section_catches(Violation::Lock, || {
        *lock.lock().unwrap() += 1;
    });
}

#[test]
fn hook_fails_when_the_section_does_io() {
    let path = std::env::temp_dir().join("llamp-rt-io-probe.txt");
    realtime::assert_section_catches(Violation::Io, || {
        std::fs::write(&path, b"no").unwrap();
    });
    let _ = std::fs::remove_file(&path);
}

#[test]
fn callback_and_fft_hop_stay_clean() {
    let mut callback = Callback::new(256, 48_000);
    let mut out = [0f32; 512];
    let mut tap = FftTap::new();
    let pcm = vec![0f32; FftTap::WINDOW * 2];
    realtime::assert_section_clean(|| {
        callback.fill(&mut out);
        assert_eq!(callback.underruns(), 1, "empty ring writes silence and counts one underrun");
        assert!(out.iter().all(|s| *s == 0.0));
        let _ = tap.push(&pcm);
    });
}
