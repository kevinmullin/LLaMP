//! Headless skin engine. Shells consume the atlas. They do not parse BMP.

mod atlas;
mod blit;
mod bmp;
mod config;
mod layout;
mod png_io;
mod zip;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use sha2::{Digest, Sha256};

pub use blit::scale_nearest;
pub use bmp::decode_bmp;
pub use config::default_vis_colors;

pub const MAIN_WIDTH: u32 = 275;
pub const MAIN_HEIGHT: u32 = 116;
pub const SHADE_HEIGHT: u32 = 14;
pub const MAX_UNCOMPRESSED_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_SHEET_DIMENSION: u32 = 4096;
/// Declared uncompressed / compressed. A flat BMP stays under this; a bomb does not.
pub const MAX_COMPRESSION_RATIO: u64 = 10_000;
pub const FALLBACK_RGBA: [u8; 4] = [0x80, 0x80, 0x80, 0xFF];

const DEFECT_IF_MISSING: &[&str] = &[
    "eqmain.bmp",
    "eq_ex.bmp",
    "pledit.bmp",
    "gen.bmp",
    "genex.bmp",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sprite {
    Main,
    Titlebar,
    TitlebarPressed,
    Minimize,
    MinimizePressed,
    Shade,
    ShadePressed,
    Close,
    ClosePressed,
    ClutterO,
    ClutterOPressed,
    ClutterA,
    ClutterAPressed,
    ClutterI,
    ClutterIPressed,
    ClutterD,
    ClutterDPressed,
    ClutterV,
    ClutterVPressed,
    Previous,
    PreviousPressed,
    Play,
    PlayPressed,
    Pause,
    PausePressed,
    Stop,
    StopPressed,
    Next,
    NextPressed,
    Eject,
    EjectPressed,
    SeekBar,
    SeekThumb,
    VolumeTrack,
    VolumeThumb,
    BalanceTrack,
    BalanceThumb,
    Mono,
    Stereo,
    IndicatorPlay,
    IndicatorPause,
    IndicatorStop,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    DigitColon,
    DigitMinus,
    DigitBlank,
    ShuffleOff,
    ShuffleOn,
    RepeatOff,
    RepeatOn,
    EqOff,
    EqOn,
    PlaylistOff,
    PlaylistOn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Titlebar,
    Minimize,
    Shade,
    Close,
    ClutterO,
    ClutterA,
    ClutterI,
    ClutterD,
    ClutterV,
    Previous,
    Play,
    Pause,
    Stop,
    Next,
    Eject,
    Seek,
    Volume,
    Balance,
    Mono,
    Stereo,
    EqToggle,
    PlaylistToggle,
    Shuffle,
    Repeat,
    Time,
    Marquee,
    VisPane,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistColors {
    pub normal: Rgb,
    pub current: Rgb,
    pub normal_bg: Rgb,
    pub selected_bg: Rgb,
    pub mb_fg: Rgb,
    pub mb_bg: Rgb,
    pub requested_font: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Polygon {
    pub points: Vec<(i32, i32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regions {
    pub normal: Vec<Polygon>,
    pub window_shade: Vec<Polygon>,
    pub equalizer: Vec<Polygon>,
    pub equalizer_ws: Vec<Polygon>,
}

#[derive(Clone, Debug)]
pub struct Skin {
    pub id: [u8; 32],
    pub atlas: Vec<u8>,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub sprites: Vec<(Sprite, Rect)>,
    pub regions: Regions,
    pub vis_colors: [Rgb; 24],
    pub playlist_colors: PlaylistColors,
    pub glyphs: Vec<(char, Rect)>,
    pub defects: Vec<String>,
    pub cursors: Vec<String>,
}

impl Skin {
    pub fn sprite(&self, id: Sprite) -> Option<Rect> {
        self.sprites
            .iter()
            .find(|(sprite, _)| *sprite == id)
            .map(|(_, rect)| *rect)
    }

    pub fn sprite_px(&self, id: Sprite, x: u32, y: u32) -> [u8; 4] {
        let rect = self.sprite(id).expect("sprite");
        let ax = rect.x + x;
        let ay = rect.y + y;
        let i = ((ay * self.atlas_width + ax) * 4) as usize;
        self.atlas[i..i + 4].try_into().expect("pixel")
    }
}

#[derive(Debug)]
pub struct LoadError(pub String);

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoadError {}

#[derive(Clone, Debug)]
pub struct DecodedBmp {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct GoldenPng {
    pub width: u32,
    pub height: u32,
    pub color_type: u8,
    pub bit_depth: u8,
    pub srgb: bool,
    pub iccp: bool,
    pub gama: bool,
    pub chrm: bool,
    pub rgba: Vec<u8>,
}

pub struct SkinSlot {
    current: Option<Skin>,
}

impl Default for SkinSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl SkinSlot {
    pub fn new() -> Self {
        Self { current: None }
    }

    pub fn current(&self) -> Option<&Skin> {
        self.current.as_ref()
    }

    pub fn load_wsz(&mut self, bytes: &[u8]) -> Result<&Skin, LoadError> {
        let id = Sha256::digest(bytes).into();
        let archive = zip::read_archive(bytes)?;
        let skin = assemble(id, archive.files, archive.defects, archive.cursors)?;
        self.current = Some(skin);
        Ok(self.current.as_ref().expect("just stored"))
    }

    /// Directory of the same layout as a `.wsz`. This is a host path, not an archive entry.
    pub fn load_dir(&mut self, path: &Path) -> Result<&Skin, LoadError> {
        let mut files = Vec::new();
        let mut raw = Vec::new();
        let entries = std::fs::read_dir(path).map_err(|err| LoadError(err.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|err| LoadError(err.to_string()))?;
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.contains("..") || name.contains('/') || name.contains('\\') {
                return Err(LoadError(format!("zip-slip rejected {name}")));
            }
            let bytes = std::fs::read(entry.path()).map_err(|err| LoadError(err.to_string()))?;
            raw.extend(name.as_bytes());
            raw.extend(&bytes);
            files.push(zip::ArchiveFile {
                name,
                depth: 0,
                bytes,
            });
        }
        let id = Sha256::digest(&raw).into();
        let skin = assemble(id, files, Vec::new(), Vec::new())?;
        self.current = Some(skin);
        Ok(self.current.as_ref().expect("just stored"))
    }
}

pub fn blit_main(skin: &Skin) -> Vec<u8> {
    blit::blit_main(skin)
}

pub fn write_golden_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    png_io::write_golden_png(rgba, width, height)
}

pub fn read_golden_png(bytes: &[u8]) -> Result<GoldenPng, String> {
    png_io::read_golden_png(bytes)
}

pub fn control_rect(control: Control) -> Rect {
    layout::control_rect(control)
}

pub fn fuzz_zip(data: &[u8]) {
    let mut slot = SkinSlot::new();
    let _ = slot.load_wsz(data);
}

pub fn fuzz_bmp(data: &[u8]) {
    let _ = decode_bmp(data);
}

pub fn render_skin_cli(args: &[String]) -> ExitCode {
    if args.len() != 2 {
        eprintln!("usage: llamp render-skin <in.wsz> <out.png>");
        return ExitCode::FAILURE;
    }
    match render_skin_to_file(Path::new(&args[0]), Path::new(&args[1])) {
        Ok(()) => {
            println!("{}", args[1]);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn render_skin_to_file(input: &Path, output: &Path) -> Result<(), String> {
    let bytes = std::fs::read(input).map_err(|err| err.to_string())?;
    let mut slot = SkinSlot::new();
    let skin = slot.load_wsz(&bytes).map_err(|err| err.to_string())?;
    let rgba = blit_main(skin);
    let png = write_golden_png(&rgba, MAIN_WIDTH, MAIN_HEIGHT)?;
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    std::fs::write(output, png).map_err(|err| err.to_string())
}

fn assemble(
    id: [u8; 32],
    files: Vec<zip::ArchiveFile>,
    mut defects: Vec<String>,
    cursors: Vec<String>,
) -> Result<Skin, LoadError> {
    let chosen = shallowest(files, &mut defects);
    for name in DEFECT_IF_MISSING {
        if !chosen.contains_key(*name) {
            defects.push(format!("missing {name}"));
        }
    }
    let mut images: Vec<(layout::Sheet, Option<DecodedBmp>)> = Vec::new();
    for sheet in [
        layout::Sheet::Main,
        layout::Sheet::Titlebar,
        layout::Sheet::Cbuttons,
        layout::Sheet::Shufrep,
        layout::Sheet::Posbar,
        layout::Sheet::Volume,
        layout::Sheet::Balance,
        layout::Sheet::Monoster,
        layout::Sheet::Playpaus,
        layout::Sheet::Numbers,
        layout::Sheet::Text,
    ] {
        images.push((sheet, decode_sheet(sheet, &chosen, &mut defects)?));
    }
    let main = images
        .iter()
        .find(|(sheet, _)| *sheet == layout::Sheet::Main)
        .and_then(|(_, img)| img.as_ref())
        .ok_or_else(|| LoadError("missing main.bmp".into()))?;
    if main.width != MAIN_WIDTH || main.height != MAIN_HEIGHT {
        return Err(LoadError(format!(
            "main.bmp is {}x{}, expected 275x116",
            main.width, main.height
        )));
    }
    let refs: Vec<(layout::Sheet, Option<&DecodedBmp>)> = images
        .iter()
        .map(|(sheet, img)| (*sheet, img.as_ref()))
        .collect();
    let packed = atlas::pack(&refs, &mut defects);
    let vis_colors = match chosen.get("viscolor.txt") {
        Some(bytes) => {
            let text = String::from_utf8_lossy(bytes);
            config::parse_viscolor(&text, &mut defects)
        }
        None => {
            defects.push("missing viscolor.txt".into());
            default_vis_colors()
        }
    };
    let playlist_colors = match chosen.get("pledit.txt") {
        Some(bytes) => config::parse_pledit(&String::from_utf8_lossy(bytes)),
        None => {
            defects.push("missing pledit.txt".into());
            config::default_playlist_colors()
        }
    };
    let regions = match chosen.get("region.txt") {
        Some(bytes) => config::parse_region(&String::from_utf8_lossy(bytes), &mut defects),
        None => Regions {
            normal: Vec::new(),
            window_shade: Vec::new(),
            equalizer: Vec::new(),
            equalizer_ws: Vec::new(),
        },
    };
    Ok(Skin {
        id,
        atlas: packed.atlas,
        atlas_width: packed.width,
        atlas_height: packed.height,
        sprites: packed.sprites,
        regions,
        vis_colors,
        playlist_colors,
        glyphs: packed.glyphs,
        defects,
        cursors,
    })
}

fn shallowest(
    files: Vec<zip::ArchiveFile>,
    defects: &mut Vec<String>,
) -> BTreeMap<String, Vec<u8>> {
    let mut best: BTreeMap<String, (usize, Vec<u8>)> = BTreeMap::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for file in files {
        *counts.entry(file.name.clone()).or_insert(0) += 1;
        match best.get_mut(&file.name) {
            Some((depth, bytes)) if file.depth < *depth => {
                *depth = file.depth;
                *bytes = file.bytes;
            }
            Some(_) => {}
            None => {
                best.insert(file.name, (file.depth, file.bytes));
            }
        }
    }
    for (name, count) in counts {
        if count > 1 {
            defects.push(format!("duplicate {name}"));
        }
    }
    best.into_iter()
        .map(|(name, (_, bytes))| (name, bytes))
        .collect()
}

fn decode_sheet(
    sheet: layout::Sheet,
    files: &BTreeMap<String, Vec<u8>>,
    defects: &mut Vec<String>,
) -> Result<Option<DecodedBmp>, LoadError> {
    let bmp_name = format!("{}.bmp", sheet.file_stem());
    let png_name = format!("{}.png", sheet.file_stem());
    let (name, bytes, png) = if let Some(bytes) = files.get(&bmp_name) {
        (bmp_name, bytes, false)
    } else if let Some(bytes) = files.get(&png_name) {
        (png_name, bytes, true)
    } else {
        return Ok(None);
    };
    let decoded = if png {
        png_io::decode_png(bytes)
    } else {
        decode_bmp(bytes)
    };
    match decoded {
        Ok(image) => Ok(Some(image)),
        Err(err) if sheet == layout::Sheet::Main => Err(LoadError(format!("{name}: {err}"))),
        Err(err) => {
            defects.push(format!("{name}: {err}"));
            Ok(None)
        }
    }
}
