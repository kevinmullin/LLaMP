use llamp_lyrics_lrclib::{parse_lrclib, query_string};

#[test]
fn query_sends_the_four_fields() {
    let q = query_string("Artist", "Title", "Album", 200);
    assert!(q.contains("track_name=Title"));
    assert!(q.contains("artist_name=Artist"));
    assert!(q.contains("album_name=Album"));
    assert!(q.contains("duration=200"));
}

#[test]
fn synced_lyrics_win_and_instrumental_is_a_miss() {
    let body = r#"{"syncedLyrics":"[00:01.00]Hi\n","plainLyrics":"plain","instrumental":false}"#;
    assert_eq!(parse_lrclib(body).expect("synced"), "[00:01.00]Hi\n");
    assert!(parse_lrclib(r#"{"instrumental":true}"#).is_err());
    assert_eq!(parse_lrclib(r#"{"plainLyrics":"only plain"}"#).expect("plain"), "only plain");
}
