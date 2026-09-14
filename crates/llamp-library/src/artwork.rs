//! Artwork cache. Default cap is 512 MiB. Eviction deletes cache files and rows, not audio.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

pub const DEFAULT_CAP: u64 = 512 * 1024 * 1024;

pub fn store(
    conn: &Connection,
    cache_dir: &Path,
    track_id: i64,
    bytes: &[u8],
    cap: u64,
    tick: &mut i64,
) -> Result<PathBuf, String> {
    fs::create_dir_all(cache_dir).map_err(|err| err.to_string())?;
    *tick += 1;
    let path = cache_dir.join(format!("{track_id}-{tick}.bin"));
    fs::write(&path, bytes).map_err(|err| err.to_string())?;
    conn.execute(
        "INSERT INTO artwork (track_id, path, bytes, last_used) VALUES (?1, ?2, ?3, ?4)",
        params![track_id, path.to_string_lossy().as_ref(), bytes.len() as i64, *tick],
    )
    .map_err(|err| err.to_string())?;
    evict(conn, cap)?;
    Ok(path)
}

fn evict(conn: &Connection, cap: u64) -> Result<(), String> {
    loop {
        let used: i64 = conn
            .query_row("SELECT COALESCE(SUM(bytes), 0) FROM artwork", [], |row| row.get(0))
            .map_err(|err| err.to_string())?;
        if used as u64 <= cap {
            return Ok(());
        }
        let next: Option<(i64, String)> = conn
            .query_row(
                "SELECT id, path FROM artwork ORDER BY last_used ASC, id ASC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        let Some((id, path)) = next else {
            return Ok(());
        };
        let file = PathBuf::from(&path);
        if file.is_file() {
            fs::remove_file(&file).map_err(|err| err.to_string())?;
        }
        conn.execute("DELETE FROM artwork WHERE id = ?1", [id])
            .map_err(|err| err.to_string())?;
    }
}
