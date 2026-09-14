//! BMP and ZIP builders for skin tests. Not the production decoder.

#![allow(dead_code)]

use std::io::Write;

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

pub fn zip_stored(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut locals = Vec::new();
    let mut central = Vec::new();
    let mut offset = 0u32;
    for (name, data) in files {
        let name_bytes = name.as_bytes();
        let crc = crc32(data);
        let mut local = Vec::new();
        local.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name_bytes);
        local.extend_from_slice(data);
        locals.extend_from_slice(&local);

        central.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
        offset += local.len() as u32;
    }
    let mut out = locals;
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Patch every local and central uncompressed-size field. Used to forge a bomb header.
pub fn patch_uncompressed_sizes(zip: &mut [u8], size: u32) {
    let local = 0x0403_4B50u32.to_le_bytes();
    let central = 0x0201_4B50u32.to_le_bytes();
    let mut i = 0;
    while i + 26 < zip.len() {
        if zip[i..i + 4] == local {
            zip[i + 22..i + 26].copy_from_slice(&size.to_le_bytes());
            i += 4;
        } else if zip[i..i + 4] == central {
            zip[i + 24..i + 28].copy_from_slice(&size.to_le_bytes());
            i += 4;
        } else {
            i += 1;
        }
    }
}

pub fn zip_deflate(name: &str, data: &[u8]) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer.start_file(name, options).expect("start");
        writer.write_all(data).expect("write");
        writer.finish().expect("finish");
    }
    cursor.into_inner()
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// `pixels` is top-down RGB. BMP stores BGR. `padding` fills each row to stride.
pub fn bmp24(width: u32, height: u32, top_down: bool, pixels: &[u8], padding: u8) -> Vec<u8> {
    assert_eq!(pixels.len(), width as usize * height as usize * 3);
    let stride = (((width * 24 + 31) / 32) * 4) as usize;
    let row_bytes = (width * 3) as usize;
    let mut rows = vec![padding; stride * height as usize];
    for y in 0..height as usize {
        let src_y = if top_down { y } else { height as usize - 1 - y };
        let src = src_y * row_bytes;
        let dst = y * stride;
        for x in 0..width as usize {
            let rgb = src + x * 3;
            let bgr = dst + x * 3;
            rows[bgr] = pixels[rgb + 2];
            rows[bgr + 1] = pixels[rgb + 1];
            rows[bgr + 2] = pixels[rgb];
        }
    }
    bmp_file(
        width,
        if top_down {
            -(height as i32)
        } else {
            height as i32
        },
        24,
        0,
        &[],
        &rows,
    )
}

pub fn bmp8(width: u32, height: u32, indices: &[u8], palette_bgr0: &[[u8; 4]]) -> Vec<u8> {
    assert_eq!(indices.len(), width as usize * height as usize);
    let stride = (((width * 8 + 31) / 32) * 4) as usize;
    let mut rows = vec![0u8; stride * height as usize];
    for y in 0..height as usize {
        let file_y = height as usize - 1 - y;
        let src = y * width as usize;
        let dst = file_y * stride;
        rows[dst..dst + width as usize].copy_from_slice(&indices[src..src + width as usize]);
    }
    let mut palette = Vec::new();
    for entry in palette_bgr0 {
        palette.extend_from_slice(entry);
    }
    bmp_file(width, height as i32, 8, 0, &palette, &rows)
}

pub fn bmp4(width: u32, height: u32, packed: &[u8], palette_bgr0: &[[u8; 4]]) -> Vec<u8> {
    let stride = (((width * 4 + 31) / 32) * 4) as usize;
    let mut rows = vec![0u8; stride * height as usize];
    let src_stride = width.div_ceil(2) as usize;
    assert_eq!(packed.len(), src_stride * height as usize);
    for y in 0..height as usize {
        let file_y = height as usize - 1 - y;
        let src = y * src_stride;
        let dst = file_y * stride;
        rows[dst..dst + src_stride].copy_from_slice(&packed[src..src + src_stride]);
    }
    let mut palette = Vec::new();
    for entry in palette_bgr0 {
        palette.extend_from_slice(entry);
    }
    bmp_file(width, height as i32, 4, 0, &palette, &rows)
}

pub fn bmp_rle8(width: u32, height: i32, palette_bgr0: &[[u8; 4]], rle: &[u8]) -> Vec<u8> {
    let mut palette = Vec::new();
    for entry in palette_bgr0 {
        palette.extend_from_slice(entry);
    }
    bmp_file(width, height, 8, 1, &palette, rle)
}

pub fn bmp_header_only(width: i32, height: i32, bpp: u16, compression: u32) -> Vec<u8> {
    bmp_file(width as u32, height, bpp, compression, &[], &[])
}

fn bmp_file(
    width: u32,
    height: i32,
    bpp: u16,
    compression: u32,
    palette: &[u8],
    pixels: &[u8],
) -> Vec<u8> {
    let offset = 14 + 40 + palette.len() as u32;
    let file_size = offset + pixels.len() as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"BM");
    write_u32(&mut out, file_size);
    write_u16(&mut out, 0);
    write_u16(&mut out, 0);
    write_u32(&mut out, offset);
    write_u32(&mut out, 40);
    write_i32(&mut out, width as i32);
    write_i32(&mut out, height);
    write_u16(&mut out, 1);
    write_u16(&mut out, bpp);
    write_u32(&mut out, compression);
    write_u32(&mut out, pixels.len() as u32);
    write_i32(&mut out, 0);
    write_i32(&mut out, 0);
    write_u32(
        &mut out,
        if palette.is_empty() {
            0
        } else {
            (palette.len() / 4) as u32
        },
    );
    write_u32(&mut out, 0);
    out.extend_from_slice(palette);
    out.extend_from_slice(pixels);
    out
}

pub fn solid_main(rgb: [u8; 3]) -> Vec<u8> {
    let mut pixels = vec![0u8; 275 * 116 * 3];
    for chunk in pixels.chunks_exact_mut(3) {
        chunk.copy_from_slice(&rgb);
    }
    bmp24(275, 116, false, &pixels, 0)
}

pub fn main_with_pixel(rgb: [u8; 3], at: (u32, u32), pixel: [u8; 3]) -> Vec<u8> {
    let mut pixels = vec![0u8; 275 * 116 * 3];
    for chunk in pixels.chunks_exact_mut(3) {
        chunk.copy_from_slice(&rgb);
    }
    let i = ((at.1 * 275 + at.0) * 3) as usize;
    pixels[i..i + 3].copy_from_slice(&pixel);
    bmp24(275, 116, false, &pixels, 0)
}
