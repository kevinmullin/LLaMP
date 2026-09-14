//! Playlist and gen chrome from fixture-locked pledit/gen slices.

use crate::{layout, Rect, Skin, Sprite, FALLBACK_RGBA, MAIN_HEIGHT, MAIN_WIDTH};

pub fn blit_playlist(skin: &Skin, width: u32, height: u32) -> Vec<u8> {
    let w = width.max(MAIN_WIDTH);
    let h = height.max(MAIN_HEIGHT);
    let mut buf = vec![0u8; w as usize * h as usize * 4];
    for pixel in buf.chunks_exact_mut(4) {
        pixel.copy_from_slice(&FALLBACK_RGBA);
    }
    fill_rgba(&mut buf, w, h, layout::r(8, 14, w.saturating_sub(16), h.saturating_sub(28)), [
        skin.playlist_colors.normal_bg.r,
        skin.playlist_colors.normal_bg.g,
        skin.playlist_colors.normal_bg.b,
        255,
    ]);
    stamp_repeat_x(skin, &mut buf, w, h, Sprite::PleditTitle, 0, 0, w, 14);
    stamp_repeat_y(skin, &mut buf, w, h, Sprite::PleditLeft, 0, 14, 8, h.saturating_sub(28));
    stamp_repeat_y(skin, &mut buf, w, h, Sprite::PleditRight, w.saturating_sub(8), 14, 8, h.saturating_sub(28));
    stamp_repeat_x(skin, &mut buf, w, h, Sprite::PleditBottom, 0, h.saturating_sub(14), w, 14);
    let buttons = [
        Sprite::PleditAdd,
        Sprite::PleditRem,
        Sprite::PleditSel,
        Sprite::PleditMisc,
        Sprite::PleditList,
    ];
    let button_w = w / 5;
    for (index, sprite) in buttons.into_iter().enumerate() {
        stamp_to(skin, &mut buf, w, h, sprite, index as u32 * button_w, h.saturating_sub(13));
    }
    buf
}

pub fn blit_gen(skin: &Skin, width: u32, height: u32) -> Vec<u8> {
    let w = width.max(MAIN_WIDTH);
    let h = height.max(MAIN_HEIGHT);
    let mut buf = vec![0u8; w as usize * h as usize * 4];
    for pixel in buf.chunks_exact_mut(4) {
        pixel.copy_from_slice(&FALLBACK_RGBA);
    }
    stamp_repeat_x(skin, &mut buf, w, h, Sprite::GenTitle, 0, 0, w, 14);
    stamp_repeat_y(skin, &mut buf, w, h, Sprite::GenLeft, 0, 14, 8, h.saturating_sub(22));
    stamp_repeat_y(skin, &mut buf, w, h, Sprite::GenRight, w.saturating_sub(8), 14, 8, h.saturating_sub(22));
    stamp_repeat_x(skin, &mut buf, w, h, Sprite::GenBottom, 8, h.saturating_sub(8), w.saturating_sub(16), 8);
    stamp_to(skin, &mut buf, w, h, Sprite::GenClient, 8, 14);
    stamp_to(skin, &mut buf, w, h, Sprite::GenexClose, w.saturating_sub(20), 1);
    stamp_to(skin, &mut buf, w, h, Sprite::GenexShade, w.saturating_sub(40), 1);
    buf
}

fn fill_rgba(buf: &mut [u8], stride: u32, height: u32, rect: Rect, rgba: [u8; 4]) {
    for y in rect.y..rect.y.saturating_add(rect.h).min(height) {
        for x in rect.x..rect.x.saturating_add(rect.w).min(stride) {
            let i = ((y * stride + x) * 4) as usize;
            buf[i..i + 4].copy_from_slice(&rgba);
        }
    }
}

fn stamp_to(skin: &Skin, buf: &mut [u8], stride: u32, height: u32, sprite: Sprite, dest_x: u32, dest_y: u32) {
    let Some(src) = skin.sprite(sprite) else {
        return;
    };
    stamp_rect(skin, buf, stride, height, src, dest_x, dest_y);
}

fn stamp_repeat_x(
    skin: &Skin,
    buf: &mut [u8],
    stride: u32,
    height: u32,
    sprite: Sprite,
    dest_x: u32,
    dest_y: u32,
    dest_w: u32,
    dest_h: u32,
) {
    let Some(src) = skin.sprite(sprite) else {
        return;
    };
    let mut x = dest_x;
    while x < dest_x + dest_w {
        stamp_rect(skin, buf, stride, height, Rect { x: src.x, y: src.y, w: src.w.min(dest_w - (x - dest_x)), h: src.h.min(dest_h) }, x, dest_y);
        x += src.w.max(1);
    }
}

fn stamp_repeat_y(
    skin: &Skin,
    buf: &mut [u8],
    stride: u32,
    height: u32,
    sprite: Sprite,
    dest_x: u32,
    dest_y: u32,
    dest_w: u32,
    dest_h: u32,
) {
    let Some(src) = skin.sprite(sprite) else {
        return;
    };
    let mut y = dest_y;
    while y < dest_y + dest_h {
        stamp_rect(skin, buf, stride, height, Rect { x: src.x, y: src.y, w: src.w.min(dest_w), h: src.h.min(dest_h - (y - dest_y)) }, dest_x, y);
        y += src.h.max(1);
    }
}

fn stamp_rect(skin: &Skin, buf: &mut [u8], stride: u32, height: u32, src: Rect, dest_x: u32, dest_y: u32) {
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
            if dx >= stride || dy >= height {
                continue;
            }
            let di = ((dy * stride + dx) * 4) as usize;
            buf[di..di + 4].copy_from_slice(&skin.atlas[si..si + 4]);
        }
    }
}
