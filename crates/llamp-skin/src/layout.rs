//! Fixture-locked rectangles. `docs/spec/skin-atlas.md` is the table. Do not invent another one.

use crate::{Control, Rect, Sprite};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    Main,
    Titlebar,
    Cbuttons,
    Shufrep,
    Posbar,
    Volume,
    Balance,
    Monoster,
    Playpaus,
    Numbers,
    Text,
}

impl Sheet {
    pub fn file_stem(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Titlebar => "titlebar",
            Self::Cbuttons => "cbuttons",
            Self::Shufrep => "shufrep",
            Self::Posbar => "posbar",
            Self::Volume => "volume",
            Self::Balance => "balance",
            Self::Monoster => "monoster",
            Self::Playpaus => "playpaus",
            Self::Numbers => "numbers",
            Self::Text => "text",
        }
    }

    pub fn color_key(self) -> bool {
        self != Self::Main
    }

    /// Sheet must be at least this size to contain the fixture rectangles.
    pub fn min_size(self) -> (u32, u32) {
        match self {
            Self::Main => (275, 116),
            Self::Titlebar => (275, 28),
            Self::Cbuttons => (139, 37),
            Self::Shufrep => (93, 25),
            Self::Posbar => (249, 23),
            Self::Volume => (69, 28),
            Self::Balance => (69, 28),
            Self::Monoster => (58, 13),
            Self::Playpaus => (30, 10),
            Self::Numbers => (118, 14),
            Self::Text => (80, 42),
        }
    }
}

pub struct Slice {
    pub sprite: Sprite,
    pub sheet: Sheet,
    pub src: Rect,
    pub blit: Option<Rect>,
}

const fn r(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

pub fn slices() -> &'static [Slice] {
    &SLICES
}

pub fn controls() -> &'static [Control] {
    &CONTROLS
}

pub fn control_label(control: Control) -> &'static str {
    match control {
        Control::Titlebar => "Title bar",
        Control::Minimize => "Minimize",
        Control::Shade => "Shade",
        Control::Close => "Close",
        Control::ClutterO => "Options",
        Control::ClutterA => "Always on top",
        Control::ClutterI => "File info",
        Control::ClutterD => "Double size",
        Control::ClutterV => "Visualizer",
        Control::Previous => "Previous",
        Control::Play => "Play",
        Control::Pause => "Pause",
        Control::Stop => "Stop",
        Control::Next => "Next",
        Control::Eject => "Eject",
        Control::Seek => "Seek",
        Control::Volume => "Volume",
        Control::Balance => "Balance",
        Control::Mono => "Mono",
        Control::Stereo => "Stereo",
        Control::EqToggle => "Equalizer",
        Control::PlaylistToggle => "Playlist",
        Control::Shuffle => "Shuffle",
        Control::Repeat => "Repeat",
        Control::Time => "Time",
        Control::Marquee => "Marquee",
        Control::VisPane => "Visualizer pane",
    }
}

pub fn control_from_id(id: u32) -> Option<Control> {
    CONTROLS.iter().copied().find(|control| *control as u32 == id)
}

pub fn control_rect(control: Control) -> Rect {
    match control {
        Control::Titlebar => r(0, 0, 275, 14),
        Control::Minimize => r(244, 3, 9, 9),
        Control::Shade => r(254, 3, 9, 9),
        Control::Close => r(264, 3, 9, 9),
        Control::ClutterO => r(10, 3, 8, 8),
        Control::ClutterA => r(20, 3, 8, 8),
        Control::ClutterI => r(30, 3, 8, 8),
        Control::ClutterD => r(40, 3, 8, 8),
        Control::ClutterV => r(50, 3, 8, 8),
        Control::Previous => r(8, 98, 23, 18),
        Control::Play => r(31, 98, 23, 18),
        Control::Pause => r(54, 98, 23, 18),
        Control::Stop => r(77, 98, 23, 18),
        Control::Next => r(100, 98, 23, 18),
        Control::Eject => r(123, 98, 23, 18),
        Control::Seek => r(8, 86, 248, 10),
        Control::Volume => r(90, 70, 68, 14),
        Control::Balance => r(166, 70, 68, 14),
        Control::Mono => r(8, 22, 28, 12),
        Control::Stereo => r(40, 22, 28, 12),
        Control::EqToggle => r(202, 98, 23, 12),
        Control::PlaylistToggle => r(225, 98, 23, 12),
        Control::Shuffle => r(156, 98, 23, 12),
        Control::Repeat => r(179, 98, 23, 12),
        Control::Time => r(72, 22, 36, 13),
        Control::Marquee => r(8, 42, 35, 7),
        Control::VisPane => r(24, 52, 76, 16),
    }
}

pub const DIGIT_ORIGIN: (u32, u32) = (72, 22);
pub const MARQUEE_ORIGIN: (u32, u32) = (8, 42);
pub const MARQUEE_TEXT: &str = "FIXTURE";
pub const TIME_TEXT: &str = "0:00";
pub const GLYPH_CELL: (u32, u32) = (5, 7);
pub const GLYPH_COLUMNS: u32 = 16;
pub const ATLAS_STRIDE: u32 = 512;

const CONTROLS: [Control; 27] = [
    Control::Titlebar,
    Control::Minimize,
    Control::Shade,
    Control::Close,
    Control::ClutterO,
    Control::ClutterA,
    Control::ClutterI,
    Control::ClutterD,
    Control::ClutterV,
    Control::Previous,
    Control::Play,
    Control::Pause,
    Control::Stop,
    Control::Next,
    Control::Eject,
    Control::Seek,
    Control::Volume,
    Control::Balance,
    Control::Mono,
    Control::Stereo,
    Control::EqToggle,
    Control::PlaylistToggle,
    Control::Shuffle,
    Control::Repeat,
    Control::Time,
    Control::Marquee,
    Control::VisPane,
];

static SLICES: [Slice; 63] = [
    Slice {
        sprite: Sprite::Main,
        sheet: Sheet::Main,
        src: r(0, 0, 275, 116),
        blit: Some(r(0, 0, 275, 116)),
    },
    Slice {
        sprite: Sprite::Titlebar,
        sheet: Sheet::Titlebar,
        src: r(0, 0, 275, 14),
        blit: Some(r(0, 0, 275, 14)),
    },
    Slice {
        sprite: Sprite::TitlebarPressed,
        sheet: Sheet::Titlebar,
        src: r(0, 14, 275, 14),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClutterO,
        sheet: Sheet::Titlebar,
        src: r(10, 3, 8, 8),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClutterOPressed,
        sheet: Sheet::Titlebar,
        src: r(10, 17, 8, 8),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClutterA,
        sheet: Sheet::Titlebar,
        src: r(20, 3, 8, 8),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClutterAPressed,
        sheet: Sheet::Titlebar,
        src: r(20, 17, 8, 8),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClutterI,
        sheet: Sheet::Titlebar,
        src: r(30, 3, 8, 8),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClutterIPressed,
        sheet: Sheet::Titlebar,
        src: r(30, 17, 8, 8),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClutterD,
        sheet: Sheet::Titlebar,
        src: r(40, 3, 8, 8),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClutterDPressed,
        sheet: Sheet::Titlebar,
        src: r(40, 17, 8, 8),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClutterV,
        sheet: Sheet::Titlebar,
        src: r(50, 3, 8, 8),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClutterVPressed,
        sheet: Sheet::Titlebar,
        src: r(50, 17, 8, 8),
        blit: None,
    },
    Slice {
        sprite: Sprite::Minimize,
        sheet: Sheet::Titlebar,
        src: r(244, 3, 9, 9),
        blit: None,
    },
    Slice {
        sprite: Sprite::MinimizePressed,
        sheet: Sheet::Titlebar,
        src: r(244, 17, 9, 9),
        blit: None,
    },
    Slice {
        sprite: Sprite::Shade,
        sheet: Sheet::Titlebar,
        src: r(254, 3, 9, 9),
        blit: None,
    },
    Slice {
        sprite: Sprite::ShadePressed,
        sheet: Sheet::Titlebar,
        src: r(254, 17, 9, 9),
        blit: None,
    },
    Slice {
        sprite: Sprite::Close,
        sheet: Sheet::Titlebar,
        src: r(264, 3, 9, 9),
        blit: None,
    },
    Slice {
        sprite: Sprite::ClosePressed,
        sheet: Sheet::Titlebar,
        src: r(264, 17, 9, 9),
        blit: None,
    },
    Slice {
        sprite: Sprite::Previous,
        sheet: Sheet::Cbuttons,
        src: r(1, 1, 23, 18),
        blit: Some(r(8, 98, 23, 18)),
    },
    Slice {
        sprite: Sprite::PreviousPressed,
        sheet: Sheet::Cbuttons,
        src: r(1, 19, 23, 18),
        blit: None,
    },
    Slice {
        sprite: Sprite::Play,
        sheet: Sheet::Cbuttons,
        src: r(24, 1, 23, 18),
        blit: Some(r(31, 98, 23, 18)),
    },
    Slice {
        sprite: Sprite::PlayPressed,
        sheet: Sheet::Cbuttons,
        src: r(24, 19, 23, 18),
        blit: None,
    },
    Slice {
        sprite: Sprite::Pause,
        sheet: Sheet::Cbuttons,
        src: r(47, 1, 23, 18),
        blit: Some(r(54, 98, 23, 18)),
    },
    Slice {
        sprite: Sprite::PausePressed,
        sheet: Sheet::Cbuttons,
        src: r(47, 19, 23, 18),
        blit: None,
    },
    Slice {
        sprite: Sprite::Stop,
        sheet: Sheet::Cbuttons,
        src: r(70, 1, 23, 18),
        blit: Some(r(77, 98, 23, 18)),
    },
    Slice {
        sprite: Sprite::StopPressed,
        sheet: Sheet::Cbuttons,
        src: r(70, 19, 23, 18),
        blit: None,
    },
    Slice {
        sprite: Sprite::Next,
        sheet: Sheet::Cbuttons,
        src: r(93, 1, 23, 18),
        blit: Some(r(100, 98, 23, 18)),
    },
    Slice {
        sprite: Sprite::NextPressed,
        sheet: Sheet::Cbuttons,
        src: r(93, 19, 23, 18),
        blit: None,
    },
    Slice {
        sprite: Sprite::Eject,
        sheet: Sheet::Cbuttons,
        src: r(116, 1, 23, 18),
        blit: Some(r(123, 98, 23, 18)),
    },
    Slice {
        sprite: Sprite::EjectPressed,
        sheet: Sheet::Cbuttons,
        src: r(116, 19, 23, 18),
        blit: None,
    },
    Slice {
        sprite: Sprite::ShuffleOff,
        sheet: Sheet::Shufrep,
        src: r(1, 1, 23, 12),
        blit: Some(r(156, 98, 23, 12)),
    },
    Slice {
        sprite: Sprite::ShuffleOn,
        sheet: Sheet::Shufrep,
        src: r(1, 13, 23, 12),
        blit: None,
    },
    Slice {
        sprite: Sprite::RepeatOff,
        sheet: Sheet::Shufrep,
        src: r(24, 1, 23, 12),
        blit: Some(r(179, 98, 23, 12)),
    },
    Slice {
        sprite: Sprite::RepeatOn,
        sheet: Sheet::Shufrep,
        src: r(24, 13, 23, 12),
        blit: None,
    },
    Slice {
        sprite: Sprite::EqOff,
        sheet: Sheet::Shufrep,
        src: r(47, 1, 23, 12),
        blit: Some(r(202, 98, 23, 12)),
    },
    Slice {
        sprite: Sprite::EqOn,
        sheet: Sheet::Shufrep,
        src: r(47, 13, 23, 12),
        blit: None,
    },
    Slice {
        sprite: Sprite::PlaylistOff,
        sheet: Sheet::Shufrep,
        src: r(70, 1, 23, 12),
        blit: Some(r(225, 98, 23, 12)),
    },
    Slice {
        sprite: Sprite::PlaylistOn,
        sheet: Sheet::Shufrep,
        src: r(70, 13, 23, 12),
        blit: None,
    },
    Slice {
        sprite: Sprite::SeekBar,
        sheet: Sheet::Posbar,
        src: r(1, 1, 248, 10),
        blit: Some(r(8, 86, 248, 10)),
    },
    Slice {
        sprite: Sprite::SeekThumb,
        sheet: Sheet::Posbar,
        src: r(1, 12, 29, 10),
        blit: Some(r(8, 86, 29, 10)),
    },
    Slice {
        sprite: Sprite::VolumeTrack,
        sheet: Sheet::Volume,
        src: r(1, 1, 68, 14),
        blit: Some(r(90, 70, 68, 14)),
    },
    Slice {
        sprite: Sprite::VolumeThumb,
        sheet: Sheet::Volume,
        src: r(1, 16, 14, 11),
        blit: Some(r(90, 72, 14, 11)),
    },
    Slice {
        sprite: Sprite::BalanceTrack,
        sheet: Sheet::Balance,
        src: r(1, 1, 68, 14),
        blit: Some(r(166, 70, 68, 14)),
    },
    Slice {
        sprite: Sprite::BalanceThumb,
        sheet: Sheet::Balance,
        src: r(1, 16, 14, 11),
        blit: Some(r(193, 72, 14, 11)),
    },
    Slice {
        sprite: Sprite::Mono,
        sheet: Sheet::Monoster,
        src: r(1, 1, 28, 12),
        blit: Some(r(8, 22, 28, 12)),
    },
    Slice {
        sprite: Sprite::Stereo,
        sheet: Sheet::Monoster,
        src: r(30, 1, 28, 12),
        blit: Some(r(40, 22, 28, 12)),
    },
    Slice {
        sprite: Sprite::IndicatorPlay,
        sheet: Sheet::Playpaus,
        src: r(1, 1, 9, 9),
        blit: None,
    },
    Slice {
        sprite: Sprite::IndicatorPause,
        sheet: Sheet::Playpaus,
        src: r(11, 1, 9, 9),
        blit: None,
    },
    Slice {
        sprite: Sprite::IndicatorStop,
        sheet: Sheet::Playpaus,
        src: r(21, 1, 9, 9),
        blit: Some(r(112, 24, 9, 9)),
    },
    Slice {
        sprite: Sprite::Digit0,
        sheet: Sheet::Numbers,
        src: r(1, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::Digit1,
        sheet: Sheet::Numbers,
        src: r(10, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::Digit2,
        sheet: Sheet::Numbers,
        src: r(19, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::Digit3,
        sheet: Sheet::Numbers,
        src: r(28, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::Digit4,
        sheet: Sheet::Numbers,
        src: r(37, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::Digit5,
        sheet: Sheet::Numbers,
        src: r(46, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::Digit6,
        sheet: Sheet::Numbers,
        src: r(55, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::Digit7,
        sheet: Sheet::Numbers,
        src: r(64, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::Digit8,
        sheet: Sheet::Numbers,
        src: r(73, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::Digit9,
        sheet: Sheet::Numbers,
        src: r(82, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::DigitColon,
        sheet: Sheet::Numbers,
        src: r(91, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::DigitMinus,
        sheet: Sheet::Numbers,
        src: r(100, 1, 9, 13),
        blit: None,
    },
    Slice {
        sprite: Sprite::DigitBlank,
        sheet: Sheet::Numbers,
        src: r(109, 1, 9, 13),
        blit: None,
    },
];

pub fn digit_sprite(ch: char) -> Option<Sprite> {
    Some(match ch {
        '0' => Sprite::Digit0,
        '1' => Sprite::Digit1,
        '2' => Sprite::Digit2,
        '3' => Sprite::Digit3,
        '4' => Sprite::Digit4,
        '5' => Sprite::Digit5,
        '6' => Sprite::Digit6,
        '7' => Sprite::Digit7,
        '8' => Sprite::Digit8,
        '9' => Sprite::Digit9,
        ':' => Sprite::DigitColon,
        '-' => Sprite::DigitMinus,
        ' ' => Sprite::DigitBlank,
        _ => return None,
    })
}
