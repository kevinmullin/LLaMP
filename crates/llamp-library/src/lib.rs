//! SQLite library. Music stays at the granted path. See ADR 007.

mod artwork;
mod lrc;
mod lyrics;
mod m3u8;
mod tags;
mod watch;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};

pub use artwork::DEFAULT_CAP as ARTWORK_CAP;
pub use lrc::{parse_lrc, write_lrc};
pub use lyrics::{
    active_at, clock_ms, clamp_offset, lyrics_from_tag, read_embedded, read_sidecar, resolve_lyrics,
    LrclibConsent, LyricCursor, LyricDoc, LyricKind, LyricLine, LyricQuery, LyricSource, LyricTags,
    LyricWord, OffsetSave, MISS_RETRY_SECS, OFFSET_MAX_MS, OFFSET_MIN_MS, OFFSET_STEP_MS,
};
pub use tags::{LoftyTags, TagStore, TrackTags};

use crate::watch::Watch;

/// `tracks.storage`. Music import writes only `referenced`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    Referenced,
    Managed,
}

impl Storage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Referenced => "referenced",
            Self::Managed => "managed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub path: PathBuf,
}

pub struct Library {
    inner: Arc<Mutex<LibraryInner>>,
    path: PathBuf,
    watch: Mutex<Option<Watch>>,
}

pub struct LibraryInner {
    conn: Connection,
    artwork_cap: u64,
    artwork_tick: i64,
    cache_dir: PathBuf,
}

impl Library {
    /// Opens or creates the database and runs ordered `user_version` scripts.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let conn = Connection::open(path).map_err(|err| err.to_string())?;
        conn.pragma_update(None, "cache_size", -64)
            .map_err(|err| err.to_string())?;
        conn.pragma_update(None, "mmap_size", 0)
            .map_err(|err| err.to_string())?;
        migrate(&conn)?;
        let cache_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("artwork");
        Ok(Self {
            inner: Arc::new(Mutex::new(LibraryInner {
                conn,
                artwork_cap: artwork::DEFAULT_CAP,
                artwork_tick: 0,
                cache_dir,
            })),
            path: path.to_path_buf(),
            watch: Mutex::new(None),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn user_version(&self) -> Result<i32, String> {
        let guard = self.lock()?;
        pragma_user_version(&guard.conn)
    }

    /// Inserts one row. Does not copy the file. Identity is the path.
    pub fn insert(&self, path: &Path, storage: Storage) -> Result<(), String> {
        self.lock()?.insert(path, storage)
    }

    /// One referenced row per file in `dir`. Does not copy the files. Not recursive.
    pub fn insert_referenced_dir(&self, dir: &Path) -> Result<usize, String> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            if entry.file_type().map(|kind| kind.is_file()).unwrap_or(false) {
                paths.push(entry.path());
            }
        }
        paths.sort();
        let mut guard = self.lock()?;
        for path in &paths {
            guard.insert(path, Storage::Referenced)?;
        }
        guard
            .conn
            .execute_batch("PRAGMA shrink_memory")
            .map_err(|err| err.to_string())?;
        Ok(paths.len())
    }

    /// References every file under `dir`. Does not copy. Does not scan other folders.
    pub fn grant_folder(&self, dir: &Path) -> Result<usize, String> {
        let mut files = Vec::new();
        walk_files(dir, &mut files)?;
        files.sort();
        {
            let mut guard = self.lock()?;
            guard
                .conn
                .execute(
                    "INSERT OR IGNORE INTO grants (path) VALUES (?1)",
                    [dir.to_string_lossy().as_ref()],
                )
                .map_err(|err| err.to_string())?;
            for path in &files {
                guard.insert_if_absent(path)?;
            }
        }
        self.watch_folder(dir)?;
        Ok(files.len())
    }

    pub fn count_referenced(&self) -> Result<usize, String> {
        let guard = self.lock()?;
        let n: i64 = guard
            .conn
            .query_row(
                "SELECT COUNT(*) FROM tracks WHERE storage = 'referenced'",
                [],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())?;
        Ok(n as usize)
    }

    pub fn storage_of(&self, path: &Path) -> Result<Option<String>, String> {
        self.lock()?.storage_of(path)
    }

    pub fn missing_of(&self, path: &Path) -> Result<Option<bool>, String> {
        self.lock()?.missing_of(path)
    }

    pub fn track_id(&self, path: &Path) -> Result<i64, String> {
        self.lock()?
            .track_id(path)?
            .ok_or_else(|| format!("no row for {}", path.display()))
    }

    pub fn write_tags(&self, path: &Path, tags: &TrackTags) -> Result<(), String> {
        LoftyTags.write(path, tags)?;
        let guard = self.lock()?;
        let id = guard
            .track_id(path)?
            .ok_or_else(|| format!("not in library: {}", path.display()))?;
        guard
            .conn
            .execute(
                "UPDATE tracks SET title = ?1, artist = ?2, album = ?3, genre = ?4 WHERE id = ?5",
                params![tags.title, tags.artist, tags.album, tags.genre, id],
            )
            .map_err(|err| err.to_string())?;
        guard.index_tags(id, tags)?;
        Ok(())
    }

    /// Reads the granted file, not a copy.
    pub fn read_tags(&self, path: &Path) -> Result<TrackTags, String> {
        LoftyTags.read(path)
    }

    /// Appends the granted path to `playlist_items`. Does not copy the file.
    pub fn enqueue(&self, path: &Path) -> Result<(), String> {
        let guard = self.lock()?;
        let stored = path.to_string_lossy();
        let position: i64 = guard
            .conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_items",
                [],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())?;
        guard
            .conn
            .execute(
                "INSERT INTO playlist_items (position, path) VALUES (?1, ?2)",
                params![position, stored.as_ref()],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn enqueued_paths(&self) -> Result<Vec<PathBuf>, String> {
        let guard = self.lock()?;
        let mut stmt = guard
            .conn
            .prepare("SELECT path FROM playlist_items ORDER BY position ASC")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| err.to_string())?;
        let mut paths = Vec::new();
        for row in rows {
            paths.push(PathBuf::from(row.map_err(|err| err.to_string())?));
        }
        Ok(paths)
    }

    /// Granted tracks, path as stored. Does not copy audio.
    pub fn granted_paths(&self) -> Result<Vec<PathBuf>, String> {
        let guard = self.lock()?;
        let mut stmt = guard
            .conn
            .prepare("SELECT path FROM tracks WHERE storage = 'referenced' ORDER BY path ASC")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| err.to_string())?;
        let mut paths = Vec::new();
        for row in rows {
            paths.push(PathBuf::from(row.map_err(|err| err.to_string())?));
        }
        Ok(paths)
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchHit>, String> {
        let quoted = format!("\"{}\"", query.replace('"', "\"\""));
        let guard = self.lock()?;
        let mut stmt = guard
            .conn
            .prepare(
                "SELECT tracks.path FROM tracks_fts
                 JOIN tracks ON tracks.id = tracks_fts.rowid
                 WHERE tracks_fts MATCH ?1",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([quoted], |row| row.get::<_, String>(0))
            .map_err(|err| err.to_string())?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(SearchHit {
                path: PathBuf::from(row.map_err(|err| err.to_string())?),
            });
        }
        Ok(hits)
    }

    /// Marks a missing file. Does not drop the row. Does not walk the grant.
    pub fn rescan_path(&self, path: &Path) -> Result<(), String> {
        let mut guard = self.lock()?;
        if path.is_file() {
            guard.insert_if_absent(path)?;
        } else {
            guard.mark_missing(path)?;
        }
        Ok(())
    }

    /// Drops the row. Does not delete the user's file.
    pub fn remove_from_library(&self, path: &Path) -> Result<(), String> {
        let guard = self.lock()?;
        if let Some(id) = guard.track_id(path)? {
            guard
                .conn
                .execute("DELETE FROM tracks_fts WHERE rowid = ?1", [id])
                .map_err(|err| err.to_string())?;
            guard
                .conn
                .execute("DELETE FROM tracks WHERE id = ?1", [id])
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub fn import_m3u8(&self, path: &Path) -> Result<(), String> {
        m3u8::import_m3u8(&self.lock()?.conn, path)
    }

    pub fn export_m3u8(&self, path: &Path) -> Result<(), String> {
        m3u8::export_m3u8(&self.lock()?.conn, path)
    }

    pub fn import_m3u(&self, path: &Path) -> Result<(), String> {
        m3u8::import_m3u(&self.lock()?.conn, path)
    }

    pub fn export_m3u(&self, path: &Path) -> Result<(), String> {
        m3u8::export_m3u(&self.lock()?.conn, path)
    }

    pub fn import_pls(&self, path: &Path) -> Result<(), String> {
        m3u8::import_pls(&self.lock()?.conn, path)
    }

    pub fn export_pls(&self, path: &Path) -> Result<(), String> {
        m3u8::export_pls(&self.lock()?.conn, path)
    }

    pub fn import_xspf(&self, path: &Path) -> Result<(), String> {
        m3u8::import_xspf(&self.lock()?.conn, path)
    }

    pub fn export_xspf(&self, path: &Path) -> Result<(), String> {
        m3u8::export_xspf(&self.lock()?.conn, path)
    }

    pub fn set_artwork_cap(&self, bytes: u64) {
        if let Ok(mut guard) = self.lock() {
            guard.artwork_cap = bytes;
        }
    }

    pub fn store_artwork(&self, audio: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
        let mut guard = self.lock()?;
        let id = guard
            .track_id(audio)?
            .ok_or_else(|| format!("not in library: {}", audio.display()))?;
        let cap = guard.artwork_cap;
        let cache = guard.cache_dir.clone();
        let mut tick = guard.artwork_tick;
        let path = artwork::store(&guard.conn, &cache, id, bytes, cap, &mut tick)?;
        guard.artwork_tick = tick;
        Ok(path)
    }

    fn watch_folder(&self, dir: &Path) -> Result<(), String> {
        let mut slot = self.watch.lock().map_err(|err| err.to_string())?;
        if let Some(watch) = slot.as_mut() {
            return watch.add(dir);
        }
        *slot = Some(Watch::start(Arc::clone(&self.inner), vec![dir.to_path_buf()])?);
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, LibraryInner>, String> {
        self.inner.lock().map_err(|err| err.to_string())
    }
}

impl LibraryInner {
    fn insert(&mut self, path: &Path, storage: Storage) -> Result<(), String> {
        let missing = u8::from(!path.is_file());
        self.conn
            .execute(
                "INSERT INTO tracks (storage, path, missing) VALUES (?1, ?2, ?3)",
                params![storage.as_str(), path.to_string_lossy().as_ref(), missing],
            )
            .map_err(|err| err.to_string())?;
        let id = self.conn.last_insert_rowid();
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.index_tags(
            id,
            &TrackTags {
                title: stem,
                artist: String::new(),
                album: String::new(),
                genre: String::new(),
            },
        )?;
        Ok(())
    }

    fn insert_if_absent(&mut self, path: &Path) -> Result<(), String> {
        if self.storage_of(path)?.is_some() {
            if path.is_file() {
                self.conn
                    .execute(
                        "UPDATE tracks SET missing = 0 WHERE path = ?1",
                        [path.to_string_lossy().as_ref()],
                    )
                    .map_err(|err| err.to_string())?;
            }
            return Ok(());
        }
        self.insert(path, Storage::Referenced)
    }

    /// FSEvents may report `/private/var` for a grant stored as `/var`. Keep the grant's spelling.
    fn spell_like_grant(&self, path: &Path) -> Option<PathBuf> {
        let grants = self.grant_paths().ok()?;
        let needle = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        for grant in grants {
            let grant_path = PathBuf::from(&grant);
            let canon = grant_path.canonicalize().unwrap_or_else(|_| grant_path.clone());
            if needle == canon {
                return Some(grant_path);
            }
            if let Ok(rel) = needle.strip_prefix(&canon) {
                return Some(grant_path.join(rel));
            }
        }
        None
    }

    fn grant_paths(&self) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM grants")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<String>, _>>()
            .map_err(|err| err.to_string())
    }

    fn note_new_files(&mut self, dir: &Path) -> Result<(), String> {
        let Ok(entries) = fs::read_dir(dir) else {
            return Ok(());
        };
        for entry in entries.flatten() {
            if entry.file_type().map(|kind| kind.is_file()).unwrap_or(false) {
                self.insert_if_absent(&entry.path())?;
            }
        }
        Ok(())
    }

    fn mark_missing(&mut self, path: &Path) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE tracks SET missing = 1 WHERE path = ?1",
                [path.to_string_lossy().as_ref()],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn storage_of(&self, path: &Path) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "SELECT storage FROM tracks WHERE path = ?1",
                [path.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())
    }

    fn missing_of(&self, path: &Path) -> Result<Option<bool>, String> {
        let value: Option<i64> = self
            .conn
            .query_row(
                "SELECT missing FROM tracks WHERE path = ?1",
                [path.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())?;
        Ok(value.map(|n| n != 0))
    }

    fn track_id(&self, path: &Path) -> Result<Option<i64>, String> {
        self.conn
            .query_row(
                "SELECT id FROM tracks WHERE path = ?1",
                [path.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())
    }

    fn index_tags(&self, id: i64, tags: &TrackTags) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM tracks_fts WHERE rowid = ?1", [id])
            .map_err(|err| err.to_string())?;
        self.conn
            .execute(
                "INSERT INTO tracks_fts(rowid, title, artist, album, genre) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, tags.title, tags.artist, tags.album, tags.genre],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}

fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|err| err.to_string())?;
        if kind.is_dir() {
            walk_files(&path, out)?;
        } else if kind.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn migrate(conn: &Connection) -> Result<(), String> {
    if pragma_user_version(conn)? < 1 {
        conn.execute_batch(include_str!("../migrations/001.sql"))
            .map_err(|err| err.to_string())?;
        conn.pragma_update(None, "user_version", 1)
            .map_err(|err| err.to_string())?;
    }
    if pragma_user_version(conn)? < 2 {
        conn.execute_batch(include_str!("../migrations/002.sql"))
            .map_err(|err| err.to_string())?;
        conn.pragma_update(None, "user_version", 2)
            .map_err(|err| err.to_string())?;
    }
    if pragma_user_version(conn)? < 3 {
        conn.execute_batch(include_str!("../migrations/003.sql"))
            .map_err(|err| err.to_string())?;
        conn.pragma_update(None, "user_version", 3)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn pragma_user_version(conn: &Connection) -> Result<i32, String> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|err| err.to_string())
}
