//! EQ window blit. 275×116 and shade 14 are locked sizes.
//! Slider and graph rectangles are a phase-4 layout formula, not `eqmain` slices.
//! Those origins are not in `docs/spec/skin-atlas.md` and are not guessed.

use crate::{default_vis_colors, Rect, Rgb, FALLBACK_RGBA, MAIN_HEIGHT, MAIN_WIDTH, SHADE_HEIGHT};

pub const EQ_WIDTH: u32 = MAIN_WIDTH;
pub const EQ_HEIGHT: u32 = MAIN_HEIGHT;

pub const EQ_ON: u32 = 1;
pub const EQ_AUTO: u32 = 2;
pub const EQ_PRESETS: u32 = 3;
pub const EQ_SHADE: u32 = 4;
pub const EQ_CLOSE: u32 = 5;
pub const EQ_PREAMP: u32 = 6;
pub const EQ_BAND0: u32 = 7;

/// Published notes: AUTO loads the auto-load preset for this track. Not a preamp law.
pub const AUTO_LABEL: &str = "Auto-load preset for this track";
pub const DISABLED_CAPTION: &str = "Equalizer does not apply — playback is remote";
pub const APPLYING_CAPTION: &str = "Equalizer";

const SLIDERS: u32 = 11;
const THUMB: u32 = 8;
const SLIDER_TOP: u32 = 58;
const SLIDER_BOTTOM_MARGIN: u32 = 6;
const TRACK: u32 = EQ_HEIGHT - SLIDER_TOP - THUMB - SLIDER_BOTTOM_MARGIN;
const GRAPH: Rect = Rect { x: 4, y: 16, w: 267, h: 38 };

pub struct EqControl {
    pub id: u32,
    pub label: &'static str,
    pub rect: Rect,
}

pub struct EqPaint {
    pub on: bool,
    pub auto_on: bool,
    pub applies: bool,
    pub preamp_db: f32,
    pub bands: [f32; 10],
    pub curve_db: Vec<f32>,
    pub vis: [Rgb; 24],
}

impl Default for EqPaint {
    fn default() -> Self {
        Self {
            on: false,
            auto_on: false,
            applies: true,
            preamp_db: 0.0,
            bands: [0.0; 10],
            curve_db: Vec::new(),
            vis: default_vis_colors(),
        }
    }
}

pub fn eq_controls() -> Vec<EqControl> {
    let mut controls = vec![
        EqControl { id: 0, label: "Title bar", rect: Rect { x: 0, y: 0, w: 244, h: SHADE_HEIGHT } },
        EqControl {
            id: EQ_ON,
            label: "On",
            rect: Rect { x: 2, y: 1, w: 23, h: 12 },
        },
        EqControl {
            id: EQ_AUTO,
            label: AUTO_LABEL,
            rect: Rect { x: 27, y: 1, w: 23, h: 12 },
        },
        EqControl {
            id: EQ_PRESETS,
            label: "Presets",
            rect: Rect { x: 52, y: 1, w: 48, h: 12 },
        },
        // Locked titlebar slots from `docs/spec/skin-atlas.md`, not an eqmain origin.
        EqControl {
            id: EQ_SHADE,
            label: "Shade",
            rect: Rect { x: 254, y: 3, w: 9, h: 9 },
        },
        EqControl {
            id: EQ_CLOSE,
            label: "Close",
            rect: Rect { x: 264, y: 3, w: 9, h: 9 },
        },
        EqControl {
            id: EQ_PREAMP,
            label: "Preamp",
            rect: slider_rect(0),
        },
    ];
    for band in 0..10 {
        controls.push(EqControl {
            id: EQ_BAND0 + band,
            label: "Band",
            rect: slider_rect(band + 1),
        });
    }
    controls
}

pub fn blit_eq(paint: &EqPaint) -> Vec<u8> {
    let mut buf = vec![0u8; EQ_WIDTH as usize * EQ_HEIGHT as usize * 4];
    for pixel in buf.chunks_exact_mut(4) {
        pixel.copy_from_slice(&FALLBACK_RGBA);
    }
    let vis = if paint.vis.iter().all(|c| c.r == 0 && c.g == 0 && c.b == 0) {
        default_vis_colors()
    } else {
        paint.vis
    };
    fill_rect(&mut buf, control_rect(EQ_ON), if paint.on { vis[23] } else { vis[17] });
    fill_rect(&mut buf, control_rect(EQ_AUTO), if paint.auto_on { vis[23] } else { vis[17] });
    fill_rect(&mut buf, control_rect(EQ_PRESETS), vis[2]);
    fill_rect(&mut buf, control_rect(EQ_SHADE), vis[18]);
    fill_rect(&mut buf, control_rect(EQ_CLOSE), vis[23]);
    for index in 0..SLIDERS {
        let track = slider_rect(index);
        fill_rect(&mut buf, track, vis[0]);
        let db = if index == 0 {
            paint.preamp_db
        } else {
            paint.bands[(index - 1) as usize]
        };
        let thumb = Rect { x: track.x, y: thumb_y(db), w: THUMB, h: THUMB };
        fill_rect(&mut buf, thumb, if paint.applies { vis[23] } else { vis[1] });
    }
    draw_curve(&mut buf, paint, &vis);
    buf
}

fn draw_curve(buf: &mut [u8], paint: &EqPaint, vis: &[Rgb; 24]) {
    let color = if paint.applies && paint.on { vis[22] } else { vis[1] };
    let samples = GRAPH.w as usize;
    for x in 0..samples {
        let db = if paint.applies && paint.on {
            sample_curve(&paint.curve_db, x, samples)
        } else {
            0.0
        };
        let y = graph_y(db);
        put(buf, GRAPH.x + x as u32, y, color);
    }
}

fn sample_curve(curve: &[f32], x: usize, width: usize) -> f32 {
    if curve.is_empty() || width <= 1 {
        return 0.0;
    }
    let t = x as f32 / (width - 1) as f32;
    let pos = t * (curve.len() - 1) as f32;
    let i = pos.floor() as usize;
    let frac = pos - i as f32;
    let a = curve[i.min(curve.len() - 1)];
    let b = curve[(i + 1).min(curve.len() - 1)];
    a + (b - a) * frac
}

fn graph_y(db: f32) -> u32 {
    let t = (12.0 - db.clamp(-12.0, 12.0)) / 24.0;
    GRAPH.y + (t * (GRAPH.h - 1) as f32) as u32
}

fn thumb_y(db: f32) -> u32 {
    let t = (12.0 - db.clamp(-12.0, 12.0)) / 24.0;
    SLIDER_TOP + (t * TRACK as f32) as u32
}

/// Even spacing from the window width. Not a copied slider slot.
pub fn slider_x(index: u32) -> u32 {
    let gaps = SLIDERS + 1;
    let gap = (EQ_WIDTH - SLIDERS * THUMB) / gaps;
    let used = SLIDERS * THUMB + gaps * gap;
    let pad = (EQ_WIDTH - used) / 2;
    pad + gap + index * (THUMB + gap)
}

fn slider_rect(index: u32) -> Rect {
    Rect { x: slider_x(index), y: SLIDER_TOP, w: THUMB, h: TRACK + THUMB }
}

fn control_rect(id: u32) -> Rect {
    eq_controls()
        .into_iter()
        .find(|control| control.id == id)
        .map(|control| control.rect)
        .unwrap_or(Rect { x: 0, y: 0, w: 0, h: 0 })
}

fn fill_rect(buf: &mut [u8], rect: Rect, color: Rgb) {
    for y in rect.y..rect.y.saturating_add(rect.h).min(EQ_HEIGHT) {
        for x in rect.x..rect.x.saturating_add(rect.w).min(EQ_WIDTH) {
            put(buf, x, y, color);
        }
    }
}

fn put(buf: &mut [u8], x: u32, y: u32, color: Rgb) {
    if x >= EQ_WIDTH || y >= EQ_HEIGHT {
        return;
    }
    let i = ((y * EQ_WIDTH + x) * 4) as usize;
    buf[i] = color.r;
    buf[i + 1] = color.g;
    buf[i + 2] = color.b;
    buf[i + 3] = 255;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_not_a_copied_eqmain_slot_table() {
        let xs: Vec<u32> = (0..11).map(slider_x).collect();
        let copied = [21, 78, 96, 114, 132, 150, 168, 186, 204, 222, 240];
        assert!(xs.iter().all(|x| !copied.contains(x)));
        assert_eq!(EQ_WIDTH, 275);
        assert_eq!(EQ_HEIGHT, 116);
    }

    #[test]
    fn sliders_stop_above_the_window_edge() {
        let track = slider_rect(0);
        assert_eq!(track.y + track.h + SLIDER_BOTTOM_MARGIN, EQ_HEIGHT);
        assert!(track.y + track.h < EQ_HEIGHT);
        let min_thumb = thumb_y(-12.0);
        assert!(min_thumb + THUMB <= EQ_HEIGHT - SLIDER_BOTTOM_MARGIN);
    }

    #[test]
    fn disabled_curve_uses_viscolor_index_1() {
        let mut paint = EqPaint::default();
        paint.applies = false;
        paint.on = true;
        let buf = blit_eq(&paint);
        let x = GRAPH.x + GRAPH.w / 2;
        let y = graph_y(0.0);
        let i = ((y * EQ_WIDTH + x) * 4) as usize;
        let vis = default_vis_colors();
        assert_eq!(&buf[i..i + 3], &[vis[1].r, vis[1].g, vis[1].b]);
    }
}
