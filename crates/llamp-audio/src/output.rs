//! Output seam from ADR 006. The session handles underrun and device-loss as values.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use rtrb::Consumer;

pub const PERIOD_MIN: u32 = 128;
pub const PERIOD_MAX: u32 = 4096;

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Clone, Debug)]
pub struct StreamRequest {
    pub device_id: Option<String>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputEvent {
    Underrun,
    DeviceLost,
    DeviceChanged,
}

pub struct OutputEvents {
    pub underruns: AtomicU64,
    pub device_lost: AtomicBool,
    pub device_changed: AtomicBool,
    pub period_frames: AtomicU32,
    pub sample_rate: AtomicU32,
    pub period_outside: AtomicBool,
}

impl OutputEvents {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            underruns: AtomicU64::new(0),
            device_lost: AtomicBool::new(false),
            device_changed: AtomicBool::new(false),
            period_frames: AtomicU32::new(0),
            sample_rate: AtomicU32::new(0),
            period_outside: AtomicBool::new(false),
        })
    }

    pub fn record_period(&self, frames: u32) {
        self.period_frames.store(frames, Ordering::Relaxed);
        self.period_outside.store(frames < PERIOD_MIN || frames > PERIOD_MAX, Ordering::Relaxed);
    }

    pub fn take_loss(&self) -> Option<OutputEvent> {
        if self.device_changed.swap(false, Ordering::AcqRel) {
            Some(OutputEvent::DeviceChanged)
        } else if self.device_lost.swap(false, Ordering::AcqRel) {
            Some(OutputEvent::DeviceLost)
        } else {
            None
        }
    }
}

impl Default for OutputEvents {
    fn default() -> Self {
        Self {
            underruns: AtomicU64::new(0),
            device_lost: AtomicBool::new(false),
            device_changed: AtomicBool::new(false),
            period_frames: AtomicU32::new(0),
            sample_rate: AtomicU32::new(0),
            period_outside: AtomicBool::new(false),
        }
    }
}

#[derive(Debug)]
pub struct OutputError(pub String);

impl std::fmt::Display for OutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait Playback: Send {
    fn sample_rate(&self) -> u32;
    fn stop(&mut self);
}

pub trait Output: Send {
    fn enumerate(&self) -> Result<Vec<DeviceInfo>, OutputError>;
    fn open(
        &mut self,
        request: StreamRequest,
        ring: Consumer<f32>,
        events: Arc<OutputEvents>,
    ) -> Result<Box<dyn Playback>, OutputError>;
}

pub fn ring_capacity(sample_rate: u32, channels: u16) -> usize {
    sample_rate as usize * channels as usize
}
