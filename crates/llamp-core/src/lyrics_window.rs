//! Sixth window. `gen` chrome is the core blit. The shell draws CoreText in the hole.

use llamp_library::{active_at, clock_ms, clamp_offset, LyricCursor, LyricDoc, LyricKind};

use crate::vis_window::{self, ClientRect, CHROME_BOTTOM, CHROME_LEFT, CHROME_RIGHT, CHROME_TOP};

pub const MIN_W: i32 = vis_window::MIN_W;
pub const MIN_H: i32 = vis_window::MIN_H;
pub const LINE_H: i32 = 16;
pub const TEXT_PAD: i32 = 4;
pub const MARK_W: i32 = 32;
pub const MARK_H: i32 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStyle {
    Idle,
    Active,
    ActiveWord,
}

pub fn propose_size(w: i32, h: i32) -> (i32, i32) {
    vis_window::propose_size(w, h)
}

pub fn client_rect(w: i32, h: i32) -> ClientRect {
    vis_window::client_rect(w, h, false)
}

/// Skin-pixel origin of the CoreText layer. Integer at every allowed scale.
pub fn text_origin(scale: i32) -> (i32, i32) {
    let scale = scale.max(1);
    (
        (CHROME_LEFT + TEXT_PAD) * scale,
        (CHROME_TOP + TEXT_PAD) * scale,
    )
}

pub fn line_rect(index: i32, scroll_px: i32, client_w: i32, scale: i32) -> TextRect {
    let scale = scale.max(1);
    let (x, y0) = text_origin(scale);
    TextRect {
        x,
        y: y0 + (index * LINE_H - scroll_px) * scale,
        w: (client_w - 2 * TEXT_PAD) * scale,
        h: LINE_H * scale,
    }
}

/// Empty-state mark. Integer nearest-neighbor box, not composited into chrome.
pub fn mark_rect(window_w: i32, window_h: i32, scale: i32) -> TextRect {
    let scale = scale.max(1);
    let client = client_rect(window_w, window_h);
    let w = MARK_W * scale;
    let h = MARK_H * scale;
    TextRect {
        x: (client.x + (client.w - MARK_W) / 2) * scale,
        y: (client.y + (client.h - MARK_H) / 2) * scale,
        w,
        h,
    }
}

pub fn scroll_for_active(line: usize, client_h: i32) -> i32 {
    let center = (client_h - 2 * TEXT_PAD) / 2;
    let target = line as i32 * LINE_H + LINE_H / 2;
    (target - center).max(0)
}

pub fn cursor_for(doc: &LyricDoc, position_frames: u64, sample_rate: u32, user_offset_ms: i32) -> LyricCursor {
    if doc.kind != LyricKind::Synced {
        return LyricCursor {
            line: None,
            word: None,
        };
    }
    active_at(doc, clock_ms(position_frames, sample_rate, user_offset_ms))
}

pub fn line_style(cursor: LyricCursor, line: usize, word: Option<usize>) -> LineStyle {
    match cursor.line {
        Some(active) if active == line => match (cursor.word, word) {
            (Some(cur), Some(w)) if cur == w => LineStyle::ActiveWord,
            (Some(_), Some(_)) => LineStyle::Active,
            _ => LineStyle::Active,
        },
        _ => LineStyle::Idle,
    }
}

pub fn step_offset(current: i32, steps: i32) -> i32 {
    clamp_offset(current + steps * llamp_library::OFFSET_STEP_MS)
}

pub const PRIVACY_NOTE: &str =
    "Look up lyrics? We will send artist, title, album, and duration to LRCLIB.";

pub const SIDECAR_NOT_WRITTEN: &str = "The sidecar was not written.";

#[allow(dead_code)]
fn _chrome_insets() -> (i32, i32, i32, i32) {
    (CHROME_LEFT, CHROME_TOP, CHROME_RIGHT, CHROME_BOTTOM)
}
