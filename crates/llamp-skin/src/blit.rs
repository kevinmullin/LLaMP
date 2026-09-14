//! CPU reference blit of the unshaded main window. Straight RGBA, top-down, no padding.

use crate::layout::{self, Slice};
use crate::{Skin, MAIN_HEIGHT, MAIN_WIDTH};

/// Idle defaults. A display blit with these values is `blit_main`.
pub struct Display<'a> {
    pub time: &'a str,
    pub marquee: &'a str,
    pub marquee_skip: usize,
    pub volume_ppm: u16,
    pub balance_ppm: u16,
    /// 0 is the fixture seek-thumb origin. Travel is the seek-bar slice minus the thumb slice.
    pub seek_ppm: u16,
    /// Latest mono samples, newest at the end. `None` leaves the pane unpainted.
    /// Painting uses the fixture vis-pane rect. It does not invent a bar count.
    pub scope: Option<&'a [f32]>,
    /// Digit readout. Idle blit does not stamp this.
    pub kbps: u16,
    /// Digit readout. Idle blit does not stamp this.
    pub khz: u16,
    /// Band energies 0..1. Bar width is `layout::VIS_BAR_W`. `None` leaves the pane unpainted.
    pub spectrum: Option<&'a [f32]>,
}

impl Default for Display<'static> {
    fn default() -> Self {
        Self {
            time: layout::TIME_TEXT,
            marquee: layout::MARQUEE_TEXT,
            marquee_skip: 0,
            volume_ppm: 0,
            balance_ppm: 500,
            seek_ppm: 0,
            scope: None,
            kbps: 0,
            khz: 0,
            spectrum: None,
        }
    }
}

pub fn blit_display(skin: &Skin, display: Display<'_>) -> Vec<u8> {
    let mut buf = blit_main(skin);
    if display.time == layout::TIME_TEXT
        && display.marquee == layout::MARQUEE_TEXT
        && display.marquee_skip == 0
        && display.volume_ppm == 0
        && display.balance_ppm == 500
        && display.seek_ppm == 0
        && display.scope.is_none()
        && display.kbps == 0
        && display.khz == 0
        && display.spectrum.is_none()
    {
        return buf;
    }
    let time_box = layout::control_rect(crate::Control::Time);
    stamp_text(&mut buf, skin, display.time, time_box, 9, true);
    let marquee = layout::control_rect(crate::Control::Marquee);
    let skipped: String = display.marquee.chars().skip(display.marquee_skip).collect();
    stamp_text(&mut buf, skin, &skipped, marquee, layout::GLYPH_CELL.0, false);
    stamp_thumb(&mut buf, skin, true, display.volume_ppm);
    stamp_thumb(&mut buf, skin, false, display.balance_ppm);
    stamp_along(
        &mut buf,
        skin,
        crate::Sprite::SeekBar,
        crate::Sprite::SeekThumb,
        display.seek_ppm,
    );
    if let Some(samples) = display.scope {
        stamp_scope(&mut buf, skin, samples);
    }
    if let Some(bands) = display.spectrum {
        stamp_spectrum(&mut buf, skin, bands);
    }
    if display.kbps > 0 {
        stamp_text(
            &mut buf,
            skin,
            &format!("{:03}", display.kbps.min(999)),
            crate::Rect { x: layout::KBPS_ORIGIN.0, y: layout::KBPS_ORIGIN.1, w: 27, h: 13 },
            9,
            true,
        );
    }
    if display.khz > 0 {
        stamp_text(
            &mut buf,
            skin,
            &format!("{:02}", display.khz.min(99)),
            crate::Rect { x: layout::KHZ_ORIGIN.0, y: layout::KHZ_ORIGIN.1, w: 18, h: 13 },
            9,
            true,
        );
    }
    buf
}

pub fn blit_main(skin: &Skin) -> Vec<u8> {
    let mut buf = vec![0u8; MAIN_WIDTH as usize * MAIN_HEIGHT as usize * 4];
    for slice in layout::slices() {
        if let Some(dest) = slice.blit {
            stamp(skin, &mut buf, slice, dest);
        }
    }
    blit_string(
        skin,
        &mut buf,
        layout::TIME_TEXT,
        layout::DIGIT_ORIGIN,
        9,
        true,
    );
    blit_string(
        skin,
        &mut buf,
        layout::MARQUEE_TEXT,
        layout::MARQUEE_ORIGIN,
        layout::GLYPH_CELL.0,
        false,
    );
    if !skin.regions.normal.is_empty() {
        apply_mask(&mut buf, &skin.regions.normal);
    }
    buf
}

pub fn scale_nearest(src: &[u8], width: u32, height: u32, factor: u32) -> Vec<u8> {
    let out_w = width * factor;
    let out_h = height * factor;
    let mut out = vec![0u8; out_w as usize * out_h as usize * 4];
    for y in 0..height {
        for x in 0..width {
            let si = ((y * width + x) * 4) as usize;
            let pixel = &src[si..si + 4];
            for dy in 0..factor {
                for dx in 0..factor {
                    let di = (((y * factor + dy) * out_w + (x * factor + dx)) * 4) as usize;
                    out[di..di + 4].copy_from_slice(pixel);
                }
            }
        }
    }
    out
}

fn stamp_text(
    buf: &mut [u8],
    skin: &Skin,
    text: &str,
    bounds: crate::Rect,
    advance: u32,
    digits: bool,
) {
    for (i, ch) in text.chars().enumerate() {
        let dest_x = bounds.x + i as u32 * advance;
        if dest_x >= bounds.x + bounds.w {
            break;
        }
        if digits {
            let Some(sprite) = layout::digit_sprite(ch) else { continue };
            let Some(slice) = layout::slices().iter().find(|slice| slice.sprite == sprite) else {
                continue;
            };
            stamp_clipped(
                skin,
                buf,
                slice,
                crate::Rect { x: dest_x, y: bounds.y, w: slice.src.w, h: slice.src.h },
                bounds,
            );
        } else if let Some((_, rect)) = skin.glyphs.iter().find(|(glyph, _)| *glyph == ch) {
            stamp_rect_clipped(skin, buf, *rect, dest_x, bounds.y, bounds);
        }
    }
}

/// Oscilloscope into the fixture vis pane. One column per pane pixel, not a bar count.
/// Background is viscolor 0, a static sparse grid is 1, the plot is 18.
/// Indices 19–22 and peak-hold 23 stay unused until a golden locks them.
fn stamp_scope(buf: &mut [u8], skin: &Skin, samples: &[f32]) {
    let pane = layout::control_rect(crate::Control::VisPane);
    let bg = skin.vis_colors[0];
    let dot = skin.vis_colors[1];
    let plot = skin.vis_colors[18];
    for y in 0..pane.h {
        for x in 0..pane.w {
            put(buf, pane.x + x, pane.y + y, bg);
        }
    }
    // Not fixture-locked. Not a spectrum bar width.
    const SPARSE_DOT_PITCH: u32 = 8;
    for y in (0..pane.h).step_by(SPARSE_DOT_PITCH as usize) {
        for x in (0..pane.w).step_by(SPARSE_DOT_PITCH as usize) {
            put(buf, pane.x + x, pane.y + y, dot);
        }
    }
    let columns = pane.w as usize;
    let start = samples.len().saturating_sub(columns);
    let mid = pane.h as f32 / 2.0;
    for x in 0..pane.w {
        let sample = samples.get(start + x as usize).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
        let y = (mid - sample * (mid - 0.5)).round() as i32;
        let y = y.clamp(0, pane.h as i32 - 1) as u32;
        put(buf, pane.x + x, pane.y + y, plot);
    }
}

fn stamp_spectrum(buf: &mut [u8], skin: &Skin, bands: &[f32]) {
    let pane = layout::control_rect(crate::Control::VisPane);
    let bg = skin.vis_colors[0];
    let bar = skin.vis_colors[18];
    for y in 0..pane.h {
        for x in 0..pane.w {
            put(buf, pane.x + x, pane.y + y, bg);
        }
    }
    let width = layout::VIS_BAR_W;
    let count = (pane.w / width) as usize;
    for index in 0..count {
        let energy = bands.get(index).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let bar_h = ((energy * pane.h as f32).round() as u32).min(pane.h);
        let x0 = pane.x + index as u32 * width;
        for y in pane.y + pane.h - bar_h..pane.y + pane.h {
            for dx in 0..width {
                put(buf, x0 + dx, y, bar);
            }
        }
    }
}

fn put(buf: &mut [u8], x: u32, y: u32, color: crate::Rgb) {
    if x >= MAIN_WIDTH || y >= MAIN_HEIGHT {
        return;
    }
    let di = ((y * MAIN_WIDTH + x) * 4) as usize;
    buf[di] = color.r;
    buf[di + 1] = color.g;
    buf[di + 2] = color.b;
    buf[di + 3] = 255;
}

fn stamp_thumb(buf: &mut [u8], skin: &Skin, volume: bool, ppm: u16) {
    let (track_sprite, thumb_sprite) = if volume {
        (crate::Sprite::VolumeTrack, crate::Sprite::VolumeThumb)
    } else {
        (crate::Sprite::BalanceTrack, crate::Sprite::BalanceThumb)
    };
    stamp_along(buf, skin, track_sprite, thumb_sprite, ppm);
}

fn stamp_along(buf: &mut [u8], skin: &Skin, track_sprite: crate::Sprite, thumb_sprite: crate::Sprite, ppm: u16) {
    let Some(track) = layout::slices().iter().find(|slice| slice.sprite == track_sprite) else {
        return;
    };
    let Some(thumb) = layout::slices().iter().find(|slice| slice.sprite == thumb_sprite) else {
        return;
    };
    let Some(track_dest) = track.blit else { return };
    let Some(thumb_dest) = thumb.blit else { return };
    stamp(skin, buf, track, track_dest);
    let travel = track_dest.w.saturating_sub(thumb_dest.w);
    let x = track_dest.x + (u32::from(ppm.min(1000)) * travel) / 1000;
    stamp(
        skin,
        buf,
        thumb,
        crate::Rect { x, y: thumb_dest.y, w: thumb_dest.w, h: thumb_dest.h },
    );
}

fn stamp_clipped(skin: &Skin, buf: &mut [u8], slice: &Slice, dest: crate::Rect, bounds: crate::Rect) {
    let Some(src) = skin.sprite(slice.sprite) else { return };
    stamp_rect_clipped(skin, buf, src, dest.x, dest.y, bounds);
}

fn stamp_rect_clipped(skin: &Skin, buf: &mut [u8], src: crate::Rect, dest_x: u32, dest_y: u32, bounds: crate::Rect) {
    for y in 0..src.h {
        for x in 0..src.w {
            let dx = dest_x + x;
            let dy = dest_y + y;
            if dx < bounds.x || dy < bounds.y || dx >= bounds.x + bounds.w || dy >= bounds.y + bounds.h {
                continue;
            }
            let ax = src.x + x;
            let ay = src.y + y;
            if ax >= skin.atlas_width || ay >= skin.atlas_height {
                continue;
            }
            let si = ((ay * skin.atlas_width + ax) * 4) as usize;
            if skin.atlas[si + 3] == 0 {
                continue;
            }
            if dx >= MAIN_WIDTH || dy >= MAIN_HEIGHT {
                continue;
            }
            let di = ((dy * MAIN_WIDTH + dx) * 4) as usize;
            buf[di..di + 4].copy_from_slice(&skin.atlas[si..si + 4]);
        }
    }
}

fn blit_string(
    skin: &Skin,
    buf: &mut [u8],
    text: &str,
    origin: (u32, u32),
    advance: u32,
    digits: bool,
) {
    for (i, ch) in text.chars().enumerate() {
        let dest_x = origin.0 + i as u32 * advance;
        if digits {
            let Some(sprite) = layout::digit_sprite(ch) else {
                continue;
            };
            let Some(slice) = layout::slices().iter().find(|slice| slice.sprite == sprite) else {
                continue;
            };
            stamp(
                skin,
                buf,
                slice,
                crate::Rect {
                    x: dest_x,
                    y: origin.1,
                    w: slice.src.w,
                    h: slice.src.h,
                },
            );
        } else if let Some((_, rect)) = skin.glyphs.iter().find(|(glyph, _)| *glyph == ch) {
            stamp_rect(skin, buf, *rect, dest_x, origin.1);
        }
    }
}

fn stamp(skin: &Skin, buf: &mut [u8], slice: &Slice, dest: crate::Rect) {
    let Some(src) = skin.sprite(slice.sprite) else {
        return;
    };
    stamp_rect(skin, buf, src, dest.x, dest.y);
}

fn stamp_rect(skin: &Skin, buf: &mut [u8], src: crate::Rect, dest_x: u32, dest_y: u32) {
    for y in 0..src.h {
        for x in 0..src.w {
            let ax = src.x + x;
            let ay = src.y + y;
            if ax >= skin.atlas_width || ay >= skin.atlas_height {
                continue;
            }
            let si = ((ay * skin.atlas_width + ax) * 4) as usize;
            if skin.atlas[si + 3] == 0 {
                continue;
            }
            let dx = dest_x + x;
            let dy = dest_y + y;
            if dx >= MAIN_WIDTH || dy >= MAIN_HEIGHT {
                continue;
            }
            let di = ((dy * MAIN_WIDTH + dx) * 4) as usize;
            buf[di..di + 4].copy_from_slice(&skin.atlas[si..si + 4]);
        }
    }
}

fn apply_mask(buf: &mut [u8], polygons: &[crate::Polygon]) {
    for y in 0..MAIN_HEIGHT {
        for x in 0..MAIN_WIDTH {
            if polygons
                .iter()
                .any(|poly| contains(&poly.points, x as i32, y as i32))
            {
                continue;
            }
            let i = ((y * MAIN_WIDTH + x) * 4) as usize;
            buf[i + 3] = 0;
        }
    }
}

fn contains(points: &[(i32, i32)], x: i32, y: i32) -> bool {
    if points.len() < 3 {
        return false;
    }
    let n = points.len();
    for i in 0..n {
        let (ax, ay) = points[i];
        let (bx, by) = points[(i + 1) % n];
        if on_segment(x, y, ax, ay, bx, by) {
            return true;
        }
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = points[i];
        let (xj, yj) = points[j];
        if (yi > y) != (yj > y) {
            let denom = (yj - yi) as i64;
            if denom != 0 {
                let cross = (xj - xi) as i64 * (y - yi) as i64;
                let left = (x - xi) as i64 * denom;
                if (denom > 0 && left < cross) || (denom < 0 && left > cross) {
                    inside = !inside;
                }
            }
        }
        j = i;
    }
    inside
}

fn on_segment(px: i32, py: i32, ax: i32, ay: i32, bx: i32, by: i32) -> bool {
    let cross = (px - ax) as i64 * (by - ay) as i64 - (py - ay) as i64 * (bx - ax) as i64;
    if cross != 0 {
        return false;
    }
    let dot = (px - ax) as i64 * (bx - ax) as i64 + (py - ay) as i64 * (by - ay) as i64;
    if dot < 0 {
        return false;
    }
    let len2 = (bx - ax) as i64 * (bx - ax) as i64 + (by - ay) as i64 * (by - ay) as i64;
    dot <= len2
}
