//! Golden PNG: 8-bit RGBA, color type 6, sRGB chunk, no iCCP, gAMA, or cHRM.
//! Bytes are row-major, top to bottom, straight alpha, no row padding.

use std::io::Cursor;

use crate::{DecodedBmp, GoldenPng};

const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";

pub fn write_golden_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    if rgba.len() != width as usize * height as usize * 4 {
        return Err("png byte length does not match width * height * 4".into());
    }
    let mut cursor = Cursor::new(Vec::new());
    let mut encoder = png::Encoder::new(&mut cursor, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = encoder.write_header().map_err(|err| err.to_string())?;
    writer
        .write_image_data(rgba)
        .map_err(|err| err.to_string())?;
    writer.finish().map_err(|err| err.to_string())?;
    Ok(cursor.into_inner())
}

pub fn read_golden_png(bytes: &[u8]) -> Result<GoldenPng, String> {
    let kinds = chunk_types(bytes)?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().map_err(|err| err.to_string())?;
    let info = reader.info();
    let width = info.width;
    let height = info.height;
    let color_type = match info.color_type {
        png::ColorType::Rgba => 6,
        png::ColorType::Rgb => 2,
        png::ColorType::Grayscale => 0,
        png::ColorType::GrayscaleAlpha => 4,
        png::ColorType::Indexed => 3,
    };
    let bit_depth = match info.bit_depth {
        png::BitDepth::One => 1,
        png::BitDepth::Two => 2,
        png::BitDepth::Four => 4,
        png::BitDepth::Eight => 8,
        png::BitDepth::Sixteen => 16,
    };
    let mut rgba = vec![0u8; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut rgba)
        .map_err(|err| err.to_string())?;
    rgba.truncate(frame.buffer_size());
    Ok(GoldenPng {
        width,
        height,
        color_type,
        bit_depth,
        srgb: kinds.contains(b"sRGB"),
        iccp: kinds.contains(b"iCCP"),
        gama: kinds.contains(b"gAMA"),
        chrm: kinds.contains(b"cHRM"),
        rgba,
    })
}

pub fn decode_png(bytes: &[u8]) -> Result<DecodedBmp, String> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|err| err.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buf).map_err(|err| err.to_string())?;
    buf.truncate(frame.buffer_size());
    let width = frame.width;
    let height = frame.height;
    let rgba = match frame.color_type {
        png::ColorType::Rgba => {
            let mut out = buf;
            for pixel in out.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
            out
        }
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for chunk in buf.chunks_exact(3) {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            out
        }
        other => return Err(format!("unsupported png color type {other:?}")),
    };
    Ok(DecodedBmp {
        width,
        height,
        rgba,
    })
}

fn chunk_types(bytes: &[u8]) -> Result<Vec<[u8; 4]>, String> {
    if bytes.len() < 8 || &bytes[0..8] != PNG_SIG {
        return Err("not a png".into());
    }
    let mut kinds = Vec::new();
    let mut i = 8;
    while i + 12 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        let kind = bytes[i + 4..i + 8].try_into().unwrap();
        kinds.push(kind);
        i = i.checked_add(12 + len).ok_or("png chunk overflow")?;
    }
    Ok(kinds)
}
