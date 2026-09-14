//! SQLite library. Music stays at the granted path. See ADR 007.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

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

pub struct Library {
    conn: Connection,
    path: PathBuf,
}

impl Library {
    /// Opens or creates the database and runs ordered `user_version` scripts.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let conn = Connection::open(path).map_err(|err| err.to_string())?;
        // Default page cache is larger than 1000 path rows need. Keep it small so
        // the idle RSS scene can stay under 80 MiB with the connection open.
        conn.pragma_update(None, "cache_size", -64)
            .map_err(|err| err.to_string())?;
        conn.pragma_update(None, "mmap_size", 0)
            .map_err(|err| err.to_string())?;
        migrate(&conn)?;
        Ok(Self { conn, path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn user_version(&self) -> Result<i32, String> {
        pragma_user_version(&self.conn)
    }

    /// Inserts one row. Does not copy the file. Identity is the path.
    pub fn insert(&self, path: &Path, storage: Storage) -> Result<(), String> {
        let missing = u8::from(!path.is_file());
        self.conn
            .execute(
                "INSERT INTO tracks (storage, path, missing) VALUES (?1, ?2, ?3)",
                (storage.as_str(), path.to_string_lossy().as_ref(), missing),
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    /// One referenced row per file in `dir`. Does not copy the files.
    pub fn insert_referenced_dir(&self, dir: &Path) -> Result<usize, String> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            if entry.file_type().map(|kind| kind.is_file()).unwrap_or(false) {
                paths.push(entry.path());
            }
        }
        paths.sort();
        for path in &paths {
            self.insert(path, Storage::Referenced)?;
        }
        self.conn
            .execute_batch("PRAGMA shrink_memory")
            .map_err(|err| err.to_string())?;
        Ok(paths.len())
    }

    pub fn count_referenced(&self) -> Result<usize, String> {
        let n: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM tracks WHERE storage = 'referenced'",
                [],
                |row| row.get(0),
            )
            .map_err(|err| err.to_string())?;
        Ok(n as usize)
    }

    /// Returns the storage string stored for `path`.
    pub fn storage_of(&self, path: &Path) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "SELECT storage FROM tracks WHERE path = ?1",
                [path.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| err.to_string())
    }
}

fn migrate(conn: &Connection) -> Result<(), String> {
    let version = pragma_user_version(conn)?;
    if version < 1 {
        conn.execute_batch(include_str!("../migrations/001.sql"))
            .map_err(|err| err.to_string())?;
        conn.pragma_update(None, "user_version", 1)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn pragma_user_version(conn: &Connection) -> Result<i32, String> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|err| err.to_string())
}
