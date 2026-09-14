//! Lyrics lookup chain, tag extract, cache, and decoder-clock highlight.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lofty::file::TaggedFileExt;
use lofty::id3::v2::{Frame, FrameFlags, Id3v2Tag, SynchronizedTextFrame, TimestampFormat};
use lofty::tag::{ItemKey, Tag, TagType};
use rusqlite::{params, OptionalExtension};

use crate::lrc::{parse_lrc, write_lrc};
use crate::{Library, LibraryInner, Storage};

pub const MISS_RETRY_SECS: i64 = 30 * 24 * 60 * 60;
pub const OFFSET_MIN_MS: i32 = -10_000;
pub const OFFSET_MAX_MS: i32 = 10_000;
pub const OFFSET_STEP_MS: i32 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricKind {
    Synced,
    Unsynced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricSource {
    Embedded,
    Sidecar,
    Remote,
    Cache,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LyricTags {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub length: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricWord {
    pub start_ms: u32,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricLine {
    pub start_ms: u32,
    pub text: String,
    pub words: Vec<LyricWord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricDoc {
    pub kind: LyricKind,
    pub source: LyricSource,
    pub tags: LyricTags,
    pub offset_ms: i32,
    pub lines: Vec<LyricLine>,
    pub plain: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LrclibConsent {
    Unknown,
    Accept,
    Decline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LyricQuery {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub duration_seconds: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LyricCursor {
    pub line: Option<usize>,
    pub word: Option<usize>,
}

pub fn lyrics_from_tag(tag: &Tag, id3: Option<&Id3v2Tag>) -> Option<LyricDoc> {
    if let Some(id3) = id3 {
        if let Some(doc) = sylt_from_id3(id3) {
            return Some(doc);
        }
        if let Some(uslt) = first_uslt(id3) {
            return Some(unsynced(uslt, LyricSource::Embedded));
        }
    }
    if let Some(text) = tag.get_string(ItemKey::UnsyncLyrics) {
        if !text.is_empty() {
            return Some(unsynced(text, LyricSource::Embedded));
        }
    }
    if let Some(text) = tag.get_string(ItemKey::Lyrics) {
        if !text.is_empty() {
            return Some(unsynced(text, LyricSource::Embedded));
        }
    }
    None
}

pub fn read_embedded(path: &Path) -> Option<LyricDoc> {
    let tagged = lofty::read_from_path(path).ok()?;
    let id3 = tagged
        .tag(TagType::Id3v2)
        .cloned()
        .map(Id3v2Tag::from);
    if let Some(tag) = tagged.primary_tag() {
        let mut doc = lyrics_from_tag(tag, id3.as_ref())?;
        doc.source = LyricSource::Embedded;
        Some(doc)
    } else if let Some(id3) = id3.as_ref() {
        lyrics_from_tag(&Tag::new(TagType::Id3v2), Some(id3)).map(|mut doc| {
            doc.source = LyricSource::Embedded;
            doc
        })
    } else {
        None
    }
}

pub fn resolve_lyrics(path: &Path, managed_dir: Option<&Path>) -> Option<LyricDoc> {
    let embedded = read_embedded(path);
    let sidecar = read_sidecar(path, managed_dir);
    match (embedded, sidecar) {
        (Some(emb), Some(side)) if emb.kind == LyricKind::Unsynced && side.kind == LyricKind::Synced => {
            Some(side)
        }
        (Some(emb), _) => Some(emb),
        (None, Some(side)) => Some(side),
        (None, None) => None,
    }
}

pub fn read_sidecar(path: &Path, managed_dir: Option<&Path>) -> Option<LyricDoc> {
    let dir = sidecar_dir(path, managed_dir)?;
    let stem = path.file_stem()?.to_string_lossy();
    let lrc = dir.join(format!("{stem}.lrc"));
    if let Some(doc) = read_sidecar_file(&lrc) {
        return Some(doc);
    }
    let txt = dir.join(format!("{stem}.txt"));
    let doc = read_sidecar_file(&txt)?;
    if doc.kind == LyricKind::Synced {
        Some(doc)
    } else {
        None
    }
}

pub fn clock_ms(position_frames: u64, sample_rate: u32, user_offset_ms: i32) -> i64 {
    let rate = i64::from(sample_rate.max(1));
    (position_frames as i64 * 1000) / rate + i64::from(user_offset_ms)
}

pub fn active_at(doc: &LyricDoc, clock_ms: i64) -> LyricCursor {
    if doc.kind != LyricKind::Synced {
        return LyricCursor {
            line: None,
            word: None,
        };
    }
    let line = doc
        .lines
        .iter()
        .enumerate()
        .rev()
        .find(|(_, item)| i64::from(item.start_ms) <= clock_ms)
        .map(|(index, _)| index);
    let word = line.and_then(|index| {
        let item = &doc.lines[index];
        if item.words.is_empty() {
            return None;
        }
        item.words
            .iter()
            .enumerate()
            .rev()
            .find(|(_, word)| i64::from(word.start_ms) <= clock_ms)
            .map(|(word, _)| word)
    });
    LyricCursor { line, word }
}

pub fn clamp_offset(ms: i32) -> i32 {
    ms.clamp(OFFSET_MIN_MS, OFFSET_MAX_MS)
}

fn unsynced(text: &str, source: LyricSource) -> LyricDoc {
    LyricDoc {
        kind: LyricKind::Unsynced,
        source,
        tags: LyricTags::default(),
        offset_ms: 0,
        lines: Vec::new(),
        plain: text.trim().to_string(),
    }
}

fn first_uslt(id3: &Id3v2Tag) -> Option<&str> {
    id3.unsync_text()
        .map(|frame| frame.content.as_ref())
        .find(|text| !text.is_empty())
}

fn sylt_from_id3(id3: &Id3v2Tag) -> Option<LyricDoc> {
    for frame in id3.iter() {
        let Frame::Binary(bin) = frame else {
            continue;
        };
        if bin.id().as_str() != "SYLT" {
            continue;
        }
        let parsed = SynchronizedTextFrame::parse(&bin.data, FrameFlags::default()).ok()?;
        if parsed.timestamp_format != TimestampFormat::MS {
            continue;
        }
        if parsed.content.is_empty() {
            continue;
        }
        let lines: Vec<LyricLine> = parsed
            .content
            .iter()
            .map(|(ms, text)| LyricLine {
                start_ms: *ms,
                text: text.trim().to_string(),
                words: Vec::new(),
            })
            .filter(|line| !line.text.is_empty())
            .collect();
        if lines.is_empty() {
            continue;
        }
        return Some(LyricDoc {
            kind: LyricKind::Synced,
            source: LyricSource::Embedded,
            tags: LyricTags::default(),
            offset_ms: 0,
            lines,
            plain: String::new(),
        });
    }
    None
}

fn sidecar_dir(path: &Path, managed_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(managed) = managed_dir {
        return Some(managed.to_path_buf());
    }
    path.parent().map(Path::to_path_buf)
}

fn read_sidecar_file(path: &Path) -> Option<LyricDoc> {
    let text = fs::read_to_string(path).ok()?;
    let mut doc = parse_lrc(&text)?;
    doc.source = LyricSource::Sidecar;
    Some(doc)
}

impl Library {
    pub fn lyrics_consent(&self) -> Result<LrclibConsent, String> {
        let guard = self.lock()?;
        let value: Option<String> = guard
            .conn
            .query_row(
                "SELECT value FROM lyrics_prefs WHERE key = 'lrclib_opt_in'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())?;
        Ok(match value.as_deref() {
            Some("accept") => LrclibConsent::Accept,
            Some("decline") => LrclibConsent::Decline,
            _ => LrclibConsent::Unknown,
        })
    }

    pub fn set_lyrics_consent(&self, consent: LrclibConsent) -> Result<(), String> {
        let value = match consent {
            LrclibConsent::Accept => "accept",
            LrclibConsent::Decline => "decline",
            LrclibConsent::Unknown => return self.clear_lyrics_consent(),
        };
        let guard = self.lock()?;
        guard
            .conn
            .execute(
                "INSERT INTO lyrics_prefs(key, value) VALUES('lrclib_opt_in', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [value],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn clear_lyrics_consent(&self) -> Result<(), String> {
        let guard = self.lock()?;
        guard
            .conn
            .execute("DELETE FROM lyrics_prefs WHERE key = 'lrclib_opt_in'", [])
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn cache_lyrics(
        &self,
        path: Option<&Path>,
        query: &LyricQuery,
        body: Option<&str>,
        source: LyricSource,
        kind: Option<LyricKind>,
    ) -> Result<(), String> {
        let guard = self.lock()?;
        let track_id = match path {
            Some(path) => guard.track_id(path)?,
            None => None,
        };
        let now = unix_now();
        let kind = match (body, kind) {
            (None, _) => "miss",
            (Some(_), Some(LyricKind::Synced)) => "synced",
            (Some(_), _) => "unsynced",
        };
        let source = match source {
            LyricSource::Remote => "remote",
            LyricSource::Cache => "cache",
            LyricSource::Sidecar => "sidecar",
            LyricSource::Embedded => "embedded",
        };
        guard
            .conn
            .execute(
                "INSERT INTO lyrics_cache(
                    track_id, artist, title, album, duration_seconds, body, source, kind, fetched_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(artist, title, album, duration_seconds) DO UPDATE SET
                    track_id = excluded.track_id,
                    body = excluded.body,
                    source = excluded.source,
                    kind = excluded.kind,
                    fetched_at = excluded.fetched_at",
                params![
                    track_id,
                    query.artist,
                    query.title,
                    query.album,
                    query.duration_seconds as i64,
                    body,
                    source,
                    kind,
                    now
                ],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn cached_lyrics(&self, path: Option<&Path>, query: &LyricQuery) -> Result<Option<LyricDoc>, String> {
        let guard = self.lock()?;
        let track_id = match path {
            Some(path) => guard.track_id(path)?,
            None => None,
        };
        let row = guard.lookup_cache(track_id, query)?;
        let Some((body, kind, fetched_at)) = row else {
            return Ok(None);
        };
        if kind == "miss" {
            let age = unix_now().saturating_sub(fetched_at);
            if age < MISS_RETRY_SECS {
                return Ok(None);
            }
            return Ok(None);
        }
        let Some(body) = body else {
            return Ok(None);
        };
        let mut doc = parse_lrc(&body).unwrap_or_else(|| unsynced(&body, LyricSource::Cache));
        doc.source = LyricSource::Cache;
        Ok(Some(doc))
    }

    pub fn cached_miss_fresh(&self, path: Option<&Path>, query: &LyricQuery) -> Result<bool, String> {
        let guard = self.lock()?;
        let track_id = match path {
            Some(path) => guard.track_id(path)?,
            None => None,
        };
        let Some((_, kind, fetched_at)) = guard.lookup_cache(track_id, query)? else {
            return Ok(false);
        };
        Ok(kind == "miss" && unix_now().saturating_sub(fetched_at) < MISS_RETRY_SECS)
    }

    pub fn save_offset(
        &self,
        path: &Path,
        doc: &LyricDoc,
        user_offset_ms: i32,
        managed_dir: Option<&Path>,
    ) -> Result<OffsetSave, String> {
        let offset = clamp_offset(doc.offset_ms + user_offset_ms);
        let text = write_lrc(doc, offset);
        let dir = sidecar_dir(path, managed_dir).ok_or("no sidecar directory")?;
        let stem = path
            .file_stem()
            .ok_or("no stem")?
            .to_string_lossy();
        let sidecar = dir.join(format!("{stem}.lrc"));
        if writable_dir(&dir) {
            fs::write(&sidecar, text).map_err(|err| err.to_string())?;
            return Ok(OffsetSave::Sidecar(sidecar));
        }
        let guard = self.lock()?;
        let id = guard.track_id(path)?;
        guard
            .conn
            .execute(
                "INSERT INTO lyrics_offsets(track_id, path, offset_ms) VALUES(?1, ?2, ?3)
                 ON CONFLICT(path) DO UPDATE SET offset_ms = excluded.offset_ms, track_id = excluded.track_id",
                params![id, path.to_string_lossy().as_ref(), offset],
            )
            .map_err(|err| err.to_string())?;
        Ok(OffsetSave::Sqlite)
    }

    pub fn stored_offset(&self, path: &Path) -> Result<Option<i32>, String> {
        let guard = self.lock()?;
        guard
            .conn
            .query_row(
                "SELECT offset_ms FROM lyrics_offsets WHERE path = ?1",
                [path.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())
    }

    pub fn storage_kind(&self, path: &Path) -> Result<Option<Storage>, String> {
        Ok(self.storage_of(path)?.and_then(|s| match s.as_str() {
            "referenced" => Some(Storage::Referenced),
            "managed" => Some(Storage::Managed),
            _ => None,
        }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OffsetSave {
    Sidecar(PathBuf),
    Sqlite,
}

impl LibraryInner {
    fn lookup_cache(
        &self,
        track_id: Option<i64>,
        query: &LyricQuery,
    ) -> Result<Option<(Option<String>, String, i64)>, String> {
        if let Some(id) = track_id {
            let row = self
                .conn
                .query_row(
                    "SELECT body, kind, fetched_at FROM lyrics_cache WHERE track_id = ?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(|err| err.to_string())?;
            if row.is_some() {
                return Ok(row);
            }
        }
        self.conn
            .query_row(
                "SELECT body, kind, fetched_at FROM lyrics_cache
                 WHERE artist = ?1 AND title = ?2 AND album = ?3 AND duration_seconds = ?4",
                params![
                    query.artist,
                    query.title,
                    query.album,
                    query.duration_seconds as i64
                ],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|err| err.to_string())
    }
}

fn writable_dir(dir: &Path) -> bool {
    let probe = dir.join(".llamp-offset-write");
    match fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
