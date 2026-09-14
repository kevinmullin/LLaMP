//! Fifth window. `gen` chrome owns the border. The wgpu client is the hole.

pub const MIN_W: i32 = 275;
pub const MIN_H: i32 = 116;
pub const CHROME_LEFT: i32 = 8;
pub const CHROME_RIGHT: i32 = 8;
pub const CHROME_TOP: i32 = 14;
pub const CHROME_BOTTOM: i32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub fn propose_size(w: i32, h: i32) -> (i32, i32) {
    (w.max(MIN_W), h.max(MIN_H))
}

/// Chrome pixels stay with the core blit. The GPU view is this rectangle only.
pub fn client_rect(w: i32, h: i32, fullscreen: bool) -> ClientRect {
    let (w, h) = propose_size(w, h);
    if fullscreen {
        return ClientRect { x: 0, y: 0, w, h };
    }
    ClientRect {
        x: CHROME_LEFT,
        y: CHROME_TOP,
        w: w - CHROME_LEFT - CHROME_RIGHT,
        h: h - CHROME_TOP - CHROME_BOTTOM,
    }
}
