//! BMP decode. Caps are checked before any pixel allocation.

use crate::{DecodedBmp, MAX_SHEET_DIMENSION, MAX_UNCOMPRESSED_BYTES};

pub fn decode_bmp(bytes: &[u8]) -> Result<DecodedBmp, String> {
    if bytes.len() < 54 || &bytes[0..2] != b"BM" {
        return Err("not a BMP".into());
    }
    let pixel_offset = read_u32(bytes, 10)? as usize;
    let header_size = read_u32(bytes, 14)?;
    if header_size != 40 {
        return Err(format!("unsupported BMP header size {header_size}"));
    }
    let width_i = read_i32(bytes, 18)?;
    let height_i = read_i32(bytes, 22)?;
    let planes = read_u16(bytes, 26)?;
    let bpp = read_u16(bytes, 28)?;
    let compression = read_u32(bytes, 30)?;
    let colors_used = read_u32(bytes, 46)?;
    if planes != 1 {
        return Err(format!("unsupported BMP planes {planes}"));
    }
    if width_i <= 0 {
        return Err("BMP width is not positive".into());
    }
    let top_down = height_i < 0;
    let height_i = height_i.unsigned_abs();
    let width = width_i as u32;
    let height = height_i;
    // BMP width and height are i32. The RGBA byte count is checked in u32 so a
    // wrapping multiply cannot become an allocation size. i32::MAX * i32::MAX * 4
    // does not wrap u64; it still must not be allocated.
    let rgba_bytes = (width)
        .checked_mul(height)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| "width * height * 4 overflow".to_string())?;
    if width > MAX_SHEET_DIMENSION || height > MAX_SHEET_DIMENSION {
        return Err(format!("sheet dimension exceeds {MAX_SHEET_DIMENSION}"));
    }
    if u64::from(rgba_bytes) > MAX_UNCOMPRESSED_BYTES {
        return Err("uncompressed cap".into());
    }
    if pixel_offset > bytes.len() {
        return Err("BMP pixel offset past end".into());
    }
    let palette_colors = if matches!(bpp, 1 | 4 | 8) {
        if colors_used == 0 {
            1usize << bpp
        } else {
            colors_used as usize
        }
    } else {
        0
    };
    let palette_start = 54;
    let palette_end = palette_start + palette_colors * 4;
    if palette_end > bytes.len() || palette_end > pixel_offset && palette_colors > 0 {
        return Err("BMP palette truncated".into());
    }
    let mut palette = vec![[0u8, 0, 0, 0]; palette_colors];
    for (i, entry) in palette.iter_mut().enumerate() {
        let o = palette_start + i * 4;
        *entry = [bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]];
    }
    let pixels_bytes = &bytes[pixel_offset..];
    match compression {
        0 => decode_rgb(width, height, top_down, bpp, &palette, pixels_bytes),
        1 if bpp == 8 => decode_rle8(width, height, top_down, &palette, pixels_bytes),
        1 => Err("BI_RLE8 requires 8-bit".into()),
        2 => Err("unsupported compression BI_RLE4".into()),
        3 => Err("unsupported compression bitfields".into()),
        4 => Err("unsupported compression JPEG-in-BMP".into()),
        5 => Err("unsupported compression PNG-in-BMP".into()),
        other => Err(format!("unsupported compression {other}")),
    }
}

fn decode_rgb(
    width: u32,
    height: u32,
    top_down: bool,
    bpp: u16,
    palette: &[[u8; 4]],
    data: &[u8],
) -> Result<DecodedBmp, String> {
    if !matches!(bpp, 1 | 4 | 8 | 24 | 32) {
        return Err(format!("unsupported BMP bit count {bpp}"));
    }
    let stride = row_stride(width, bpp)?;
    let need = stride
        .checked_mul(height as u64)
        .ok_or_else(|| "width * height * 4 overflow".to_string())?;
    if need > data.len() as u64 {
        return Err("BMP pixel data truncated".into());
    }
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    for file_y in 0..height {
        let dest_y = if top_down {
            file_y
        } else {
            height - 1 - file_y
        };
        let row = &data[file_y as usize * stride as usize
            ..(file_y as usize * stride as usize) + stride as usize];
        for x in 0..width {
            let (r, g, b) = pixel_rgb(row, x, bpp, palette)?;
            put(&mut rgba, width, x, dest_y, r, g, b, 255);
        }
    }
    Ok(DecodedBmp {
        width,
        height,
        rgba,
    })
}

fn decode_rle8(
    width: u32,
    height: u32,
    top_down: bool,
    palette: &[[u8; 4]],
    data: &[u8],
) -> Result<DecodedBmp, String> {
    let mut indices = vec![0u8; width as usize * height as usize];
    let mut x = 0u32;
    let mut y = 0u32;
    let mut i = 0;
    while i < data.len() && y < height {
        if i + 1 >= data.len() {
            break;
        }
        let count = data[i];
        let value = data[i + 1];
        i += 2;
        if count > 0 {
            for _ in 0..count {
                if y >= height {
                    break;
                }
                if x < width {
                    let row = if top_down { y } else { height - 1 - y };
                    indices[(row * width + x) as usize] = value;
                    x += 1;
                }
                if x >= width {
                    x = 0;
                    y = y.saturating_add(1);
                }
            }
        } else {
            match value {
                0 => {
                    x = 0;
                    y = y.saturating_add(1);
                }
                1 => break,
                2 => {
                    if i + 1 >= data.len() {
                        break;
                    }
                    x = x.saturating_add(u32::from(data[i]));
                    y = y.saturating_add(u32::from(data[i + 1]));
                    i += 2;
                }
                n => {
                    let n = usize::from(n);
                    if i + n > data.len() {
                        return Err("truncated BI_RLE8".into());
                    }
                    for byte in &data[i..i + n] {
                        if y >= height {
                            break;
                        }
                        if x < width {
                            let row = if top_down { y } else { height - 1 - y };
                            indices[(row * width + x) as usize] = *byte;
                            x += 1;
                        }
                        if x >= width {
                            x = 0;
                            y = y.saturating_add(1);
                        }
                    }
                    i += n;
                    if n % 2 == 1 {
                        i += 1;
                    }
                }
            }
        }
    }
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    for y in 0..height {
        for x in 0..width {
            let index = indices[(y * width + x) as usize] as usize;
            let (b, g, r) = palette_rgb(palette, index);
            put(&mut rgba, width, x, y, r, g, b, 255);
        }
    }
    Ok(DecodedBmp {
        width,
        height,
        rgba,
    })
}

fn pixel_rgb(row: &[u8], x: u32, bpp: u16, palette: &[[u8; 4]]) -> Result<(u8, u8, u8), String> {
    match bpp {
        1 | 4 | 8 => {
            let index = match bpp {
                1 => {
                    let byte = *row.get((x / 8) as usize).ok_or("BMP row truncated")?;
                    (byte >> (7 - (x % 8))) & 1
                }
                4 => {
                    let byte = *row.get((x / 2) as usize).ok_or("BMP row truncated")?;
                    if x % 2 == 0 {
                        byte >> 4
                    } else {
                        byte & 0x0F
                    }
                }
                _ => *row.get(x as usize).ok_or("BMP row truncated")?,
            };
            let (b, g, r) = palette_rgb(palette, usize::from(index));
            Ok((r, g, b))
        }
        24 => {
            let o = (x * 3) as usize;
            if o + 2 >= row.len() {
                return Err("BMP row truncated".into());
            }
            Ok((row[o + 2], row[o + 1], row[o]))
        }
        32 => {
            let o = (x * 4) as usize;
            if o + 2 >= row.len() {
                return Err("BMP row truncated".into());
            }
            Ok((row[o + 2], row[o + 1], row[o]))
        }
        _ => Err(format!("unsupported BMP bit count {bpp}")),
    }
}

fn palette_rgb(palette: &[[u8; 4]], index: usize) -> (u8, u8, u8) {
    palette
        .get(index)
        .map(|p| (p[0], p[1], p[2]))
        .unwrap_or((0, 0, 0))
}

fn put(rgba: &mut [u8], width: u32, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
    let i = ((y * width + x) * 4) as usize;
    rgba[i] = r;
    rgba[i + 1] = g;
    rgba[i + 2] = b;
    rgba[i + 3] = a;
}

fn row_stride(width: u32, bpp: u16) -> Result<u64, String> {
    let bits = (width as u64)
        .checked_mul(u64::from(bpp))
        .ok_or_else(|| "width * height * 4 overflow".to_string())?;
    Ok(bits.saturating_add(31) / 32 * 4)
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, String> {
    let slice = bytes.get(at..at + 2).ok_or("truncated BMP")?;
    Ok(u16::from_le_bytes(slice.try_into().unwrap()))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, String> {
    let slice = bytes.get(at..at + 4).ok_or("truncated BMP")?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()))
}

fn read_i32(bytes: &[u8], at: usize) -> Result<i32, String> {
    Ok(read_u32(bytes, at)? as i32)
}
