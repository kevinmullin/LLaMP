//! macOS backend: cpal 0.18 shared float, no hog mode.
//! Device open happens on the caller, never inside the callback.
//! The error callback only stores atomics.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use rtrb::Consumer;

use crate::output::{DeviceInfo, Output, OutputError, OutputEvents, Playback, StreamRequest};

pub struct CpalOutput {
    host: cpal::Host,
}

impl CpalOutput {
    pub fn new() -> Self {
        Self { host: cpal::default_host() }
    }
}

impl Default for CpalOutput {
    fn default() -> Self {
        Self::new()
    }
}

struct OpenStream {
    stream: cpal::Stream,
    rate: u32,
}

impl Playback for OpenStream {
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn stop(&mut self) {
        let _ = self.stream.pause();
    }
}

impl Output for CpalOutput {
    fn enumerate(&self) -> Result<Vec<DeviceInfo>, OutputError> {
        let default = self.host.default_output_device().and_then(|d| d.description().ok()).map(|d| d.name().to_string());
        let devices = self.host.output_devices().map_err(|err| OutputError(err.to_string()))?;
        let mut out = Vec::new();
        for device in devices {
            let name = device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| "unknown".into());
            out.push(DeviceInfo {
                id: name.clone(),
                name: name.clone(),
                is_default: default.as_deref() == Some(name.as_str()),
            });
        }
        Ok(out)
    }

    fn open(
        &mut self,
        request: StreamRequest,
        mut ring: Consumer<f32>,
        events: Arc<OutputEvents>,
    ) -> Result<Box<dyn Playback>, OutputError> {
        let device = match request.device_id.as_deref() {
            Some(id) => self
                .host
                .output_devices()
                .map_err(|err| OutputError(err.to_string()))?
                .find(|d| d.description().ok().map(|desc| desc.name().to_string()).as_deref() == Some(id))
                .ok_or_else(|| OutputError(format!("no device {id}")))?,
            None => self.host.default_output_device().ok_or_else(|| OutputError("no default output".into()))?,
        };
        let supported = device.default_output_config().map_err(|err| OutputError(err.to_string()))?;
        let format = supported.sample_format();
        let mut config: StreamConfig = supported.into();
        if request.sample_rate > 0 {
            config.sample_rate = request.sample_rate;
        }
        config.channels = request.channels.max(1);
        let rate = config.sample_rate;
        events.sample_rate.store(rate, Ordering::Relaxed);
        let events_cb = Arc::clone(&events);
        let err_events = Arc::clone(&events);
        let stream = match format {
            SampleFormat::F32 => device
                .build_output_stream(
                    config,
                    move |data: &mut [f32], _| fill_float(data, &mut ring, &events_cb),
                    move |err| note_error(&err_events, err.kind()),
                    None,
                )
                .map_err(|err| OutputError(err.to_string()))?,
            SampleFormat::I16 => {
                let mut rng = 0x1234_5678u32;
                device
                    .build_output_stream(
                        config,
                        move |data: &mut [i16], _| fill_i16(data, &mut ring, &events_cb, &mut rng),
                        move |err| note_error(&err_events, err.kind()),
                        None,
                    )
                    .map_err(|err| OutputError(err.to_string()))?
            }
            other => return Err(OutputError(format!("unsupported sample format {other:?}"))),
        };
        stream.play().map_err(|err| OutputError(err.to_string()))?;
        Ok(Box::new(OpenStream { stream, rate }))
    }
}

fn fill_float(data: &mut [f32], ring: &mut Consumer<f32>, events: &OutputEvents) {
    let channels = if data.is_empty() { 1 } else { 1.max(data.len() / data.len()) };
    let _ = channels;
    let mut missed = false;
    for sample in data.iter_mut() {
        match ring.pop() {
            Ok(value) => *sample = value,
            Err(_) => {
                *sample = 0.0;
                missed = true;
            }
        }
    }
    if missed {
        events.underruns.fetch_add(1, Ordering::Relaxed);
    }
    let frames = data.len() as u32;
    if frames > 0 {
        events.record_period(frames);
    }
}

fn fill_i16(data: &mut [i16], ring: &mut Consumer<f32>, events: &OutputEvents, rng: &mut u32) {
    let mut missed = false;
    for sample in data.iter_mut() {
        let value = match ring.pop() {
            Ok(value) => value,
            Err(_) => {
                missed = true;
                0.0
            }
        };
        *sample = tpdf_i16(value, rng);
    }
    if missed {
        events.underruns.fetch_add(1, Ordering::Relaxed);
    }
    events.record_period(data.len() as u32);
}

fn tpdf_i16(sample: f32, rng: &mut u32) -> i16 {
    let u1 = lcg(rng);
    let u2 = lcg(rng);
    let dither = (u1 - u2) / 2.0;
    let scaled = sample * i16::MAX as f32 + dither;
    scaled.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

fn lcg(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    (*state >> 8) as f32 / 16_777_216.0
}

fn note_error(events: &OutputEvents, kind: cpal::ErrorKind) {
    match kind {
        cpal::ErrorKind::DeviceChanged => events.device_changed.store(true, Ordering::Release),
        cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated => {
            events.device_lost.store(true, Ordering::Release);
        }
        cpal::ErrorKind::Xrun => {
            events.underruns.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }
}
