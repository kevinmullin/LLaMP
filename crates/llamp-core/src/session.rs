//! Lock-free playback snapshot. The publisher copies a POD value between two
//! atomic sequence stores. The poller loads that sequence and copies. It does
//! not wait on the audio callback, and the callback does not write this value.
//! The callback only increments [`llamp_audio::output::OutputEvents::played_frames`].

use std::cell::UnsafeCell;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use llamp_audio::cli::LoopPlayback;
use llamp_library::Library;

use llamp_audio::output::OutputEvents;

/// Left and right seek by this many seconds of the file's sample rate.
/// This is not a skin coordinate and not a value imported from another player.
pub const ARROW_SEEK_SECONDS: u64 = 1;
pub const TITLE_CAP: usize = 256;

const TRANSPORT_STOPPED: u8 = 0;
const TRANSPORT_PLAYING: u8 = 1;
const TRANSPORT_PAUSED: u8 = 2;

#[derive(Clone, Copy)]
pub struct PlaybackSnapshot {
    pub position_frames: u64,
    pub duration_frames: u64,
    pub sample_rate: u32,
    pub source_channels: u16,
    pub transport: u8,
    pub shuffle: u8,
    pub repeat_mode: u8,
    pub time_remaining: u8,
    pub vis_mode: u8,
    pub always_on_top: u8,
    pub double_size: u8,
    pub volume_ppm: u16,
    pub balance_ppm: u16,
    pub title_len: u16,
    pub title: [u8; TITLE_CAP],
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            position_frames: 0,
            duration_frames: 0,
            sample_rate: 0,
            source_channels: 0,
            transport: TRANSPORT_STOPPED,
            shuffle: 0,
            repeat_mode: 0,
            time_remaining: 0,
            vis_mode: 0,
            always_on_top: 0,
            double_size: 0,
            volume_ppm: 0,
            balance_ppm: 500,
            title_len: 0,
            title: [0; TITLE_CAP],
        }
    }
}

pub struct Session {
    events: Arc<OutputEvents>,
    base: AtomicU64,
    mark: AtomicU64,
    playing: AtomicU8,
    snap: Seqlock<PlaybackSnapshot>,
    /// Open library. Count is `SELECT COUNT(*) FROM tracks WHERE storage = 'referenced'`.
    library: Mutex<Option<Library>>,
    /// Feeder and stream. The callback does not take this lock.
    playback: Mutex<Option<LoopPlayback>>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            events: OutputEvents::new(),
            base: AtomicU64::new(0),
            mark: AtomicU64::new(0),
            playing: AtomicU8::new(TRANSPORT_STOPPED),
            snap: Seqlock::new(PlaybackSnapshot::default()),
            library: Mutex::new(None),
            playback: Mutex::new(None),
        }
    }

    pub fn events(&self) -> Arc<OutputEvents> {
        Arc::clone(&self.events)
    }

    /// Copies the snapshot and the frame counter. Does not wait. Takes no lock.
    pub fn poll(&self) -> PlaybackSnapshot {
        let mut snap = self.snap.read();
        snap.position_frames = self.position(snap.duration_frames);
        snap.transport = self.playing.load(Ordering::Relaxed);
        snap
    }

    pub fn configure(&self, sample_rate: u32, frames: u64, channels: u16, title: &[u8]) {
        self.base.store(0, Ordering::Relaxed);
        self.mark.store(self.events.played_frames.load(Ordering::Relaxed), Ordering::Relaxed);
        self.playing.store(TRANSPORT_STOPPED, Ordering::Relaxed);
        self.snap.write(|snap| {
            snap.duration_frames = frames;
            snap.sample_rate = sample_rate;
            snap.source_channels = channels;
            snap.transport = TRANSPORT_STOPPED;
            snap.position_frames = 0;
            copy_title(snap, title);
        });
    }

    /// Inserts each file in `dir` as `tracks.storage = 'referenced'`. Does not copy.
    /// The database stays open at `dir/../library.sqlite` so RSS includes those rows.
    pub fn retain_references(&self, dir: &Path) -> Result<usize, String> {
        let parent = dir.parent().ok_or("referenced directory has no parent")?;
        let library = Library::open(&parent.join("library.sqlite"))?;
        library.insert_referenced_dir(dir)?;
        let count = library.count_referenced()?;
        *self.library.lock().map_err(|err| err.to_string())? = Some(library);
        Ok(count)
    }

    pub fn reference_count(&self) -> usize {
        self.library
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().and_then(|lib| lib.count_referenced().ok()))
            .unwrap_or(0)
    }

    /// Opens the device and loops `path`. Sets transport to playing.
    /// The callback only `fetch_add`s `played_frames`.
    pub fn start_loop(&self, path: &Path) -> Result<(), String> {
        self.load_path(path)?;
        let handle = llamp_audio::cli::play_loop(path, self.events())?;
        self.mark.store(self.events.played_frames.load(Ordering::Relaxed), Ordering::Relaxed);
        self.playing.store(TRANSPORT_PLAYING, Ordering::Relaxed);
        self.snap.write(|snap| {
            snap.transport = TRANSPORT_PLAYING;
            snap.vis_mode = 0;
        });
        *self.playback.lock().map_err(|err| err.to_string())? = Some(handle);
        Ok(())
    }

    pub fn load_path(&self, path: &Path) -> Result<(), String> {
        let decoded = llamp_audio::decode::decode_path(path).map_err(|err| err.to_string())?;
        let frames = (decoded.frames.len() / 2) as u64;
        let title = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.configure(decoded.sample_rate, frames, decoded.source_channels, title.as_bytes());
        Ok(())
    }

    pub fn toggle_play(&self) {
        let next = match self.playing.load(Ordering::Relaxed) {
            TRANSPORT_PLAYING => TRANSPORT_PAUSED,
            _ => TRANSPORT_PLAYING,
        };
        self.playing.store(next, Ordering::Relaxed);
        self.snap.write(|snap| snap.transport = next);
    }

    pub fn stop(&self) {
        self.playing.store(TRANSPORT_STOPPED, Ordering::Relaxed);
        self.base.store(0, Ordering::Relaxed);
        self.mark.store(self.events.played_frames.load(Ordering::Relaxed), Ordering::Relaxed);
        self.snap.write(|snap| snap.transport = TRANSPORT_STOPPED);
    }

    /// `delta` is frames, not skin pixels. Arrow keys pass `±sample_rate * ARROW_SEEK_SECONDS`.
    pub fn seek_by(&self, delta: i32) {
        let duration = self.snap.read().duration_frames;
        let now = self.position(duration) as i64;
        let next = (now + i64::from(delta)).clamp(0, duration as i64) as u64;
        self.mark.store(self.events.played_frames.load(Ordering::Relaxed), Ordering::Relaxed);
        self.base.store(next, Ordering::Relaxed);
    }

    /// Same atomic increment the audio callback performs. Does not lock.
    pub fn note_played(&self, frames: u64) {
        self.events.played_frames.fetch_add(frames, Ordering::Relaxed);
    }

    pub fn press(&self, id: u32) {
        match id {
            10 | 11 => self.toggle_play(),
            12 => self.stop(),
            5 => self.snap.write(|snap| snap.always_on_top ^= 1),
            7 => self.snap.write(|snap| snap.double_size ^= 1),
            8 | 26 => self.snap.write(|snap| snap.vis_mode ^= 1),
            22 => self.snap.write(|snap| snap.shuffle ^= 1),
            23 => self.snap.write(|snap| snap.repeat_mode ^= 1),
            24 => self.snap.write(|snap| snap.time_remaining ^= 1),
            _ => {}
        }
    }

    /// Seek jumps by `duration * ppm / 1000`. Volume and balance are visual only.
    pub fn set_slider(&self, id: u32, ppm: u16) {
        let ppm = ppm.min(1000);
        match id {
            15 => {
                let duration = self.snap.read().duration_frames;
                let next = (u64::from(ppm) * duration) / 1000;
                self.mark
                    .store(self.events.played_frames.load(Ordering::Relaxed), Ordering::Relaxed);
                self.base.store(next, Ordering::Relaxed);
            }
            16 => self.snap.write(|snap| snap.volume_ppm = ppm),
            17 => self.snap.write(|snap| snap.balance_ppm = ppm),
            _ => {}
        }
    }

    fn position(&self, duration: u64) -> u64 {
        let played = self.events.played_frames.load(Ordering::Relaxed);
        let mark = self.mark.load(Ordering::Relaxed);
        let delta = played.saturating_sub(mark);
        self.base.load(Ordering::Relaxed).saturating_add(delta).min(duration)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

fn copy_title(snap: &mut PlaybackSnapshot, title: &[u8]) {
    let n = title.len().min(TITLE_CAP);
    snap.title = [0; TITLE_CAP];
    snap.title[..n].copy_from_slice(&title[..n]);
    snap.title_len = n as u16;
}

struct Seqlock<T> {
    seq: AtomicU64,
    value: UnsafeCell<T>,
}

impl<T: Copy> Seqlock<T> {
    fn new(value: T) -> Self {
        Self {
            seq: AtomicU64::new(0),
            value: UnsafeCell::new(value),
        }
    }

    fn write(&self, update: impl FnOnce(&mut T)) {
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(1), Ordering::Release);
        unsafe { update(&mut *self.value.get()) };
        self.seq.store(seq.wrapping_add(2), Ordering::Release);
    }

    /// Loads a stable copy. A torn sequence retries a few times and then returns
    /// the last copy. It does not wait on the callback, which does not write here.
    fn read(&self) -> T {
        let mut last = unsafe { *self.value.get() };
        for _ in 0..8 {
            let start = self.seq.load(Ordering::Acquire);
            if start & 1 == 1 {
                continue;
            }
            let value = unsafe { *self.value.get() };
            let end = self.seq.load(Ordering::Acquire);
            if start == end {
                return value;
            }
            last = value;
        }
        last
    }
}

unsafe impl<T: Copy + Send> Sync for Seqlock<T> {}
