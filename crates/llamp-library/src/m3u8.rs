//! Playlist files keep order and path strings. They do not rewrite them under an app folder.

use std::fs;
use std::path::Path;

use rusqlite::Connection;

pub fn import_m3u(conn: &Connection, path: &Path) -> Result<(), String> {
    replace_items(conn, &read_hash_paths(path)?)
}

pub fn export_m3u(conn: &Connection, path: &Path) -> Result<(), String> {
    write_text(path, &format_m3u(&read_items(conn)?))
}

pub fn import_m3u8(conn: &Connection, path: &Path) -> Result<(), String> {
    import_m3u(conn, path)
}

pub fn export_m3u8(conn: &Connection, path: &Path) -> Result<(), String> {
    export_m3u(conn, path)
}

pub fn import_pls(conn: &Connection, path: &Path) -> Result<(), String> {
    replace_items(conn, &read_pls_paths(path)?)
}

pub fn export_pls(conn: &Connection, path: &Path) -> Result<(), String> {
    write_text(path, &format_pls(&read_items(conn)?))
}

pub fn import_xspf(conn: &Connection, path: &Path) -> Result<(), String> {
    replace_items(conn, &read_xspf_paths(path)?)
}

pub fn export_xspf(conn: &Connection, path: &Path) -> Result<(), String> {
    write_text(path, &format_xspf(&read_items(conn)?))
}

fn replace_items(conn: &Connection, paths: &[String]) -> Result<(), String> {
    conn.execute("DELETE FROM playlist_items", [])
        .map_err(|err| err.to_string())?;
    for (index, line) in paths.iter().enumerate() {
        conn.execute(
            "INSERT INTO playlist_items (position, path) VALUES (?1, ?2)",
            rusqlite::params![index as i64, line],
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn read_items(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT path FROM playlist_items ORDER BY position ASC")
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| err.to_string())?;
    rows.map(|row| row.map_err(|err| err.to_string()))
        .collect()
}

fn read_hash_paths(path: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect())
}

fn format_m3u(paths: &[String]) -> String {
    let mut out = String::from("#EXTM3U\n");
    for path in paths {
        out.push_str(path);
        out.push('\n');
    }
    out
}

fn read_pls_paths(path: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let mut files: Vec<(u32, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if let Some(rest) = key.strip_prefix("File") {
            if let Ok(index) = rest.parse::<u32>() {
                files.push((index, value.to_string()));
            }
        }
    }
    files.sort_by_key(|(index, _)| *index);
    Ok(files.into_iter().map(|(_, path)| path).collect())
}

fn format_pls(paths: &[String]) -> String {
    let mut out = String::from("[playlist]\n");
    out.push_str(&format!("NumberOfEntries={}\n", paths.len()));
    for (index, path) in paths.iter().enumerate() {
        out.push_str(&format!("File{}={}\n", index + 1, path));
    }
    out.push_str("Version=2\n");
    out
}

fn read_xspf_paths(path: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    if text.len() > 32 * 1024 * 1024 {
        return Err("xspf too large".into());
    }
    let mut paths = Vec::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find("<location>") {
        rest = &rest[start + "<location>".len()..];
        let Some(end) = rest.find("</location>") else {
            break;
        };
        paths.push(unescape_xml(rest[..end].trim()));
        rest = &rest[end + "</location>".len()..];
    }
    Ok(paths)
}

fn format_xspf(paths: &[String]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<playlist version=\"1\" xmlns=\"http://xspf.org/ns/0/\">\n  <trackList>\n",
    );
    for path in paths {
        out.push_str("    <track><location>");
        out.push_str(&escape_xml(path));
        out.push_str("</location></track>\n");
    }
    out.push_str("  </trackList>\n</playlist>\n");
    out
}

fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

fn unescape_xml(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, text).map_err(|err| err.to_string())
}
