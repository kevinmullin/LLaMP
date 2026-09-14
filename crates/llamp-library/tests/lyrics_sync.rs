//! 8b — active line and word follow the decoder clock, not the wall.

use llamp_library::{active_at, clock_ms, parse_lrc};

fn karaoke() -> llamp_library::LyricDoc {
    parse_lrc("[00:00.50]One\n[00:01.00]Two <00:01.40>words\n").expect("lrc")
}

#[test]
fn seek_10ms_before_and_after_a_line() {
    let doc = karaoke();
    let before = active_at(&doc, 990);
    assert_eq!(before.line, Some(0), "10 ms before the second line stays on the first");
    let after = active_at(&doc, 1010);
    assert_eq!(after.line, Some(1), "10 ms after the second line must select it");
}

#[test]
fn word_highlight_snaps_without_interpolation() {
    let doc = karaoke();
    let first = active_at(&doc, 1200);
    assert_eq!(first.line, Some(1));
    assert_eq!(first.word, Some(0));
    let second = active_at(&doc, 1400);
    assert_eq!(second.word, Some(1));
}

#[test]
fn pause_uses_a_frozen_clock() {
    let doc = karaoke();
    let paused = clock_ms(48_000, 48_000, 0);
    assert_eq!(paused, 1000);
    assert_eq!(active_at(&doc, paused).line, Some(1));
    let still = clock_ms(48_000, 48_000, 0);
    assert_eq!(still, paused, "the same decoder frame count must not advance the line");
}

#[test]
fn sample_rate_change_keeps_the_same_millisecond() {
    let at_44k = clock_ms(44_100, 44_100, 0);
    let at_48k = clock_ms(48_000, 48_000, 0);
    assert_eq!(at_44k, 1000);
    assert_eq!(at_48k, 1000);
    let doc = karaoke();
    assert_eq!(active_at(&doc, at_44k).line, active_at(&doc, at_48k).line);
}

#[test]
fn user_offset_shifts_the_clock() {
    let doc = karaoke();
    assert_eq!(active_at(&doc, clock_ms(48_000, 48_000, 50)).line, Some(1));
    assert_eq!(active_at(&doc, clock_ms(48_000, 48_000, -50)).line, Some(0));
}
