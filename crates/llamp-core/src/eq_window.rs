//! EQ window state. Snap and dock are skin pixels from the architecture note.
//! AUTO loads a filename preset. That is the published-notes result, not a preamp law.
//! Presets live in a core-owned file. The library key/value table is phase 5b.

use std::fs;
use std::path::{Path, PathBuf};

use llamp_audio::{drag_band, drag_preamp, set_eq_enabled};

const SNAP: i32 = 10;
const UNDOCK2: i32 = 12 * 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Frame {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Main = 0,
    Eq = 1,
}

impl Which {
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Main),
            1 => Some(Self::Eq),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gains {
    pub preamp: i16,
    pub bands: [i16; 10],
}

impl Default for Gains {
    fn default() -> Self {
        Self { preamp: 0, bands: [0; 10] }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Named {
    name: String,
    gains: Gains,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DockMove {
    pub main: Frame,
    pub eq: Frame,
    pub docked: bool,
    pub group_on_top: bool,
}

pub struct EqWindow {
    on: bool,
    auto: bool,
    supports_eq: bool,
    eq_on_top: bool,
    presets: Vec<Named>,
    autoloads: Vec<Named>,
    default_gain: Option<Gains>,
    store: Option<PathBuf>,
    main: Frame,
    eq: Frame,
    docked: bool,
    accum_x: i32,
    accum_y: i32,
    last_track: String,
}

impl EqWindow {
    pub fn new() -> Self {
        Self {
            on: false,
            auto: false,
            supports_eq: true,
            eq_on_top: false,
            presets: Vec::new(),
            autoloads: Vec::new(),
            default_gain: None,
            store: None,
            main: Frame::new(0, 0, 275, 116),
            eq: Frame::new(0, 140, 275, 116),
            docked: false,
            accum_x: 0,
            accum_y: 0,
            last_track: String::new(),
        }
    }

    pub fn on(&self) -> bool {
        self.on
    }

    pub fn auto(&self) -> bool {
        self.auto
    }

    pub fn supports_eq(&self) -> bool {
        self.supports_eq
    }

    pub fn docked(&self) -> bool {
        self.docked
    }

    pub fn set_supports(&mut self, on: bool) {
        self.supports_eq = on;
        if !on {
            self.on = false;
            set_eq_enabled(false);
        }
    }

    pub fn toggle_on(&mut self) {
        if !self.supports_eq {
            return;
        }
        self.on = !self.on;
        set_eq_enabled(self.on);
        if self.on && self.auto {
            self.apply_track_preset();
        }
    }

    pub fn toggle_auto(&mut self) {
        if !self.supports_eq {
            return;
        }
        self.auto = !self.auto;
        if self.auto && self.on {
            self.apply_track_preset();
        }
    }

    /// Slider drag. Writes atomic targets. Does not redesign coefficients.
    pub fn drag_slider(&mut self, preamp: bool, band: usize, millidb: i32) {
        if !self.supports_eq {
            return;
        }
        let db = millidb.clamp(-12_000, 12_000) as f32 / 1000.0;
        if preamp {
            drag_preamp(db);
        } else if band < 10 {
            drag_band(band, db);
        }
    }

    pub fn current_gains(&self) -> Gains {
        Gains {
            preamp: (llamp_audio::preamp_target_db() * 1000.0).round() as i16,
            bands: std::array::from_fn(|band| (llamp_audio::band_target_db(band) * 1000.0).round() as i16),
        }
    }

    pub fn save_preset(&mut self, name: &str) -> Result<(), String> {
        let name = clean_name(name)?;
        let gains = self.current_gains();
        if let Some(slot) = self.presets.iter_mut().find(|preset| preset.name == name) {
            slot.gains = gains;
        } else {
            self.presets.push(Named { name, gains });
        }
        self.write_store()
    }

    pub fn load_preset(&mut self, name: &str) -> Result<(), String> {
        if !self.supports_eq {
            return Err("equalizer does not apply".into());
        }
        let gains = self
            .presets
            .iter()
            .find(|preset| preset.name == name)
            .map(|preset| preset.gains)
            .ok_or_else(|| format!("no preset {name}"))?;
        self.apply(gains);
        Ok(())
    }

    pub fn save_autoload(&mut self) -> Result<(), String> {
        if self.last_track.is_empty() {
            return Err("no track".into());
        }
        let name = self.last_track.clone();
        let gains = self.current_gains();
        if let Some(slot) = self.autoloads.iter_mut().find(|row| row.name == name) {
            slot.gains = gains;
        } else {
            self.autoloads.push(Named { name, gains });
        }
        self.write_store()
    }

    pub fn save_default(&mut self) -> Result<(), String> {
        self.default_gain = Some(self.current_gains());
        self.write_store()
    }

    pub fn preset_names(&self) -> Vec<String> {
        self.presets.iter().map(|preset| preset.name.clone()).collect()
    }

    pub fn set_store(&mut self, path: &Path) -> Result<(), String> {
        self.store = Some(path.to_path_buf());
        if path.exists() {
            let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
            self.read_store(&text)?;
        }
        Ok(())
    }

    pub fn note_track(&mut self, filename: &str) {
        if filename.is_empty() || filename == self.last_track {
            return;
        }
        self.last_track = filename.to_string();
        if self.auto && self.on && self.supports_eq {
            self.apply_track_preset();
        }
    }

    pub fn set_frames(&mut self, main: Frame, eq: Frame) {
        self.main = main;
        self.eq = eq;
    }

    pub fn begin_drag(&mut self) {
        self.accum_x = 0;
        self.accum_y = 0;
    }

    pub fn drag(&mut self, which: Which, dx: i32, dy: i32) -> DockMove {
        self.accum_x = self.accum_x.saturating_add(dx);
        self.accum_y = self.accum_y.saturating_add(dy);
        let travel = self.accum_x.saturating_mul(self.accum_x) + self.accum_y.saturating_mul(self.accum_y);
        if self.docked && travel > UNDOCK2 {
            self.docked = false;
            self.move_one(which, dx, dy);
        } else if self.docked {
            self.main.x += dx;
            self.main.y += dy;
            self.eq.x += dx;
            self.eq.y += dy;
        } else {
            self.move_one(which, dx, dy);
            match which {
                Which::Eq => snap(&mut self.eq, self.main),
                Which::Main => snap(&mut self.main, self.eq),
            }
        }
        self.move_now()
    }

    pub fn end_drag(&mut self) -> DockMove {
        if touching(self.main, self.eq) {
            self.docked = true;
        }
        self.accum_x = 0;
        self.accum_y = 0;
        self.move_now()
    }

    pub fn window_on_top(&self, which: Which, main_on_top: bool) -> bool {
        if self.docked {
            main_on_top || self.eq_on_top
        } else {
            match which {
                Which::Main => main_on_top,
                Which::Eq => self.eq_on_top,
            }
        }
    }

    fn move_one(&mut self, which: Which, dx: i32, dy: i32) {
        let frame = match which {
            Which::Main => &mut self.main,
            Which::Eq => &mut self.eq,
        };
        frame.x += dx;
        frame.y += dy;
    }

    fn move_now(&self) -> DockMove {
        DockMove {
            main: self.main,
            eq: self.eq,
            docked: self.docked,
            group_on_top: self.docked && self.eq_on_top,
        }
    }

    fn apply_track_preset(&mut self) {
        if let Some(row) = self.autoloads.iter().find(|row| row.name == self.last_track) {
            self.apply(row.gains);
            return;
        }
        if let Some(gains) = self.default_gain {
            self.apply(gains);
        }
    }

    fn apply(&self, gains: Gains) {
        drag_preamp(gains.preamp as f32 / 1000.0);
        for (band, milli) in gains.bands.iter().enumerate() {
            drag_band(band, *milli as f32 / 1000.0);
        }
    }

    fn write_store(&self) -> Result<(), String> {
        let Some(path) = &self.store else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(path, self.encode()).map_err(|err| err.to_string())
    }

    fn encode(&self) -> String {
        let mut out = String::from("# llamp-eq 1\n");
        for preset in &self.presets {
            out.push_str(&format!("P {}\n", preset.name));
            push_gains(&mut out, preset.gains);
        }
        for row in &self.autoloads {
            out.push_str(&format!("A {}\n", row.name));
            push_gains(&mut out, row.gains);
        }
        if let Some(gains) = self.default_gain {
            out.push_str("D\n");
            push_gains(&mut out, gains);
        }
        out
    }

    fn read_store(&mut self, text: &str) -> Result<(), String> {
        self.presets.clear();
        self.autoloads.clear();
        self.default_gain = None;
        let mut pending: Option<Pending> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (kind, rest) = line.split_once(' ').unwrap_or((line, ""));
            match kind {
                "P" => pending = Some(Pending::Preset(rest.to_string())),
                "A" => pending = Some(Pending::Auto(rest.to_string())),
                "D" => pending = Some(Pending::Default),
                "G" => {
                    let gains = parse_gains(rest)?;
                    match pending.take() {
                        Some(Pending::Preset(name)) => self.presets.push(Named { name, gains }),
                        Some(Pending::Auto(name)) => self.autoloads.push(Named { name, gains }),
                        Some(Pending::Default) => self.default_gain = Some(gains),
                        None => return Err("gain without a preset".into()),
                    }
                }
                _ => return Err(format!("unknown preset line {kind}")),
            }
        }
        Ok(())
    }
}

impl Default for EqWindow {
    fn default() -> Self {
        Self::new()
    }
}

enum Pending {
    Preset(String),
    Auto(String),
    Default,
}

fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.contains(['\n', '\r']) {
        return Err("preset name is empty".into());
    }
    Ok(name.to_string())
}

fn push_gains(out: &mut String, gains: Gains) {
    out.push_str("G ");
    out.push_str(&gains.preamp.to_string());
    for band in gains.bands {
        out.push(' ');
        out.push_str(&band.to_string());
    }
    out.push('\n');
}

fn parse_gains(rest: &str) -> Result<Gains, String> {
    let parts: Vec<i16> = rest
        .split_whitespace()
        .map(|part| part.parse::<i16>().map_err(|_| format!("gain {part}")))
        .collect::<Result<_, _>>()?;
    if parts.len() != 11 {
        return Err("preset gain line needs preamp plus 10 bands".into());
    }
    let mut bands = [0i16; 10];
    bands.copy_from_slice(&parts[1..]);
    Ok(Gains { preamp: parts[0], bands })
}

fn snap(mover: &mut Frame, other: Frame) {
    if (mover.y - (other.y + other.h)).abs() <= SNAP {
        mover.y = other.y + other.h;
    } else if ((mover.y + mover.h) - other.y).abs() <= SNAP {
        mover.y = other.y - mover.h;
    }
    if (mover.x - (other.x + other.w)).abs() <= SNAP {
        mover.x = other.x + other.w;
    } else if ((mover.x + mover.w) - other.x).abs() <= SNAP {
        mover.x = other.x - mover.w;
    }
    if vertically_adjacent(*mover, other) && (mover.x - other.x).abs() <= SNAP {
        mover.x = other.x;
    }
    if horizontally_adjacent(*mover, other) && (mover.y - other.y).abs() <= SNAP {
        mover.y = other.y;
    }
}

fn touching(a: Frame, b: Frame) -> bool {
    (vertically_adjacent(a, b) && overlaps(a.x, a.w, b.x, b.w))
        || (horizontally_adjacent(a, b) && overlaps(a.y, a.h, b.y, b.h))
}

fn vertically_adjacent(a: Frame, b: Frame) -> bool {
    a.y + a.h == b.y || b.y + b.h == a.y
}

fn horizontally_adjacent(a: Frame, b: Frame) -> bool {
    a.x + a.w == b.x || b.x + b.w == a.x
}

fn overlaps(origin: i32, len: i32, other: i32, other_len: i32) -> bool {
    origin < other + other_len && other < origin + len
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn audio_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap()
    }

    fn pair() -> EqWindow {
        let mut eq = EqWindow::new();
        eq.set_frames(Frame::new(0, 0, 275, 116), Frame::new(8, 124, 275, 116));
        eq
    }

    #[test]
    fn snap_within_10_docks_on_mouse_up_and_a_docked_drag_moves_both() {
        let mut eq = pair();
        eq.begin_drag();
        let moved = eq.drag(Which::Eq, -2, -8);
        assert_eq!(moved.eq.y, 116);
        assert_eq!(moved.eq.x, 0);
        let ended = eq.end_drag();
        assert!(ended.docked);
        eq.begin_drag();
        let both = eq.drag(Which::Main, 4, 5);
        assert!(both.docked);
        assert_eq!(both.main, Frame::new(4, 5, 275, 116));
        assert_eq!(both.eq, Frame::new(4, 121, 275, 116));
    }

    #[test]
    fn a_drag_past_12_undocks_that_window() {
        let mut eq = pair();
        eq.begin_drag();
        eq.drag(Which::Eq, -6, -8);
        eq.end_drag();
        eq.begin_drag();
        let stayed = eq.drag(Which::Eq, 0, 12);
        assert!(stayed.docked);
        eq.end_drag();
        let origin = stayed.eq;
        eq.begin_drag();
        let solo = eq.drag(Which::Eq, 0, 13);
        assert!(!solo.docked);
        assert_eq!(solo.main, stayed.main);
        assert_eq!(solo.eq.y, origin.y + 13);
    }

    #[test]
    fn auto_does_not_write_a_preamp_law() {
        let _guard = audio_lock();
        let mut eq = EqWindow::new();
        drag_band(4, 12.0);
        let before = llamp_audio::preamp_target_db();
        eq.toggle_auto();
        assert!(eq.auto());
        assert_eq!(llamp_audio::preamp_target_db(), before);
        eq.toggle_on();
        assert!(eq.on());
        assert_eq!(llamp_audio::preamp_target_db(), before);
    }

    #[test]
    fn preset_load_jumps_targets_and_the_callback_still_slews() {
        let _guard = audio_lock();
        let rate = 48_000u32;
        let mut stage = llamp_audio::playback_stage(rate);
        llamp_audio::set_eq_enabled(true);
        let mut eq = EqWindow::new();
        eq.drag_slider(false, 4, 0);
        eq.save_preset("flat").unwrap();
        eq.drag_slider(false, 4, 12_000);
        eq.save_preset("loud").unwrap();
        eq.load_preset("flat").unwrap();

        let n = rate as usize / 10;
        let mut tone = vec![0f32; n * 2];
        for i in 0..n {
            let t = i as f32 / rate as f32;
            let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.2;
            tone[i * 2] = s;
            tone[i * 2 + 1] = s;
        }
        let mut before = tone.clone();
        stage.eq.process(&mut before);
        let before_rms = rms(&before[..256]);
        eq.load_preset("loud").unwrap();
        let mut after = tone;
        stage.eq.process(&mut after);
        let early = rms(&after[..256]);
        let late = rms(&after[after.len() - 2048..]);
        let step = before_rms * 10f32.powf(12.0 / 20.0);
        assert!(early < before_rms + 0.5 * (step - before_rms), "preset load stepped: early {early} step {step}");
        assert!(late > early * 1.5, "preset load did not slew: early {early} late {late}");
    }

    #[test]
    fn auto_loads_the_filename_preset_when_both_toggles_are_on() {
        let _guard = audio_lock();
        let mut eq = EqWindow::new();
        eq.note_track("song.mp3");
        eq.drag_slider(true, 0, -3_000);
        eq.save_autoload().unwrap();
        eq.drag_slider(true, 0, 0);
        eq.save_default().unwrap();
        eq.toggle_on();
        eq.toggle_auto();
        assert_eq!(llamp_audio::preamp_target_db(), -3.0);
        eq.note_track("other.mp3");
        assert_eq!(llamp_audio::preamp_target_db(), 0.0);
        eq.note_track("song.mp3");
        assert_eq!(llamp_audio::preamp_target_db(), -3.0);
    }

    #[test]
    fn window_slider_band_sweep_is_audible() {
        let _guard = audio_lock();
        let mut eq = EqWindow::new();
        eq.toggle_on();
        assert!(eq.on());
        let rate = 48_000u32;
        let pcm = sweep_pcm(rate);
        let ratios = llamp_audio::band_sweep_ratios(&pcm, rate, |band, db| {
            eq.drag_slider(false, band, (db * 1000.0).round() as i32);
        });
        for (band, ratio) in ratios.iter().enumerate() {
            assert!(
                *ratio > 2.0,
                "window slider did not make band {band} audible: ratio {ratio}"
            );
        }
    }

    fn sweep_pcm(rate: u32) -> Vec<f32> {
        let per = 8192usize;
        let mut pcm = Vec::with_capacity(per * 10 * 2);
        for hz in llamp_audio::eq::BAND_HZ {
            for i in 0..per {
                let t = i as f32 / rate as f32;
                let s = (2.0 * std::f32::consts::PI * hz * t).sin() * 0.15;
                pcm.push(s);
                pcm.push(s);
            }
        }
        pcm
    }

    fn rms(samples: &[f32]) -> f32 {
        let sum: f32 = samples.iter().map(|s| s * s).sum();
        (sum / samples.len() as f32).sqrt()
    }
}
