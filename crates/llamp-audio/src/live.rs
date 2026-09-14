//! Latest PCM the feeder published. The audio callback does not write this.
//! The UI copies it. It does not wait.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, Ordering};

/// Last samples at the device rate, post the one resample on the feeder.
/// 1024 is the analysis window, not a spectrum bar count.
pub const PCM_WINDOW: usize = 1024;

static WINDOW: Seqlock<[f32; PCM_WINDOW]> = Seqlock::new([0.0; PCM_WINDOW]);

pub fn publish_pcm(samples: &[f32; PCM_WINDOW]) {
    WINDOW.write(*samples);
}

pub fn pcm_snapshot() -> [f32; PCM_WINDOW] {
    WINDOW.read()
}

struct Seqlock<T> {
    seq: AtomicU64,
    value: UnsafeCell<T>,
}

impl<T: Copy> Seqlock<T> {
    const fn new(value: T) -> Self {
        Self {
            seq: AtomicU64::new(0),
            value: UnsafeCell::new(value),
        }
    }

    fn write(&self, value: T) {
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(1), Ordering::Release);
        unsafe { *self.value.get() = value };
        self.seq.store(seq.wrapping_add(2), Ordering::Release);
    }

    fn read(&self) -> T {
        let mut last = unsafe { *self.value.get() };
        for _ in 0..8 {
            let start = self.seq.load(Ordering::Acquire);
            if start & 1 == 1 {
                continue;
            }
            let value = unsafe { *self.value.get() };
            let end = self.seq.load(Ordering::Acquire);
            if start == end {
                return value;
            }
            last = value;
        }
        last
    }
}

unsafe impl<T: Copy + Send> Sync for Seqlock<T> {}
