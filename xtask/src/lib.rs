//! Synthetic fixture skin. Source PNGs are committed. This crate paints them and packs a `.wsz`.

use std::io::Write;
use std::path::{Path, PathBuf};

const KEY: [u8; 3] = [0xFF, 0x00, 0xFF];

pub fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets/skins/src/fixture")
}

pub fn write_sources(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, image) in sheets() {
        write_png(&dir.join(format!("{name}.png")), &image)?;
    }
    std::fs::write(dir.join("pledit.txt"), PLEDIT)?;
    std::fs::write(dir.join("region.txt"), REGION)?;
    let _ = std::fs::remove_file(dir.join("viscolor.txt"));
    Ok(())
}

pub fn fixture_wsz() -> Vec<u8> {
    pack_dir(&fixture_dir()).expect("fixture sources")
}

pub fn pack_dir(dir: &Path) -> std::io::Result<Vec<u8>> {
    let mut files = Vec::new();
    for (name, _) in sheets() {
        let png = std::fs::read(dir.join(format!("{name}.png")))?;
        let (w, h, rgb) = read_png_rgb(&png).map_err(std::io::Error::other)?;
        files.push((format!("{name}.bmp"), bmp24(w, h, &rgb)));
    }
    files.push(("pledit.txt".into(), std::fs::read(dir.join("pledit.txt"))?));
    files.push(("region.txt".into(), std::fs::read(dir.join("region.txt"))?));
    files.sort_by(|a, b| a.0.cmp(&b.0));
    zip_stored(&files).map_err(std::io::Error::other)
}

fn sheets() -> Vec<(&'static str, Image)> {
    vec![
        ("main", paint_main()),
        ("titlebar", paint_titlebar()),
        ("cbuttons", paint_cbuttons()),
        ("shufrep", paint_shufrep()),
        ("posbar", paint_posbar()),
        ("volume", paint_volume()),
        ("balance", paint_balance()),
        ("monoster", paint_monoster()),
        ("playpaus", paint_playpaus()),
        ("numbers", paint_numbers()),
        ("text", paint_text()),
    ]
}

struct Image {
    width: u32,
    height: u32,
    rgb: Vec<u8>,
}

impl Image {
    fn fill(width: u32, height: u32, color: [u8; 3]) -> Self {
        let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
        for _ in 0..width * height {
            rgb.extend_from_slice(&color);
        }
        Self { width, height, rgb }
    }

    fn set(&mut self, x: u32, y: u32, color: [u8; 3]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = ((y * self.width + x) * 3) as usize;
        self.rgb[i..i + 3].copy_from_slice(&color);
    }

    fn rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, color);
            }
        }
    }
}

fn paint_main() -> Image {
    Image::fill(275, 116, [0x1A, 0x1A, 0x1A])
}

fn paint_titlebar() -> Image {
    let mut img = Image::fill(275, 28, KEY);
    img.rect(0, 0, 275, 14, [0x4A, 0x4A, 0x4A]);
    img.set(0, 0, KEY);
    img.rect(0, 14, 275, 14, [0x6A, 0x6A, 0x6A]);
    let buttons = [
        (10u32, 8u32, [0xE2, 0x4B, 0x4B]),
        (20, 8, [0xE2, 0xA1, 0x4B]),
        (30, 8, [0xE2, 0xE2, 0x4B]),
        (40, 8, [0x4B, 0xE2, 0x4B]),
        (50, 8, [0x4B, 0x4B, 0xE2]),
        (244, 9, [0xF2, 0xF2, 0xF2]),
        (254, 9, [0xC0, 0xC0, 0xC0]),
        (264, 9, [0xFF, 0x40, 0x40]),
    ];
    for (x, size, color) in buttons {
        img.rect(x, 3, size, size, color);
        img.rect(x, 17, size, size, darken(color));
    }
    img
}

fn paint_cbuttons() -> Image {
    let mut img = Image::fill(139, 37, KEY);
    let colors = [
        [0xC0, 0x40, 0x40],
        [0x40, 0xC0, 0x40],
        [0xC0, 0xC0, 0x40],
        [0x40, 0x40, 0xC0],
        [0x40, 0xC0, 0xC0],
        [0xC0, 0x40, 0xC0],
    ];
    for (i, color) in colors.into_iter().enumerate() {
        let x = 1 + i as u32 * 23;
        img.rect(x, 1, 23, 18, color);
        img.rect(x, 19, 23, 18, lighten(color));
    }
    img
}

fn paint_shufrep() -> Image {
    let mut img = Image::fill(93, 25, KEY);
    let colors = [
        [0xA0, 0x50, 0x50],
        [0x50, 0xA0, 0x50],
        [0x50, 0x50, 0xA0],
        [0xA0, 0xA0, 0x50],
    ];
    for (i, color) in colors.into_iter().enumerate() {
        let x = 1 + i as u32 * 23;
        img.rect(x, 1, 23, 12, color);
        img.rect(x, 13, 23, 12, lighten(color));
    }
    img
}

fn paint_posbar() -> Image {
    let mut img = Image::fill(249, 23, KEY);
    img.rect(1, 1, 248, 10, [0xB0, 0xB0, 0xB0]);
    img.rect(1, 12, 29, 10, [0xFF, 0xFF, 0xFF]);
    img
}

fn paint_volume() -> Image {
    let mut img = Image::fill(69, 28, KEY);
    img.rect(1, 1, 68, 14, [0x70, 0x70, 0x70]);
    img.rect(1, 16, 14, 11, [0xF0, 0xF0, 0xF0]);
    img
}

fn paint_balance() -> Image {
    let mut img = Image::fill(69, 28, KEY);
    img.rect(1, 1, 68, 14, [0x60, 0x60, 0x80]);
    img.rect(1, 16, 14, 11, [0xE0, 0xE0, 0xFF]);
    img
}

fn paint_monoster() -> Image {
    let mut img = Image::fill(58, 13, KEY);
    img.rect(1, 1, 28, 12, [0xD0, 0xD0, 0x40]);
    img.rect(30, 1, 28, 12, [0x40, 0xD0, 0xD0]);
    img
}

fn paint_playpaus() -> Image {
    let mut img = Image::fill(30, 10, KEY);
    img.rect(1, 1, 9, 9, [0x40, 0xFF, 0x40]);
    img.rect(11, 1, 9, 9, [0xFF, 0xFF, 0x40]);
    img.rect(21, 1, 9, 9, [0xFF, 0x40, 0x40]);
    img
}

fn paint_numbers() -> Image {
    let mut img = Image::fill(118, 14, KEY);
    for i in 0..13u32 {
        let x = 1 + i * 9;
        img.rect(x, 1, 9, 13, [0x10, 0x10, 0x10]);
        match i {
            0 => box_digit(&mut img, x, 1),
            10 => {
                img.rect(x + 3, 4, 2, 2, [0xF8, 0xF8, 0xF8]);
                img.rect(x + 3, 8, 2, 2, [0xF8, 0xF8, 0xF8]);
            }
            11 => img.rect(x + 2, 6, 5, 1, [0xF8, 0xF8, 0xF8]),
            12 => {}
            n => img.rect(x + 1 + (n % 7), 2, 1, 9, [0xF8, 0xF8, 0xF8]),
        }
    }
    img
}

fn box_digit(img: &mut Image, x: u32, y: u32) {
    img.rect(x + 1, y + 1, 7, 1, [0xF8, 0xF8, 0xF8]);
    img.rect(x + 1, y + 11, 7, 1, [0xF8, 0xF8, 0xF8]);
    img.rect(x + 1, y + 1, 1, 11, [0xF8, 0xF8, 0xF8]);
    img.rect(x + 7, y + 1, 1, 11, [0xF8, 0xF8, 0xF8]);
}

fn paint_text() -> Image {
    let mut img = Image::fill(80, 42, KEY);
    // Bit 4 is the leftmost pixel of a 5-wide cell.
    let letters = [
        (
            'F',
            [
                0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b10000,
            ],
        ),
        (
            'I',
            [
                0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
            ],
        ),
        (
            'X',
            [
                0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b01010, 0b10001,
            ],
        ),
        (
            'T',
            [
                0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
        ),
        (
            'U',
            [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
        ),
        (
            'R',
            [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
            ],
        ),
        (
            'E',
            [
                0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
            ],
        ),
    ];
    for (ch, rows) in letters {
        let index = ch as u32 - 32;
        let col = index % 16;
        let row = index / 16;
        let ox = col * 5;
        let oy = row * 7;
        for (dy, bits) in rows.into_iter().enumerate() {
            for dx in 0..5 {
                if bits & (1 << (4 - dx)) != 0 {
                    img.set(ox + dx, oy + dy as u32, [0xF0, 0xF0, 0xF0]);
                }
            }
        }
    }
    img
}

fn darken(color: [u8; 3]) -> [u8; 3] {
    color.map(|c| c.saturating_sub(0x20))
}

fn lighten(color: [u8; 3]) -> [u8; 3] {
    color.map(|c| c.saturating_add(0x20))
}

const PLEDIT: &str = "\
[Text]
Normal=#00FF00
Current=#FFFFFF
NormalBG=#000000
SelectedBG=#0000C0
MbFG=#00FF00
MbBG=#000000
Font=Fixture Face
";

const REGION: &str = "\
[Normal]
NumPoints=4
PointList=-8,0,274,0,274,115,0,115

[WindowShade]
NumPoints=4
PointList=0,0,274,0,274,13,0,13

[Equalizer]
NumPoints=4
PointList=0,0,274,0,274,115,0,115

[EqualizerWS]
NumPoints=4
PointList=0,0,274,0,274,13,0,13
";

fn write_png(path: &Path, image: &Image) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(&image.rgb)
        .map_err(std::io::Error::other)?;
    writer.finish().map_err(std::io::Error::other)?;
    Ok(())
}

fn read_png_rgb(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|err| err.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buf).map_err(|err| err.to_string())?;
    buf.truncate(frame.buffer_size());
    if frame.color_type != png::ColorType::Rgb {
        return Err("fixture png is not rgb".into());
    }
    Ok((frame.width, frame.height, buf))
}

fn bmp24(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let stride = (((width * 24 + 31) / 32) * 4) as usize;
    let mut rows = vec![0u8; stride * height as usize];
    for y in 0..height as usize {
        let file_y = height as usize - 1 - y;
        for x in 0..width as usize {
            let src = (y * width as usize + x) * 3;
            let dst = file_y * stride + x * 3;
            rows[dst] = rgb[src + 2];
            rows[dst + 1] = rgb[src + 1];
            rows[dst + 2] = rgb[src];
        }
    }
    let offset = 54u32;
    let file_size = offset + rows.len() as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]);
    out.extend_from_slice(&rows);
    out
}

fn zip_stored(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in files {
            writer
                .start_file(name, options)
                .map_err(|err| err.to_string())?;
            writer.write_all(data).map_err(|err| err.to_string())?;
        }
        writer.finish().map_err(|err| err.to_string())?;
    }
    Ok(cursor.into_inner())
}
