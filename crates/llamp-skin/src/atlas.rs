//! One RGBA atlas. Color key is the top-left pixel of a sheet that needs transparency.

use crate::layout::{self, Sheet, Slice};
use crate::{DecodedBmp, Rect, Sprite, FALLBACK_RGBA};

pub struct Packed {
    pub atlas: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub sprites: Vec<(Sprite, Rect)>,
    pub glyphs: Vec<(char, Rect)>,
}

pub fn pack(sheets: &[(Sheet, Option<&DecodedBmp>)], defects: &mut Vec<String>) -> Packed {
    let mut placements = Vec::new();
    let mut glyphs = Vec::new();
    let mut x = 0u32;
    let mut y = 0u32;
    let mut row_h = 0u32;
    let mut sheet_map: Vec<(Sheet, Option<&DecodedBmp>)> = sheets.to_vec();
    sheet_map.sort_by_key(|(sheet, _)| sheet.file_stem());

    let place = |w: u32,
                 h: u32,
                 x: &mut u32,
                 y: &mut u32,
                 row_h: &mut u32,
                 placements: &mut Vec<Rect>|
     -> Rect {
        if *x > 0 && *x + w > layout::ATLAS_STRIDE {
            *y += *row_h;
            *x = 0;
            *row_h = 0;
        }
        let rect = Rect { x: *x, y: *y, w, h };
        *x += w;
        *row_h = (*row_h).max(h);
        placements.push(rect);
        rect
    };

    let mut sprite_rects = Vec::new();
    let mut reported = std::collections::BTreeSet::new();
    for slice in layout::slices() {
        let image = sheet_image(&sheet_map, slice.sheet);
        let usable = image.is_some_and(|img| contains(img, slice));
        if image.is_none() && reported.insert(slice.sheet.file_stem()) {
            defects.push(format!("missing {}.bmp", slice.sheet.file_stem()));
        } else if image.is_some() && !usable && reported.insert(slice.sheet.file_stem()) {
            defects.push(format!("unexpected size {}.bmp", slice.sheet.file_stem()));
        }
        let rect = place(
            slice.src.w,
            slice.src.h,
            &mut x,
            &mut y,
            &mut row_h,
            &mut placements,
        );
        sprite_rects.push((slice, usable, rect));
    }

    let text = sheet_image(&sheet_map, Sheet::Text);
    if let Some(img) = text {
        let (min_w, min_h) = Sheet::Text.min_size();
        if img.width >= min_w && img.height >= min_h {
            for index in 0..95u32 {
                let col = index % layout::GLYPH_COLUMNS;
                let row = index / layout::GLYPH_COLUMNS;
                let src = Rect {
                    x: col * layout::GLYPH_CELL.0,
                    y: row * layout::GLYPH_CELL.1,
                    w: layout::GLYPH_CELL.0,
                    h: layout::GLYPH_CELL.1,
                };
                let rect = place(src.w, src.h, &mut x, &mut y, &mut row_h, &mut placements);
                let ch = char::from_u32(32 + index).expect("ascii");
                glyphs.push((ch, src, rect));
            }
        }
    }

    let height = y + row_h;
    let width = layout::ATLAS_STRIDE;
    let mut atlas = vec![0u8; width as usize * height as usize * 4];
    let mut sprites = Vec::new();
    let mut missing_logged = std::collections::BTreeSet::new();
    for (slice, usable, dest) in sprite_rects {
        if let Some(img) = sheet_image(&sheet_map, slice.sheet).filter(|_| usable) {
            copy_keyed(
                &mut atlas,
                width,
                dest,
                img,
                slice.src,
                slice.sheet.color_key(),
            );
        } else {
            fill(&mut atlas, width, dest, FALLBACK_RGBA);
            if missing_logged.insert(slice.sheet.file_stem()) {
                // defect already recorded per slice; keep one readable line too
                let _ = missing_logged;
            }
        }
        sprites.push((slice.sprite, dest));
    }
    let glyph_rects = glyphs
        .into_iter()
        .map(|(ch, src, dest)| {
            if let Some(img) = text {
                copy_keyed(&mut atlas, width, dest, img, src, true);
            }
            (ch, dest)
        })
        .collect();
    Packed {
        atlas,
        width,
        height,
        sprites,
        glyphs: glyph_rects,
    }
}

fn sheet_image<'a>(
    sheets: &'a [(Sheet, Option<&DecodedBmp>)],
    sheet: Sheet,
) -> Option<&'a DecodedBmp> {
    sheets
        .iter()
        .find(|(id, _)| *id == sheet)
        .and_then(|(_, img)| *img)
}

fn contains(img: &DecodedBmp, slice: &Slice) -> bool {
    let (min_w, min_h) = slice.sheet.min_size();
    img.width >= min_w
        && img.height >= min_h
        && slice.src.x + slice.src.w <= img.width
        && slice.src.y + slice.src.h <= img.height
}

fn copy_keyed(
    atlas: &mut [u8],
    atlas_w: u32,
    dest: Rect,
    src_img: &DecodedBmp,
    src: Rect,
    keyed: bool,
) {
    let key = px(src_img, 0, 0);
    for y in 0..src.h {
        for x in 0..src.w {
            let pixel = px(src_img, src.x + x, src.y + y);
            let mut out = pixel;
            if keyed && pixel[0] == key[0] && pixel[1] == key[1] && pixel[2] == key[2] {
                out[3] = 0;
            } else {
                out[3] = 255;
            }
            put(atlas, atlas_w, dest.x + x, dest.y + y, out);
        }
    }
}

fn fill(atlas: &mut [u8], atlas_w: u32, dest: Rect, color: [u8; 4]) {
    for y in 0..dest.h {
        for x in 0..dest.w {
            put(atlas, atlas_w, dest.x + x, dest.y + y, color);
        }
    }
}

fn px(img: &DecodedBmp, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    img.rgba[i..i + 4].try_into().unwrap_or([0, 0, 0, 255])
}

fn put(atlas: &mut [u8], width: u32, x: u32, y: u32, pixel: [u8; 4]) {
    let i = ((y * width + x) * 4) as usize;
    atlas[i..i + 4].copy_from_slice(&pixel);
}
