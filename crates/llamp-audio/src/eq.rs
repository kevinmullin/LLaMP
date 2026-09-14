//! Preamp plus ten peaking biquads. Slider targets are atomic bits.
//! The callback slews applied gain; it does not write coefficients from the UI thread.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

/// Band centers in Hz. Q is `f / bandwidth`, bandwidth the geometric neighbor spacing.
pub const BAND_HZ: [f32; 10] = [
    60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0,
];

/// Locked in the phase 1 impulse test. Do not copy from another player.
pub const BAND_Q: [f32; 10] = [
    0.7517046896,
    1.3222394202,
    1.5368418110,
    1.7476432497,
    1.0444364487,
    1.1949382989,
    1.4142135624,
    2.6808453464,
    6.9820277360,
    4.8206060586,
];

const SLEW_SECONDS: f32 = 0.020;
const REDESIGN_DB: f32 = 0.01;

struct BandState {
    z1: [f32; 2],
    z2: [f32; 2],
    coeff: [f32; 5],
    applied_db: f32,
    coeff_db: f32,
}

struct Targets {
    bands: [AtomicU32; 10],
    preamp: AtomicU32,
    /// On is false until the window enables the graph. A 0 dB cascade is still a filter.
    bypass: AtomicBool,
}

impl Targets {
    fn fresh(bypass: bool) -> Self {
        Self {
            bands: std::array::from_fn(|_| AtomicU32::new(0f32.to_bits())),
            preamp: AtomicU32::new(0f32.to_bits()),
            bypass: AtomicBool::new(bypass),
        }
    }

    fn set_band(&self, band: usize, db: f32) {
        if let Some(slot) = self.bands.get(band) {
            slot.store(db.clamp(-12.0, 12.0).to_bits(), Ordering::Relaxed);
        }
    }

    fn set_preamp(&self, db: f32) {
        self.preamp
            .store(db.clamp(-12.0, 12.0).to_bits(), Ordering::Relaxed);
    }
}

fn ui_targets() -> Arc<Targets> {
    static UI: OnceLock<Arc<Targets>> = OnceLock::new();
    Arc::clone(UI.get_or_init(|| Arc::new(Targets::fresh(true))))
}

/// Window slider drag. Writes the playback target. Does not redesign coefficients.
pub fn drag_band(band: usize, db: f32) {
    ui_targets().set_band(band, db);
}

/// One step of `llamp play --eq-sweep`. Enables the cascade. Other bands go to 0 dB.
pub fn set_eq_sweep_band(band: usize) {
    set_eq_enabled(true);
    for other in 0..10 {
        drag_band(other, if other == band { 12.0 } else { 0.0 });
    }
}

/// Frame index → band for an even sweep across a play. `frames` is stereo frames.
pub fn sweep_band_at(frame: usize, frames: usize) -> usize {
    if frames == 0 {
        return 0;
    }
    frame
        .saturating_mul(10)
        .min(frames.saturating_mul(10).saturating_sub(1))
        / frames.max(1)
}

/// Window preamp drag. Same slew path as a band.
pub fn drag_preamp(db: f32) {
    ui_targets().set_preamp(db);
}

/// The On button. Off bypasses the cascade. Does not write coefficients.
pub fn set_eq_enabled(on: bool) {
    ui_targets().bypass.store(!on, Ordering::Relaxed);
}

pub fn eq_enabled() -> bool {
    !ui_targets().bypass.load(Ordering::Relaxed)
}

pub fn band_target_db(band: usize) -> f32 {
    ui_targets()
        .bands
        .get(band)
        .map(|slot| f32::from_bits(slot.load(Ordering::Relaxed)))
        .unwrap_or(0.0)
}

pub fn preamp_target_db() -> f32 {
    f32::from_bits(ui_targets().preamp.load(Ordering::Relaxed))
}

/// Magnitude of the current UI targets, in dB, one sample per `out` slot.
/// Log-spaced from 20 Hz to the Nyquist cap. UI thread only.
pub fn curve_db(sample_rate: u32, out: &mut [f32]) {
    if sample_rate == 0 || out.is_empty() {
        out.fill(0.0);
        return;
    }
    let targets = ui_targets();
    let preamp = f32::from_bits(targets.preamp.load(Ordering::Relaxed));
    let bands =
        std::array::from_fn(|band| f32::from_bits(targets.bands[band].load(Ordering::Relaxed)));
    if targets.bypass.load(Ordering::Relaxed) {
        out.fill(0.0);
        return;
    }
    fill_curve(sample_rate, preamp, &bands, out);
}

fn fill_curve(sample_rate: u32, preamp_db: f32, bands: &[f32; 10], out: &mut [f32]) {
    let coeffs: [[f32; 5]; 10] = std::array::from_fn(|band| {
        peaking(sample_rate as f32, BAND_HZ[band], BAND_Q[band], bands[band])
    });
    let pre = 10f32.powf(preamp_db / 20.0);
    let lo = 20.0f32;
    let hi = (sample_rate as f32 / 2.0).min(20_000.0).max(lo * 1.01);
    let last = out.len().saturating_sub(1).max(1) as f32;
    for (i, slot) in out.iter_mut().enumerate() {
        let t = i as f32 / last;
        let hz = lo * (hi / lo).powf(t);
        let w = 2.0 * std::f32::consts::PI * hz / sample_rate as f32;
        let mut mag = pre;
        for coeff in &coeffs {
            mag *= biquad_mag(coeff, w);
        }
        *slot = 20.0 * mag.max(1e-12).log10();
    }
}

pub struct Eq {
    rate: u32,
    slew: f32,
    bands: [BandState; 10],
    preamp_applied: f32,
    shared: Arc<Targets>,
}

impl Eq {
    pub fn new(sample_rate: u32) -> Self {
        Self::with_targets(sample_rate, Arc::new(Targets::fresh(false)))
    }

    fn with_targets(sample_rate: u32, shared: Arc<Targets>) -> Self {
        let slew = 1.0 - (-1.0 / (SLEW_SECONDS * sample_rate as f32)).exp();
        let mut bands = std::array::from_fn(|_| BandState {
            z1: [0.0; 2],
            z2: [0.0; 2],
            coeff: peaking(sample_rate as f32, 1000.0, 1.0, 0.0),
            applied_db: 0.0,
            coeff_db: 0.0,
        });
        for (band, state) in bands.iter_mut().enumerate() {
            state.coeff = peaking(sample_rate as f32, BAND_HZ[band], BAND_Q[band], 0.0);
        }
        Self {
            rate: sample_rate,
            slew,
            bands,
            preamp_applied: 0.0,
            shared,
        }
    }

    /// Playback and the window share one target bank. Filter state stays on this Eq.
    pub fn bind_ui_sliders(&mut self) {
        self.shared = ui_targets();
    }

    pub fn set_band_db(&self, band: usize, db: f32) {
        self.shared.set_band(band, db);
    }

    pub fn set_preamp_db(&self, db: f32) {
        self.shared.set_preamp(db);
    }

    pub fn set_bypass(&self, bypass: bool) {
        self.shared.bypass.store(bypass, Ordering::Relaxed);
    }

    /// In place on interleaved stereo. Allocates nothing.
    pub fn process(&mut self, frames: &mut [f32]) {
        if self.shared.bypass.load(Ordering::Relaxed) {
            return;
        }
        let n = frames.len() / 2;
        for i in 0..n {
            let pre_db = f32::from_bits(self.shared.preamp.load(Ordering::Relaxed));
            self.preamp_applied += self.slew * (pre_db - self.preamp_applied);
            let pre = 10f32.powf(self.preamp_applied / 20.0);
            let mut l = frames[i * 2] * pre;
            let mut r = frames[i * 2 + 1] * pre;
            for band in 0..10 {
                let target = f32::from_bits(self.shared.bands[band].load(Ordering::Relaxed));
                let state = &mut self.bands[band];
                state.applied_db += self.slew * (target - state.applied_db);
                if (state.applied_db - state.coeff_db).abs() > REDESIGN_DB {
                    state.coeff = peaking(
                        self.rate as f32,
                        BAND_HZ[band],
                        BAND_Q[band],
                        state.applied_db,
                    );
                    state.coeff_db = state.applied_db;
                }
                l = biquad(l, &state.coeff, &mut state.z1[0], &mut state.z2[0]);
                r = biquad(r, &state.coeff, &mut state.z1[1], &mut state.z2[1]);
            }
            frames[i * 2] = l;
            frames[i * 2 + 1] = r;
        }
    }

    /// Impulse response magnitude at the band center, after the slew has settled.
    pub fn impulse_center_db(&mut self, band: usize, n: usize) -> f32 {
        let settle = self.rate as usize;
        let mut silence = vec![0f32; settle * 2];
        self.process(&mut silence);
        let mut ir = vec![0f32; n * 2];
        ir[0] = 1.0;
        ir[1] = 1.0;
        self.process(&mut ir);
        let hz = BAND_HZ[band];
        let mut re = 0.0f32;
        let mut im = 0.0f32;
        for i in 0..n {
            let w = 2.0 * std::f32::consts::PI * hz * i as f32 / self.rate as f32;
            let s = ir[i * 2];
            re += s * w.cos();
            im -= s * w.sin();
        }
        let mag = re.hypot(im);
        20.0 * mag.max(1e-12).log10()
    }
}

fn biquad(x: f32, c: &[f32; 5], z1: &mut f32, z2: &mut f32) -> f32 {
    let y = c[0] * x + *z1;
    *z1 = c[1] * x - c[3] * y + *z2;
    *z2 = c[2] * x - c[4] * y;
    y
}

fn biquad_mag(c: &[f32; 5], w: f32) -> f32 {
    let (b0, b1, b2, a1, a2) = (c[0], c[1], c[2], c[3], c[4]);
    let cos1 = w.cos();
    let sin1 = w.sin();
    let cos2 = (2.0 * w).cos();
    let sin2 = (2.0 * w).sin();
    let num_re = b0 + b1 * cos1 + b2 * cos2;
    let num_im = -(b1 * sin1 + b2 * sin2);
    let den_re = 1.0 + a1 * cos1 + a2 * cos2;
    let den_im = -(a1 * sin1 + a2 * sin2);
    num_re.hypot(num_im) / den_re.hypot(den_im).max(1e-12)
}

fn peaking(fs: f32, f0: f32, q: f32, db: f32) -> [f32; 5] {
    let a = 10f32.powf(db / 40.0);
    let w0 = 2.0 * std::f32::consts::PI * f0 / fs;
    let alpha = w0.sin() / (2.0 * q.max(1e-4));
    let cos = w0.cos();
    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cos;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    [b0 / a0, b1 / a0, b2 / a0, b1 / a0, (1.0 - alpha / a) / a0]
}
