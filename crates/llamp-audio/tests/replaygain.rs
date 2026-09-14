//! ReplayGain 2.0 through the tag trait. Field names must round-trip; do not synthesize them.

#[path = "fixtures/gen.rs"]
mod fixtures;

use llamp_audio::tags::{self, AppliedGain, GainTags, LoftySource, ReplayGainSource};

#[test]
fn lofty_replaygain_field_names_round_trip() {
    let dir = fixtures::dir("rg");
    let path = fixtures::write_wav_tone(&dir, "tagged.wav", 48_000, 4_800, 440.0);
    fixtures::write_replaygain(&path, "-6.5 dB", "0.9", "-8.25 dB", "1.0", "album-1");
    let tags = LoftySource.read(&path).expect("read tags");
    assert_eq!(
        tags.track_gain_db,
        Some(-6.5),
        "REPLAYGAIN_TRACK_GAIN did not round-trip"
    );
    assert!((tags.track_peak.unwrap() - 0.9).abs() < 1e-4);
    assert_eq!(tags.album_gain_db, Some(-8.25));
    assert!((tags.album_peak.unwrap() - 1.0).abs() < 1e-4);
    assert_eq!(tags.album_id.as_deref(), Some("album-1"));
}

#[test]
fn album_when_shared_otherwise_track_and_peak_caps() {
    let track = GainTags {
        track_gain_db: Some(-6.0),
        track_peak: Some(1.0),
        album_gain_db: Some(-3.0),
        album_peak: Some(1.0),
        album_id: Some("same".into()),
        v2: true,
        v1_gain_db: Some(-12.0),
    };
    let next = track.clone();
    let applied = tags::select_gain(&track, Some(&next), 0.0);
    assert!(applied.used_album);
    assert!(applied.used_replaygain);
    assert!((applied.db - -3.0).abs() < 1e-4);

    let other = GainTags {
        album_id: Some("other".into()),
        ..next
    };
    let applied = tags::select_gain(&track, Some(&other), 0.0);
    assert!(!applied.used_album);
    assert!((applied.db - -6.0).abs() < 1e-4);

    let v2 = tags::select_gain(&track, None, 0.0);
    assert!((v2.db - -6.0).abs() < 1e-4, "v2 wins over v1");

    let hot = GainTags {
        track_gain_db: Some(6.0),
        track_peak: Some(1.0),
        album_gain_db: None,
        album_peak: None,
        album_id: None,
        v2: true,
        v1_gain_db: None,
    };
    let capped = tags::select_gain(&hot, None, 0.0);
    let ceiling_db = -1.0;
    assert!(
        capped.db <= ceiling_db + 1e-3,
        "peak tag must cap applied gain at -1 dBFS, got {}",
        capped.db
    );

    let missing = tags::select_gain(&GainTags::default(), None, 0.0);
    assert_eq!(
        missing,
        AppliedGain {
            db: 0.0,
            used_replaygain: false,
            used_album: false
        }
    );
}
