//! CPU reference blit of the unshaded main window. Straight RGBA, top-down, no padding.

use crate::layout::{self, Slice};
use crate::{Skin, MAIN_HEIGHT, MAIN_WIDTH};

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
