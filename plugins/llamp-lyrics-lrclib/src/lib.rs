//! LRCLIB LyricsProvider. Query build and JSON parse. The host is the only socket.

use std::fmt::Write;

pub const HOST: &str = "lrclib.net";
pub const PATH: &str = "/api/get";

pub fn query_string(artist: &str, title: &str, album: &str, duration_seconds: u32) -> String {
    let mut out = String::new();
    push_pair(&mut out, "track_name", title);
    push_pair(&mut out, "artist_name", artist);
    push_pair(&mut out, "album_name", album);
    let _ = write!(out, "&duration={duration_seconds}");
    out
}

pub fn parse_lrclib(body: &str) -> Result<String, String> {
    if field(body, "instrumental").is_some_and(|v| v == "true") {
        return Err("instrumental".into());
    }
    if let Some(synced) = json_string(body, "syncedLyrics") {
        if !synced.is_empty() {
            return Ok(synced);
        }
    }
    if let Some(plain) = json_string(body, "plainLyrics") {
        if !plain.is_empty() {
            return Ok(plain);
        }
    }
    Err("no match".into())
}

fn push_pair(out: &mut String, key: &str, value: &str) {
    if !out.is_empty() {
        out.push('&');
    }
    out.push_str(key);
    out.push('=');
    for ch in value.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(ch),
            ' ' => out.push_str("%20"),
            _ => {
                let mut buf = [0u8; 4];
                for byte in ch.encode_utf8(&mut buf).as_bytes() {
                    let _ = write!(out, "%{byte:02X}");
                }
            }
        }
    }
}

fn field(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let rest = body.split(&needle).nth(1)?;
    let rest = rest.trim_start_matches([' ', ':', '\t']);
    if rest.starts_with("true") {
        return Some("true".into());
    }
    if rest.starts_with("false") {
        return Some("false".into());
    }
    json_string(body, key)
}

fn json_string(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let rest = body.split(&needle).nth(1)?;
    let rest = rest.trim_start_matches([' ', ':', '\t']);
    if rest.starts_with("null") {
        return None;
    }
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => break,
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    Some(out)
}

#[cfg(target_arch = "wasm32")]
mod guest {
    use super::{parse_lrclib, query_string, HOST, PATH};

    wit_bindgen::generate!({
        world: "lyrics",
        path: "../../crates/llamp-plugin-host/wit/lyrics.wit",
    });

    use exports::llamp::lyrics::lyrics_provider::Guest;

    struct Component;

    export!(Component);

    impl Guest for Component {
        fn fetch(artist: String, title: String) -> Result<String, String> {
            Self::fetch_match(artist, title, String::new(), 0)
        }

        fn fetch_match(
            artist: String,
            title: String,
            album: String,
            duration_seconds: u32,
        ) -> Result<String, String> {
            let query = query_string(&artist, &title, &album, duration_seconds);
            let body = llamp::lyrics::host_http::get(HOST, PATH, &query)?;
            parse_lrclib(&body)
        }
    }
}
