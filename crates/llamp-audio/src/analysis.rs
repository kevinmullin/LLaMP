//! FFT tap beside the audio thread. The hop allocates nothing.
//!
//! The spec names a 1024-point Hann window, a hop of 512, and magnitude in dB
//! against a full-scale sine in one bin. It does not name a spectral smoothing
//! coefficient. Peak-hold decay is deferred to the phase 2–3 golden image.
//! This tap does not invent one.

use std::sync::atomic::{AtomicU64, Ordering};

use rustfft::{num_complex::Complex, Fft, FftPlanner};

pub const WINDOW: usize = 1024;
pub const HOP: usize = 512;
const BINS: usize = WINDOW / 2 + 1;
/// Unnormalized rustfft magnitude of a full-scale sine in one bin after a Hann window.
const FULL_SCALE_BIN: f32 = (WINDOW as f32 / 2.0) * 0.5;

#[derive(Clone, Copy)]
pub struct Spectrum {
    pub bins_db: [f32; BINS],
}

pub struct FftTap {
    fft: std::sync::Arc<dyn Fft<f32>>,
    time: [f32; WINDOW],
    filled: usize,
    buf: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    window: [f32; WINDOW],
    snapshot: Seqlock<Spectrum>,
}

impl FftTap {
    pub const WINDOW: usize = WINDOW;
    pub const HOP: usize = HOP;

    pub fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(WINDOW);
        let scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];
        let mut window = [0f32; WINDOW];
        for (i, w) in window.iter_mut().enumerate() {
            *w = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (WINDOW as f32 - 1.0)).cos());
        }
        Self {
            fft,
            time: [0.0; WINDOW],
            filled: 0,
            buf: vec![Complex::new(0.0, 0.0); WINDOW],
            scratch,
            window,
            snapshot: Seqlock::new(Spectrum { bins_db: [ -120.0; BINS] }),
        }
    }

    /// Feed interleaved stereo. Returns a snapshot when a hop completes. Does not allocate.
    pub fn push(&mut self, interleaved: &[f32]) -> Option<Spectrum> {
        let frames = interleaved.len() / 2;
        let mut produced = None;
        for i in 0..frames {
            let sample = interleaved[i * 2];
            if self.filled < WINDOW {
                self.time[self.filled] = sample;
                self.filled += 1;
                if self.filled == WINDOW {
                    produced = Some(self.emit());
                }
            } else {
                self.time.copy_within(HOP..WINDOW, 0);
                self.filled = WINDOW - HOP;
                self.time[self.filled] = sample;
                self.filled += 1;
            }
        }
        produced
    }

    fn emit(&mut self) -> Spectrum {
        for i in 0..WINDOW {
            self.buf[i] = Complex::new(self.time[i] * self.window[i], 0.0);
        }
        self.fft.process_with_scratch(&mut self.buf, &mut self.scratch);
        let mut bins_db = [-120.0f32; BINS];
        for (i, bin) in bins_db.iter_mut().enumerate() {
            let mag = self.buf[i].norm();
            *bin = 20.0 * (mag / FULL_SCALE_BIN).max(1e-12).log10();
        }
        let snap = Spectrum { bins_db };
        self.snapshot.write(snap);
        snap
    }
}

impl Default for FftTap {
    fn default() -> Self {
        Self::new()
    }
}

struct Seqlock<T> {
    seq: AtomicU64,
    value: std::cell::UnsafeCell<T>,
}

impl<T: Copy> Seqlock<T> {
    fn new(value: T) -> Self {
        Self { seq: AtomicU64::new(0), value: std::cell::UnsafeCell::new(value) }
    }

    fn write(&self, value: T) {
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(1), Ordering::Release);
        unsafe { *self.value.get() = value };
        self.seq.store(seq.wrapping_add(2), Ordering::Release);
    }
}

// The analysis thread writes; the UI reads. Tests stay on one thread.
unsafe impl<T: Copy + Send> Send for Seqlock<T> {}
