//! M3U8 keeps order and path strings. It does not rewrite them under an app folder.

use std::fs;
use std::path::Path;

use rusqlite::Connection;

pub fn import(conn: &Connection, path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    conn.execute("DELETE FROM playlist_items", [])
        .map_err(|err| err.to_string())?;
    for (index, line) in lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO playlist_items (position, path) VALUES (?1, ?2)",
            rusqlite::params![index as i64, line],
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn export(conn: &Connection, path: &Path) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT path FROM playlist_items ORDER BY position ASC")
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| err.to_string())?;
    let mut out = String::from("#EXTM3U\n");
    for row in rows {
        let line = row.map_err(|err| err.to_string())?;
        out.push_str(&line);
        out.push('\n');
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, out).map_err(|err| err.to_string())
}
