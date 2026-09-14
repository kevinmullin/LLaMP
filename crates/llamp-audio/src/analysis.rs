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
            *w =
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (WINDOW as f32 - 1.0)).cos());
        }
        Self {
            fft,
            time: [0.0; WINDOW],
            filled: 0,
            buf: vec![Complex::new(0.0, 0.0); WINDOW],
            scratch,
            window,
            snapshot: Seqlock::new(Spectrum {
                bins_db: [-120.0; BINS],
            }),
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
        self.fft
            .process_with_scratch(&mut self.buf, &mut self.scratch);
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

/// Unique bins from a 1024-point real FFT, Nyquist included. The packet’s
/// “512 bins” line is this length when Nyquist is omitted; this tap keeps it.
pub const LINEAR_BINS: usize = BINS;

#[derive(Clone, Copy)]
pub struct FrameHop {
    pub pcm: [f32; 1024],
    pub bins_db: [f32; BINS],
    pub rms_l: f32,
    pub rms_r: f32,
    pub peak_l: f32,
    pub peak_r: f32,
    pub onset: f32,
}

impl FrameHop {
    const fn empty() -> Self {
        Self {
            pcm: [0.0; 1024],
            bins_db: [-120.0; BINS],
            rms_l: 0.0,
            rms_r: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
            onset: 0.0,
        }
    }
}

fn hop_cell() -> &'static Seqlock<FrameHop> {
    static CELL: std::sync::OnceLock<Seqlock<FrameHop>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| Seqlock::new(FrameHop::empty()))
}

/// Analysis thread (or a test) publishes. The visualizer pulls. Not the callback.
pub fn publish_hop(hop: FrameHop) {
    hop_cell().write(hop);
}

pub fn hop_snapshot() -> FrameHop {
    hop_cell().read()
}

/// Bar count is pane width over the golden bar width. Do not pass a literal 19.
pub fn band_count(pane_w: u32, bar_w: u32) -> usize {
    (pane_w / bar_w.max(1)) as usize
}

/// Log-spaced 0..1 energies from the existing linear dB bins. No second FFT.
pub fn log_bands(bins_db: &[f32], pane_w: u32, bar_w: u32, out: &mut [f32]) -> usize {
    let count = band_count(pane_w, bar_w).min(out.len());
    if count == 0 || bins_db.len() < 2 {
        return 0;
    }
    let last = (bins_db.len() - 1) as f32;
    for i in 0..count {
        let t0 = i as f32 / count as f32;
        let t1 = (i + 1) as f32 / count as f32;
        let a = 1.0 + (last.powf(t0) - 1.0);
        let b = 1.0 + (last.powf(t1) - 1.0);
        let lo = a.min(b).clamp(1.0, last) as usize;
        let hi = a.max(b).clamp(1.0, last) as usize;
        let mut acc = 0.0f32;
        let mut n = 0u32;
        for bin in lo..=hi {
            acc += bins_db[bin];
            n += 1;
        }
        let db = if n == 0 { -120.0 } else { acc / n as f32 };
        out[i] = ((db + 120.0) / 120.0).clamp(0.0, 1.0);
    }
    count
}

pub fn stereo_meters(interleaved: &[f32]) -> ([f32; 2], [f32; 2]) {
    let mut sum = [0.0f32; 2];
    let mut peak = [0.0f32; 2];
    let mut n = 0u32;
    for pair in interleaved.chunks_exact(2) {
        sum[0] += pair[0] * pair[0];
        sum[1] += pair[1] * pair[1];
        peak[0] = peak[0].max(pair[0].abs());
        peak[1] = peak[1].max(pair[1].abs());
        n += 1;
    }
    let rms = if n == 0 {
        [0.0, 0.0]
    } else {
        [(sum[0] / n as f32).sqrt(), (sum[1] / n as f32).sqrt()]
    };
    (rms, peak)
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
        Self {
            seq: AtomicU64::new(0),
            value: std::cell::UnsafeCell::new(value),
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

// The analysis thread writes; the UI reads. Tests stay on one thread.
unsafe impl<T: Copy + Send> Send for Seqlock<T> {}
unsafe impl<T: Copy + Send> Sync for Seqlock<T> {}
