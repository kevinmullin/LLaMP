//! Seek lands within one device period. Gapless join is within one sample of the trimmed length.

#[path = "fixtures/gen.rs"]
mod fixtures;

use fixtures::Encoded;
use llamp_audio::graph::{self, DEVICE_PERIOD_MAX, DEVICE_PERIOD_MIN};

const RATE: u32 = 48_000;
const PERIOD: u32 = 256;
/// Shorter than Opus pre-skip and a typical Vorbis granule excess, so a missed trim
/// leaves this window on the wrong side of the encoder delay.
const BOUNDARY: usize = 128;

#[test]
fn seek_lands_within_one_device_period() {
    assert!(PERIOD >= DEVICE_PERIOD_MIN && PERIOD <= DEVICE_PERIOD_MAX);
    let dir = fixtures::dir("seek");
    let frames = 48_000u64;
    let path = fixtures::write_ramp_wav(&dir, "ramp.wav", RATE, frames);
    let target = 20_000u64;
    let landing = graph::seek_landing(&path, target, PERIOD).expect("seek");
    let err = (landing.frame as i64 - target as i64).abs();
    assert!(
        err <= PERIOD as i64,
        "landed at {} , target {target}, period {PERIOD}",
        landing.frame
    );
    let expected = fixtures::ramp_sample(landing.frame);
    assert!(
        (landing.first_sample - expected).abs() < 1e-5,
        "audio at landing {} is {}, expected {}",
        landing.frame,
        landing.first_sample,
        expected
    );
}

#[test]
fn gapless_join_within_one_sample() {
    let dir = fixtures::dir("gapless");
    let n = 48_000u64;
    let wav_a = fixtures::write_wav_tone(&dir, "src.wav", RATE, n, 440.0);
    let wav_b = fixtures::write_wav_tone(&dir, "src-b.wav", RATE, n, 880.0);
    let pairs = [
        (
            fixtures::write_flac_tone(&dir, "a.flac", RATE, n, 440.0),
            fixtures::write_flac_tone(&dir, "b.flac", RATE, n, 880.0),
            true,
        ),
        (
            fixtures::write_vorbis_tone(&dir, "a.ogg", RATE, n, 440.0),
            fixtures::write_vorbis_tone(&dir, "b.ogg", RATE, n, 880.0),
            false,
        ),
        (
            fixtures::write_opus_tone(&dir, "a.opus", RATE, n, 440.0),
            fixtures::write_opus_tone(&dir, "b.opus", RATE, n, 880.0),
            false,
        ),
        (
            fixtures::write_alac_tone(&dir, "a.m4a", &wav_a),
            fixtures::write_alac_tone(&dir, "b.m4a", &wav_b),
            true,
        ),
    ];
    for (a, b, lossless) in pairs {
        assert_gapless_boundary(&a, &b, 440.0, 880.0, lossless);
    }
}

fn assert_gapless_boundary(a: &Encoded, b: &Encoded, hz_a: f32, hz_b: f32, lossless: bool) {
    let joined = graph::join_gapless(&a.path, &b.path).expect("join");
    let got = joined.len() / 2;
    // Encoder playable counts, not gapless_info (that is the same decode).
    let expected = a.playable_frames + b.playable_frames;
    let delta = (got as i64 - expected as i64).abs();
    assert!(
        delta <= 1,
        "{} join {got} vs encoder playable {expected} (delay {:?} padding {:?})",
        a.path.display(),
        a.encoder_delay,
        a.encoder_padding
    );
    let extra = encoder_extra(a) + encoder_extra(b);
    if extra > 1 {
        let untrimmed = expected + u64::from(extra);
        let leftover = (got as i64 - untrimmed as i64).abs();
        assert!(
            leftover > 1,
            "{} join still includes encoder delay {extra}",
            a.path.display()
        );
    }
    let at = a.playable_frames as usize;
    assert!(at >= BOUNDARY && got >= at + BOUNDARY, "{} too short to see the boundary", a.path.display());
    let before = &joined[(at - BOUNDARY) * 2..at * 2];
    let after = &joined[at * 2..(at + BOUNDARY) * 2];
    let a_before = goertzel(before, hz_a);
    let b_before = goertzel(before, hz_b);
    let a_after = goertzel(after, hz_a);
    let b_after = goertzel(after, hz_b);
    assert!(
        a_before > b_before * 4.0,
        "{} sample before encoder boundary {} is not {} Hz ({} vs {})",
        a.path.display(),
        at,
        hz_a,
        a_before,
        b_before
    );
    assert!(
        b_after > a_after * 4.0,
        "{} sample after encoder delay boundary {} is not {} Hz ({} vs {})",
        a.path.display(),
        at,
        hz_b,
        b_after,
        a_after
    );
    if lossless {
        let end = fixtures::source_sample(RATE, a.playable_frames - 1, hz_a);
        let start = fixtures::source_sample(RATE, 0, hz_b);
        let quant = 1.5 / 32767.0;
        assert!(
            (joined[(at - 1) * 2] - end).abs() < quant,
            "{} last sample {} != source {}",
            a.path.display(),
            joined[(at - 1) * 2],
            end
        );
        assert!(
            (joined[at * 2] - start).abs() < quant,
            "{} first sample after delay {} != source {}",
            a.path.display(),
            joined[at * 2],
            start
        );
    }
}

fn encoder_extra(encoded: &Encoded) -> u32 {
    encoded.encoder_delay.unwrap_or(0).saturating_add(encoded.encoder_padding.unwrap_or(0))
}

fn goertzel(interleaved: &[f32], hz: f32) -> f32 {
    let n = interleaved.len() / 2;
    let w = 2.0 * std::f32::consts::PI * hz / RATE as f32;
    let coeff = 2.0 * w.cos();
    let mut s1 = 0.0f32;
    let mut s2 = 0.0f32;
    for i in 0..n {
        let s0 = interleaved[i * 2] + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - coeff * s1 * s2
}

#[test]
fn mp3_gapless_uses_surfaced_lame_fields_or_is_absent() {
    let dir = fixtures::dir("mp3-gapless");
    let made = fixtures::write_mp3_tone(&dir, "a.mp3", RATE, 48_000, 440.0);
    let info = graph::gapless_info(&made.path).expect("mp3 info");
    let (Some(raw_delay), Some(raw_padding)) = (made.encoder_delay, made.encoder_padding) else {
        panic!("fixture encoder did not write a LAME delay; cannot investigate");
    };
    let delay_ok = info.delay == Some(raw_delay) || info.delay == Some(raw_delay.saturating_add(529));
    let padding_ok = info.padding == Some(raw_padding)
        || info.padding == Some(raw_padding.saturating_sub(529));
    assert!(
        delay_ok && padding_ok,
        "symphonia did not surface the LAME tag (raw delay {raw_delay}, raw padding {raw_padding}); got delay {:?} padding {:?}",
        info.delay,
        info.padding
    );
}

#[test]
fn aac_is_not_trimmed() {
    let dir = fixtures::dir("aac-gapless");
    let wav = fixtures::write_wav_tone(&dir, "src.wav", RATE, 48_000, 440.0);
    let m4a = fixtures::afconvert(&wav, &dir.join("tone.m4a"), &["-f", "m4af", "-d", "aac"]);
    let info = graph::gapless_info(&m4a).expect("aac");
    assert!(
        info.aac_not_gapless,
        "AAC-LC must be documented as not gapless and must not be trimmed by guesswork"
    );
}
