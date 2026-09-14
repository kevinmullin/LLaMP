//! EQ cascade, coefficient slew, clip policy, and the FFT tap.

use llamp_audio::analysis::FftTap;
use llamp_audio::eq::{Eq, BAND_HZ, BAND_Q};
use llamp_audio::graph::Stage;
use llamp_audio::limiter::Limiter;

const Q: [f32; 10] = [
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

#[test]
fn q_vector_is_fixed_and_center_gain_is_within_half_db() {
    assert_eq!(BAND_HZ, [60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0]);
    for (got, want) in BAND_Q.iter().zip(Q) {
        assert!((got - want).abs() < 1e-6, "Q {got} != {want}");
    }
    let rate = 48_000u32;
    for (band, &hz) in BAND_HZ.iter().enumerate() {
        let mut eq = Eq::new(rate);
        eq.set_band_db(band, 6.0);
        let mag_db = eq.impulse_center_db(band, 8192);
        assert!(
            (mag_db - 6.0).abs() <= 0.5,
            "band {band} ({hz} Hz) magnitude {mag_db} dB, want 6 ± 0.5"
        );
    }
}

#[test]
fn slider_crossfades_instead_of_stepping() {
    let rate = 48_000u32;
    let mut eq = Eq::new(rate);
    let n = rate as usize / 10;
    let mut input = vec![0f32; n * 2];
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.2;
        input[i * 2] = s;
        input[i * 2 + 1] = s;
    }
    eq.process(&mut input);
    let before = rms(&input[..256]);

    eq.set_band_db(4, 12.0);
    let mut after = vec![0f32; n * 2];
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.2;
        after[i * 2] = s;
        after[i * 2 + 1] = s;
    }
    eq.process(&mut after);
    let early = rms(&after[..256]);
    let late = rms(&after[after.len() - 2048..]);
    let step = before * 10f32.powf(12.0 / 20.0);
    assert!(
        early < before + 0.5 * (step - before),
        "first 256 samples already stepped: early {early}, before {before}, step {step}"
    );
    assert!(
        late > early * 1.5,
        "envelope did not rise across the buffer: early {early}, late {late}"
    );
}

#[test]
fn boost_may_exceed_full_scale_before_the_limiter() {
    let mut stage = Stage::new(48_000);
    for band in 0..10 {
        stage.eq.set_band_db(band, 12.0);
    }
    stage.eq.set_preamp_db(0.0);
    let mut frames = sine_1k(48_000, 0.5, 8192);
    let trace = stage.process(&mut frames);
    assert!(
        trace.pre_limiter_peak > 1.0,
        "several +12 dB bands must be allowed to exceed ±1 before the limiter, peak {}",
        trace.pre_limiter_peak
    );
    let ceiling = Limiter::CEILING;
    assert!(
        frames.iter().all(|s| s.abs() <= ceiling + 1e-4),
        "limiter ceiling is -1 dBFS"
    );

    let mut quieter = Stage::new(48_000);
    for band in 0..10 {
        quieter.eq.set_band_db(band, 12.0);
    }
    quieter.eq.set_preamp_db(-12.0);
    let mut frames = sine_1k(48_000, 0.5, 8192);
    let trimmed = quieter.process(&mut frames);
    assert!(
        trimmed.pre_limiter_peak < trace.pre_limiter_peak,
        "preamp sits before the cascade"
    );
}

#[test]
fn fft_tap_uses_named_window_hop_and_full_scale_reference() {
    assert_eq!(FftTap::WINDOW, 1024);
    assert_eq!(FftTap::HOP, 512);
    let rate = 48_000f32;
    let bin = 8usize;
    let hz = bin as f32 * rate / FftTap::WINDOW as f32;
    let mut pcm = vec![0f32; FftTap::WINDOW * 2];
    for i in 0..FftTap::WINDOW {
        let s = (2.0 * std::f32::consts::PI * hz * i as f32 / rate).sin();
        pcm[i * 2] = s;
        pcm[i * 2 + 1] = s;
    }
    let mut tap = FftTap::new();
    let snap = tap.push(&pcm).expect("one hop after a full window");
    assert!((snap.bins_db[bin] - 0.0).abs() <= 1.0, "bin {bin} is {} dB", snap.bins_db[bin]);
    assert!(snap.bins_db[bin] > snap.bins_db[bin + 4]);
}

fn sine_1k(rate: u32, amp: f32, frames: usize) -> Vec<f32> {
    let mut out = vec![0f32; frames * 2];
    for i in 0..frames {
        let t = i as f32 / rate as f32;
        let s = amp * (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
        out[i * 2] = s;
        out[i * 2 + 1] = s;
    }
    out
}

fn rms(interleaved: &[f32]) -> f32 {
    let mut acc = 0.0f32;
    for s in interleaved {
        acc += s * s;
    }
    (acc / interleaved.len() as f32).sqrt()
}
