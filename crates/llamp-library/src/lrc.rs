//! LRC and enhanced LRC. Malformed lines are skipped.

use crate::lyrics::{LyricDoc, LyricKind, LyricLine, LyricSource, LyricTags, LyricWord};

pub fn parse_lrc(text: &str) -> Option<LyricDoc> {
    let mut tags = LyricTags::default();
    let mut offset_ms = 0i32;
    let mut lines = Vec::new();
    let mut plain = Vec::new();
    let mut any_time = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = id_tag(line) {
            match key {
                "ar" => tags.artist = value,
                "ti" => tags.title = value,
                "al" => tags.album = value,
                "length" => tags.length = value,
                "offset" => {
                    if let Some(ms) = parse_offset(&value) {
                        offset_ms = ms;
                    }
                }
                _ => {}
            }
            continue;
        }
        match timed_line(line) {
            Some(mut copies) => {
                any_time = true;
                for item in &mut copies {
                    item.start_ms = apply_offset(item.start_ms, offset_ms);
                    for word in &mut item.words {
                        word.start_ms = apply_offset(word.start_ms, offset_ms);
                    }
                }
                lines.extend(copies);
            }
            None if line.starts_with('[') => continue,
            None => plain.push(line.to_string()),
        }
    }

    if any_time {
        lines.sort_by_key(|line| line.start_ms);
        Some(LyricDoc {
            kind: LyricKind::Synced,
            source: LyricSource::Sidecar,
            tags,
            offset_ms,
            lines,
            plain: String::new(),
        })
    } else if !plain.is_empty() {
        Some(LyricDoc {
            kind: LyricKind::Unsynced,
            source: LyricSource::Sidecar,
            tags,
            offset_ms,
            lines: Vec::new(),
            plain: plain.join("\n"),
        })
    } else {
        None
    }
}

pub fn write_lrc(doc: &LyricDoc, offset_ms: i32) -> String {
    let mut out = String::new();
    if offset_ms != 0 {
        out.push_str(&format!("[offset:{offset_ms:+}]\n"));
    }
    if !doc.tags.artist.is_empty() {
        out.push_str(&format!("[ar:{}]\n", doc.tags.artist));
    }
    if !doc.tags.title.is_empty() {
        out.push_str(&format!("[ti:{}]\n", doc.tags.title));
    }
    if !doc.tags.album.is_empty() {
        out.push_str(&format!("[al:{}]\n", doc.tags.album));
    }
    match doc.kind {
        LyricKind::Synced => {
            for line in &doc.lines {
                let start = unapply_offset(line.start_ms, doc.offset_ms);
                out.push_str(&format!("[{}]{}", format_stamp(start), line_body(line, doc.offset_ms)));
                out.push('\n');
            }
        }
        LyricKind::Unsynced => {
            out.push_str(&doc.plain);
            if !doc.plain.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

fn line_body(line: &LyricLine, file_offset: i32) -> String {
    if line.words.is_empty() {
        return line.text.clone();
    }
    let mut body = String::new();
    for (index, word) in line.words.iter().enumerate() {
        if index > 0 {
            let start = unapply_offset(word.start_ms, file_offset);
            body.push_str(&format!(" <{}>", format_stamp(start)));
        }
        if index > 0 {
            body.push(' ');
        }
        body.push_str(&word.text);
    }
    body
}

fn id_tag(line: &str) -> Option<(&str, String)> {
    let rest = line.strip_prefix('[')?.strip_suffix(']')?;
    let (key, value) = rest.split_once(':')?;
    if key.chars().all(|c| c.is_ascii_alphabetic()) && timestamp(rest).is_none() {
        Some((key, value.trim().to_string()))
    } else {
        None
    }
}

fn timed_line(line: &str) -> Option<Vec<LyricLine>> {
    let mut rest = line;
    let mut starts = Vec::new();
    while let Some((ms, next)) = leading_stamp(rest, '[', ']') {
        starts.push(ms);
        rest = next;
    }
    if starts.is_empty() {
        return None;
    }
    let (text, words) = words(rest.trim(), starts[0]);
    if text.is_empty() && words.is_empty() {
        return None;
    }
    Some(
        starts
            .into_iter()
            .map(|start_ms| LyricLine {
                start_ms,
                text: text.clone(),
                words: words.clone(),
            })
            .collect(),
    )
}

fn words(text: &str, line_start: u32) -> (String, Vec<LyricWord>) {
    if !text.contains('<') {
        return (text.to_string(), Vec::new());
    }
    let mut words = Vec::new();
    let mut display = String::new();
    let mut rest = text;
    let mut next_start = line_start;
    while !rest.is_empty() {
        if let Some((ms, after)) = leading_stamp(rest.trim_start(), '<', '>') {
            next_start = ms;
            rest = after;
            continue;
        }
        let split = rest.find('<').unwrap_or(rest.len());
        let chunk = rest[..split].trim();
        rest = &rest[split..];
        if chunk.is_empty() {
            if rest.starts_with('<') && leading_stamp(rest, '<', '>').is_none() {
                break;
            }
            continue;
        }
        if !display.is_empty() {
            display.push(' ');
        }
        display.push_str(chunk);
        words.push(LyricWord {
            start_ms: next_start,
            text: chunk.to_string(),
        });
    }
    if words.is_empty() {
        (text.to_string(), Vec::new())
    } else {
        (display, words)
    }
}

fn leading_stamp(text: &str, open: char, close: char) -> Option<(u32, &str)> {
    let rest = text.strip_prefix(open)?;
    let end = rest.find(close)?;
    let ms = timestamp(&rest[..end])?;
    Some((ms, &rest[end + 1..]))
}

fn timestamp(text: &str) -> Option<u32> {
    let (min, sec) = text.split_once(':')?;
    if min.is_empty() || sec.is_empty() || min.len() > 3 {
        return None;
    }
    let minutes: u32 = min.parse().ok()?;
    let (seconds, frac) = match sec.split_once('.') {
        Some((s, f)) => (s, f),
        None => (sec, ""),
    };
    if seconds.len() != 2 {
        return None;
    }
    let seconds: u32 = seconds.parse().ok()?;
    if seconds > 59 {
        return None;
    }
    let millis = match frac.len() {
        0 => 0,
        1 => frac.parse::<u32>().ok()? * 100,
        2 => frac.parse::<u32>().ok()? * 10,
        3 => frac.parse::<u32>().ok()?,
        _ => return None,
    };
    Some(minutes * 60_000 + seconds * 1000 + millis)
}

fn parse_offset(value: &str) -> Option<i32> {
    let value = value.trim().trim_end_matches("ms").trim();
    value.parse().ok()
}

fn apply_offset(ms: u32, offset: i32) -> u32 {
    (i64::from(ms) + i64::from(offset)).max(0) as u32
}

fn unapply_offset(ms: u32, offset: i32) -> u32 {
    (i64::from(ms) - i64::from(offset)).max(0) as u32
}

fn format_stamp(ms: u32) -> String {
    let minutes = ms / 60_000;
    let seconds = (ms / 1000) % 60;
    let frac = ms % 1000;
    if frac == 0 {
        format!("{minutes:02}:{seconds:02}")
    } else if frac % 10 == 0 {
        format!("{minutes:02}:{seconds:02}.{:02}", frac / 10)
    } else {
        format!("{minutes:02}:{seconds:02}.{frac:03}")
    }
}
