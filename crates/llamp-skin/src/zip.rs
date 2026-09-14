//! In-memory ZIP read. No archive path is opened on the filesystem.

use std::io::{self, Read};

use crate::{LoadError, MAX_COMPRESSION_RATIO, MAX_UNCOMPRESSED_BYTES};

pub struct ArchiveFile {
    pub name: String,
    pub depth: usize,
    pub bytes: Vec<u8>,
}

pub struct ArchiveRead {
    pub files: Vec<ArchiveFile>,
    pub defects: Vec<String>,
    pub cursors: Vec<String>,
}

struct Central {
    name: String,
    method: u16,
    compressed: u64,
    uncompressed: u64,
    encrypted: bool,
    directory: bool,
}

pub fn read_archive(bytes: &[u8]) -> Result<ArchiveRead, LoadError> {
    let central = parse_central(bytes)?;
    let mut remaining = MAX_UNCOMPRESSED_BYTES;
    let mut keep = vec![false; central.len()];
    let mut defects = Vec::new();
    let mut cursors = Vec::new();
    for (i, entry) in central.iter().enumerate() {
        if entry.directory {
            continue;
        }
        if is_zip_slip(&entry.name) {
            defects.push(format!("zip-slip rejected {}", entry.name));
            continue;
        }
        let (basename, depth) = basename_depth(&entry.name);
        if basename.is_empty() {
            continue;
        }
        if entry.encrypted {
            defects.push(format!("encrypted entry skipped {basename}"));
            continue;
        }
        if entry.method != 0 && entry.method != 8 {
            return Err(LoadError(format!(
                "unsupported compression method {}",
                entry.method
            )));
        }
        if entry.uncompressed > remaining {
            return Err(LoadError(format!(
                "uncompressed size {} exceeds cap",
                entry.uncompressed
            )));
        }
        if entry.compressed == 0 && entry.uncompressed > 0
            || entry.compressed > 0 && entry.uncompressed / entry.compressed > MAX_COMPRESSION_RATIO
        {
            return Err(LoadError(format!(
                "compression ratio {} exceeds {MAX_COMPRESSION_RATIO}",
                if entry.compressed == 0 {
                    entry.uncompressed
                } else {
                    entry.uncompressed / entry.compressed
                }
            )));
        }
        if basename.ends_with(".cur") {
            cursors.push(basename);
            remaining -= entry.uncompressed;
            continue;
        }
        if matches!(basename.as_str(), "video.bmp" | "mb.bmp") {
            remaining -= entry.uncompressed;
            continue;
        }
        remaining -= entry.uncompressed;
        keep[i] = true;
        let _ = depth;
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|err| LoadError(format!("zip: {err}")))?;
    if archive.len() != central.len() {
        return Err(LoadError("zip entry count mismatch".into()));
    }
    let mut files = Vec::new();
    for (i, entry) in central.iter().enumerate() {
        if !keep[i] {
            continue;
        }
        let mut file = archive
            .by_index(i)
            .map_err(|err| LoadError(format!("zip: {err}")))?;
        if file.size() > MAX_UNCOMPRESSED_BYTES {
            return Err(LoadError(format!(
                "uncompressed size {} exceeds cap",
                file.size()
            )));
        }
        let compressed = file.compressed_size();
        if compressed == 0 && file.size() > 0
            || compressed > 0 && file.size() / compressed > MAX_COMPRESSION_RATIO
        {
            return Err(LoadError("compression ratio exceeds cap".into()));
        }
        let mut buf = Vec::new();
        let limit = file.size();
        io::copy(&mut file.by_ref().take(limit.saturating_add(1)), &mut buf)
            .map_err(|err| LoadError(format!("zip: {err}")))?;
        if buf.len() as u64 > limit || buf.len() as u64 > MAX_UNCOMPRESSED_BYTES {
            return Err(LoadError("uncompressed inflate exceeds cap".into()));
        }
        let (name, depth) = basename_depth(&entry.name);
        files.push(ArchiveFile {
            name,
            depth,
            bytes: buf,
        });
    }
    Ok(ArchiveRead {
        files,
        defects,
        cursors,
    })
}

fn parse_central(bytes: &[u8]) -> Result<Vec<Central>, LoadError> {
    let eocd = find_eocd(bytes)
        .ok_or_else(|| LoadError("zip: missing end of central directory".into()))?;
    let count = u16::from_le_bytes(bytes[eocd + 10..eocd + 12].try_into().unwrap()) as usize;
    let size = u32::from_le_bytes(bytes[eocd + 12..eocd + 16].try_into().unwrap()) as usize;
    let offset = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    if offset
        .checked_add(size)
        .map(|end| end > bytes.len())
        .unwrap_or(true)
    {
        return Err(LoadError("zip: central directory out of range".into()));
    }
    let mut entries = Vec::new();
    let mut pos = offset;
    let end = offset + size;
    while pos + 46 <= end && entries.len() < count {
        if bytes[pos..pos + 4] != 0x0201_4B50u32.to_le_bytes() {
            return Err(LoadError("zip: bad central directory".into()));
        }
        let method = u16::from_le_bytes(bytes[pos + 10..pos + 12].try_into().unwrap());
        let flags = u16::from_le_bytes(bytes[pos + 8..pos + 10].try_into().unwrap());
        let compressed = u32::from_le_bytes(bytes[pos + 20..pos + 24].try_into().unwrap()) as u64;
        let uncompressed = u32::from_le_bytes(bytes[pos + 24..pos + 28].try_into().unwrap()) as u64;
        let name_len = u16::from_le_bytes(bytes[pos + 28..pos + 30].try_into().unwrap()) as usize;
        let extra_len = u16::from_le_bytes(bytes[pos + 30..pos + 32].try_into().unwrap()) as usize;
        let comment_len =
            u16::from_le_bytes(bytes[pos + 32..pos + 34].try_into().unwrap()) as usize;
        let name_at = pos + 46;
        let name_end = name_at
            .checked_add(name_len)
            .ok_or_else(|| LoadError("zip: name overflow".into()))?;
        if name_end > bytes.len() {
            return Err(LoadError("zip: name out of range".into()));
        }
        let name = String::from_utf8_lossy(&bytes[name_at..name_end]).into_owned();
        let next = name_end
            .checked_add(extra_len)
            .and_then(|n| n.checked_add(comment_len))
            .ok_or_else(|| LoadError("zip: entry overflow".into()))?;
        if next > end && next > bytes.len() {
            return Err(LoadError("zip: entry out of range".into()));
        }
        if uncompressed == u32::MAX as u64 || compressed == u32::MAX as u64 {
            return Err(LoadError("uncompressed size exceeds cap".into()));
        }
        entries.push(Central {
            directory: name.ends_with('/') || name.ends_with('\\'),
            encrypted: flags & 1 != 0,
            name,
            method,
            compressed,
            uncompressed,
        });
        pos = next;
    }
    Ok(entries)
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 22 {
        return None;
    }
    let start = bytes.len().saturating_sub(22 + 65535);
    let sig = 0x0605_4B50u32.to_le_bytes();
    let mut i = bytes.len() - 22;
    loop {
        if bytes[i..i + 4] == sig {
            let comment = u16::from_le_bytes(bytes[i + 20..i + 22].try_into().ok()?) as usize;
            if i + 22 + comment == bytes.len() {
                return Some(i);
            }
        }
        if i == start {
            break;
        }
        i -= 1;
    }
    None
}

pub fn is_zip_slip(name: &str) -> bool {
    let name = name.replace('\\', "/");
    if name.starts_with('/') || name.starts_with("//") {
        return true;
    }
    let bytes = name.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        return true;
    }
    name.split('/').any(|part| part == "..")
}

pub fn basename_depth(name: &str) -> (String, usize) {
    let name = name.replace('\\', "/");
    let name = name.trim_end_matches('/');
    let parts: Vec<&str> = name
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    let depth = parts.len().saturating_sub(1);
    let base = parts.last().copied().unwrap_or("").to_ascii_lowercase();
    (base, depth)
}
