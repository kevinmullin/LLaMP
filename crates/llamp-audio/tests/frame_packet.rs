//! One 1024/Hann/512 tap. Nyquist stays in the linear bins.

use llamp_audio::analysis::{
    band_count, log_bands, publish_hop, stereo_meters, FftTap, FrameHop, LINEAR_BINS,
};

#[test]
fn packet_reuses_the_named_tap_and_includes_nyquist() {
    assert_eq!(FftTap::WINDOW, 1024);
    assert_eq!(FftTap::HOP, 512);
    assert_eq!(LINEAR_BINS, 513);
    let rate = 48_000f32;
    let bin = 8usize;
    let hz = bin as f32 * rate / FftTap::WINDOW as f32;
    let mut pcm = vec![0f32; FftTap::WINDOW * 2];
    for i in 0..FftTap::WINDOW {
        let s = (2.0 * std::f32::consts::PI * hz * i as f32 / rate).sin();
        pcm[i * 2] = s;
        pcm[i * 2 + 1] = s * 0.5;
    }
    let mut tap = FftTap::new();
    let spec = tap.push(&pcm).expect("one hop after a full window");
    assert_eq!(spec.bins_db.len(), LINEAR_BINS);
    assert!(spec.bins_db[LINEAR_BINS - 1] <= 0.0);
    let (rms, peak) = stereo_meters(&pcm);
    assert!(rms[0] > rms[1]);
    assert!(peak[0] > peak[1]);
    let mut packed = [0f32; 1024];
    packed.copy_from_slice(&pcm[..1024]);
    publish_hop(FrameHop {
        pcm: packed,
        bins_db: spec.bins_db,
        rms_l: rms[0],
        rms_r: rms[1],
        peak_l: peak[0],
        peak_r: peak[1],
        onset: 0.0,
    });
    let pane_w = 76u32;
    let bar_w = 4u32;
    let n = band_count(pane_w, bar_w);
    assert_eq!(n, (pane_w / bar_w) as usize);
    let mut bands = [0.0f32; 32];
    let filled = log_bands(&spec.bins_db, pane_w, bar_w, &mut bands);
    assert_eq!(filled, n);
    assert_eq!(n, (pane_w / bar_w) as usize);
}

#[test]
fn bar_count_follows_pane_and_bar_width() {
    assert_eq!(band_count(76, 4), 76 / 4);
    assert_eq!(band_count(80, 4), 80 / 4);
}
