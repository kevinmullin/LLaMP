//! Per-codec decode: frame count and sample rate. No device.

#[path = "fixtures/gen.rs"]
mod fixtures;

use std::path::Path;

use llamp_audio::decode::{self, CodecId};
use llamp_audio::pcm::{self, DOWNMIX_TRIM, EXTRA_CHANNEL_COEFF};

const RATE: u32 = 48_000;
const FRAMES: u64 = 48_000;

#[test]
fn wav_and_aiff_match_source_frame_count_and_rate() {
    let dir = fixtures::dir("pcm");
    let wav = fixtures::write_wav_tone(&dir, "tone.wav", RATE, FRAMES, 440.0);
    let aiff = fixtures::write_aiff_tone(&dir, "tone.aiff", RATE, FRAMES, 440.0);
    assert_decoded(&wav, CodecId::Wav, RATE, FRAMES);
    assert_decoded(&aiff, CodecId::Aiff, RATE, FRAMES);
}

#[test]
fn flac_matches_source_frame_count_and_rate() {
    let dir = fixtures::dir("flac");
    let path = fixtures::write_flac_tone(&dir, "tone.flac", RATE, FRAMES, 440.0);
    assert_decoded(&path.path, CodecId::Flac, RATE, FRAMES);
}

#[test]
fn mp3_vorbis_opus_report_rate_and_encoder_frame_count() {
    let dir = fixtures::dir("lossy");
    let mp3 = fixtures::write_mp3_tone(&dir, "tone.mp3", RATE, FRAMES, 440.0);
    let vorbis = fixtures::write_vorbis_tone(&dir, "tone.ogg", RATE, FRAMES, 440.0);
    let opus = fixtures::write_opus_tone(&dir, "tone.opus", RATE, FRAMES, 440.0);
    assert_decoded(&mp3.path, CodecId::Mp3, RATE, mp3.playable_frames);
    assert_decoded(&vorbis.path, CodecId::Vorbis, RATE, vorbis.playable_frames);
    assert_decoded(&opus.path, CodecId::Opus, RATE, opus.playable_frames);
}

#[test]
fn aac_isomp4_and_adts_both_decode() {
    let dir = fixtures::dir("aac");
    let wav = fixtures::write_wav_tone(&dir, "src.wav", RATE, FRAMES, 440.0);
    let m4a = fixtures::afconvert(&wav, &dir.join("tone.m4a"), &["-f", "m4af", "-d", "aac"]);
    let adts = fixtures::afconvert(&wav, &dir.join("tone.aac"), &["-f", "adts", "-d", "aac"]);
    let iso = decode::decode_path(&m4a).expect("isomp4 aac");
    let raw = decode::decode_path(&adts).expect("adts aac");
    assert_eq!(iso.codec, CodecId::AacLc);
    assert_eq!(raw.codec, CodecId::AacLc);
    assert_eq!(iso.sample_rate, RATE);
    assert_eq!(raw.sample_rate, RATE);
    assert!(iso.frames.len() / 2 > 0);
    assert_eq!(
        iso.frames.len(),
        raw.frames.len(),
        "both containers, one frame count"
    );
}

#[test]
fn downmix_constants_are_locked() {
    assert!((EXTRA_CHANNEL_COEFF - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    assert_eq!(DOWNMIX_TRIM, 1.0);
    // 5.1 order L R C LFE Ls Rs. Extras share one equal-power coefficient.
    // Silent extras must not quiet L/R. The limiter, not this fold, catches peaks above ±1.
    let ch: &[f32] = &[0.5, -0.25, 0.4, 0.1, 0.2, -0.3];
    let out = pcm::fold_to_stereo(ch, 6);
    let extras = 0.4 + 0.1 + 0.2 + -0.3;
    let k = EXTRA_CHANNEL_COEFF;
    assert!((out[0] - DOWNMIX_TRIM * (0.5 + k * extras)).abs() < 1e-5);
    assert!((out[1] - DOWNMIX_TRIM * (-0.25 + k * extras)).abs() < 1e-5);
    let stereo_only = pcm::fold_to_stereo(&[0.5, -0.25, 0.0, 0.0, 0.0, 0.0], 6);
    assert!((stereo_only[0] - 0.5).abs() < 1e-6);
    assert!((stereo_only[1] + 0.25).abs() < 1e-6);
    let mono = pcm::fold_to_stereo(&[0.4], 1);
    assert_eq!(mono, vec![0.4, 0.4]);
}

fn assert_decoded(path: &Path, codec: CodecId, rate: u32, frames: u64) {
    let decoded =
        decode::decode_path(path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    assert_eq!(decoded.codec, codec);
    assert_eq!(decoded.sample_rate, rate);
    assert_eq!(decoded.frames.len() / 2, frames as usize);
}
