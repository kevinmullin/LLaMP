//! Callback graph: ReplayGain, preamp, EQ, first-party DSP slot (no-op), limiter.
//! The ring is allocated before the stream starts. The callback does not allocate.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use rtrb::RingBuffer;

use crate::decode::{self, CodecId, DecodeError};
use crate::eq::Eq;
use crate::limiter::Limiter;
use crate::output::{self, PERIOD_MAX, PERIOD_MIN};

pub const DEVICE_PERIOD_MIN: u32 = PERIOD_MIN;
pub const DEVICE_PERIOD_MAX: u32 = PERIOD_MAX;

pub struct Trace {
    pub pre_limiter_peak: f32,
}

/// First-party DSP callback. Native only. The default is a no-op.
pub type DspFn = fn(&mut [f32]);

pub fn dsp_noop(_frames: &mut [f32]) {}

pub struct Stage {
    pub eq: Eq,
    pub limiter: Limiter,
    replaygain: f32,
    dsp: DspFn,
}

impl Stage {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            eq: Eq::new(sample_rate),
            limiter: Limiter::new(sample_rate),
            replaygain: 1.0,
            dsp: dsp_noop,
        }
    }

    /// The live callback and `llamp play` use this. Sliders write its targets.
    pub fn for_playback(sample_rate: u32) -> Self {
        let mut stage = Self::new(sample_rate);
        stage.eq.bind_ui_sliders();
        stage
    }

    pub fn set_replaygain_db(&mut self, db: f32) {
        self.replaygain = 10f32.powf(db / 20.0);
    }

    pub fn process(&mut self, frames: &mut [f32]) -> Trace {
        if (self.replaygain - 1.0).abs() > 1e-8 {
            for sample in frames.iter_mut() {
                *sample *= self.replaygain;
            }
        }
        self.eq.process(frames);
        (self.dsp)(frames);
        let mut peak = 0.0f32;
        for sample in frames.iter() {
            peak = peak.max(sample.abs());
        }
        self.limiter.process(frames);
        Trace {
            pre_limiter_peak: peak,
        }
    }
}

pub struct Callback {
    consumer: rtrb::Consumer<f32>,
    producer: rtrb::Producer<f32>,
    underruns: AtomicU64,
    /// Frames copied from the ring. The callback only `fetch_add`s this.
    played: AtomicU64,
    stage: Stage,
}

impl Callback {
    pub fn new(period_frames: u32, sample_rate: u32) -> Self {
        let capacity = output::ring_capacity(sample_rate, 2).max(period_frames as usize * 2);
        let (producer, consumer) = RingBuffer::new(capacity);
        Self {
            consumer,
            producer,
            underruns: AtomicU64::new(0),
            played: AtomicU64::new(0),
            stage: Stage::new(sample_rate),
        }
    }

    pub fn underruns(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    /// Loads the frame count the callback published. Does not wait.
    pub fn played_frames(&self) -> u64 {
        self.played.load(Ordering::Relaxed)
    }

    /// Pop one period from the ring, or silence and one underrun. Then the graph, in place.
    pub fn fill(&mut self, out: &mut [f32]) {
        let mut filled = 0;
        while filled < out.len() {
            match self.consumer.pop() {
                Ok(sample) => {
                    out[filled] = sample;
                    filled += 1;
                }
                Err(_) => {
                    out[filled..].fill(0.0);
                    self.underruns.fetch_add(1, Ordering::Relaxed);
                    break;
                }
            }
        }
        if filled > 0 {
            self.played
                .fetch_add((filled / 2) as u64, Ordering::Relaxed);
        }
        let _ = self.stage.process(out);
    }

    /// The window slider path. Does not copy coefficients onto this callback.
    pub fn bind_ui_sliders(&mut self) {
        self.stage.eq.bind_ui_sliders();
    }

    /// Swap the first-party DSP slot at a buffer boundary. Not called from the callback.
    pub fn set_dsp(&mut self, dsp: DspFn) {
        self.stage.dsp = dsp;
    }

    pub fn push_pcm(&mut self, interleaved: &[f32]) -> usize {
        let mut n = 0;
        for sample in interleaved {
            if self.producer.push(*sample).is_err() {
                break;
            }
            n += 1;
        }
        n
    }
}

pub struct GaplessInfo {
    pub delay: Option<u32>,
    pub padding: Option<u32>,
    pub frames: u64,
    pub aac_not_gapless: bool,
}

pub fn gapless_info(path: &Path) -> Result<GaplessInfo, DecodeError> {
    let decoded = decode::decode_path(path)?;
    Ok(GaplessInfo {
        delay: decoded.delay,
        padding: decoded.padding,
        frames: (decoded.frames.len() / 2) as u64,
        aac_not_gapless: decoded.codec == CodecId::AacLc,
    })
}

/// Concatenate the gapless PCM of two tracks. The caller compares the length
/// and the sample at the boundary to the encoder's playable frame count, not
/// to [`gapless_info`], which is this same decode.
pub fn join_gapless(a: &Path, b: &Path) -> Result<Vec<f32>, DecodeError> {
    let left = decode::decode_path(a)?;
    let right = decode::decode_path(b)?;
    let mut joined = left.frames;
    joined.extend_from_slice(&right.frames);
    Ok(joined)
}

pub fn seek_landing(
    path: &Path,
    frame: u64,
    period: u32,
) -> Result<decode::SeekLanding, DecodeError> {
    decode::seek_to(path, frame, period)
}

/// Decode-and-graph path `llamp play` uses, without opening a device.
/// `set_band` is the CLI drag or a window slider. Ten slices, one band each.
pub fn band_sweep_ratios(
    pcm: &[f32],
    rate: u32,
    mut set_band: impl FnMut(usize, f32),
) -> [f32; 10] {
    crate::set_eq_enabled(true);
    let frames = pcm.len() / 2;
    let per = frames / 10;
    let mut stage = Stage::for_playback(rate);
    let settle = ((rate as usize / 10).max(256)) * 2;
    std::array::from_fn(|band| {
        for other in 0..10 {
            set_band(other, 0.0);
        }
        let mut silence = vec![0f32; settle];
        stage.process(&mut silence);
        let start = band * per * 2;
        let end = start + per * 2;
        let mut flat = pcm[start..end].to_vec();
        stage.process(&mut flat);
        set_band(band, 12.0);
        let mut silence = vec![0f32; settle];
        stage.process(&mut silence);
        let mut boost = pcm[start..end].to_vec();
        stage.process(&mut boost);
        let flat_rms = rms_slice(&flat);
        if flat_rms < 1e-8 {
            0.0
        } else {
            rms_slice(&boost) / flat_rms
        }
    })
}

fn rms_slice(samples: &[f32]) -> f32 {
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}
