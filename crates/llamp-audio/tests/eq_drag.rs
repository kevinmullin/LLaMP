//! A window slider drag must take the same slew path as `Eq::set_band_db`.
//! Calling `set_band_db` here would miss the phase 4 trap.

#[test]
fn ui_slider_drag_crossfades_instead_of_stepping() {
    let _guard = lock();
    let rate = 48_000u32;
    let mut stage = llamp_audio::playback_stage(rate);
    llamp_audio::set_eq_enabled(true);
    llamp_audio::drag_band(4, 0.0);
    let mut silence = vec![0f32; rate as usize * 2];
    stage.eq.process(&mut silence);

    let n = rate as usize / 10;
    let mut tone = vec![0f32; n * 2];
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.2;
        tone[i * 2] = s;
        tone[i * 2 + 1] = s;
    }
    let mut before = tone.clone();
    stage.eq.process(&mut before);
    let before_rms = rms(&before[..256]);

    llamp_audio::drag_band(4, 12.0);

    let mut after = tone;
    stage.eq.process(&mut after);
    let early = rms(&after[..256]);
    let late = rms(&after[after.len() - 2048..]);
    let step = before_rms * 10f32.powf(12.0 / 20.0);
    assert!(
        early < before_rms + 0.5 * (step - before_rms),
        "first 256 samples already stepped: early {early}, before {before_rms}, step {step}"
    );
    assert!(
        late > early * 1.5,
        "envelope did not rise across the buffer: early {early}, late {late}"
    );
}

#[test]
fn cli_playback_band_sweep_is_audible() {
    let _guard = lock();
    let dir = std::env::temp_dir().join("llamp-eq-cli-sweep");
    std::fs::create_dir_all(&dir).expect("sweep dir");
    let path = dir.join("sweep.wav");
    let rate = 48_000u32;
    let pcm = sweep_pcm(rate);
    llamp_audio::wav::write_wav(&path, rate, &pcm, llamp_audio::wav::WavBits::F32)
        .expect("write sweep");
    let decoded = llamp_audio::decode::decode_path(&path).expect("llamp play decodes this file");
    assert_eq!(decoded.sample_rate, rate);
    let ratios =
        llamp_audio::band_sweep_ratios(&decoded.frames, decoded.sample_rate, |band, db| {
            if db >= 12.0 {
                llamp_audio::set_eq_sweep_band(band);
            } else {
                llamp_audio::drag_band(band, db);
            }
        });
    for (band, ratio) in ratios.iter().enumerate() {
        assert!(
            *ratio > 2.0,
            "CLI play graph did not make band {band} audible: ratio {ratio}"
        );
    }
    for frame in [0usize, 4095, 4096, 8191] {
        let want = frame / 8192;
        assert_eq!(llamp_audio::sweep_band_at(frame, 81_920), want);
    }
}

fn sweep_pcm(rate: u32) -> Vec<f32> {
    let per = 8192usize;
    let mut pcm = Vec::with_capacity(per * 10 * 2);
    for hz in llamp_audio::eq::BAND_HZ {
        for i in 0..per {
            let t = i as f32 / rate as f32;
            let s = (2.0 * std::f32::consts::PI * hz * t).sin() * 0.15;
            pcm.push(s);
            pcm.push(s);
        }
    }
    pcm
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap()
}

fn rms(samples: &[f32]) -> f32 {
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}
