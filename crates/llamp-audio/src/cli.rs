//! `llamp decode` and `llamp play`. No window.

use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};
use rtrb::RingBuffer;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use crate::cpal_output::CpalOutput;
use crate::decode;
use crate::live::{self, PCM_WINDOW};
use crate::output::{self, Output, OutputEvents, Playback, StreamRequest};
use crate::wav::{self, WavBits};

/// Keeps a looping feeder and the stream alive. Drop stops both.
/// The callback only copies the ring. This thread publishes the PCM window.
pub struct LoopPlayback {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    stream: Option<Box<dyn Playback>>,
}

impl Drop for LoopPlayback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        if let Some(mut stream) = self.stream.take() {
            stream.stop();
        }
    }
}

/// Decode, resample once, then loop into the ring. Does not run on the callback.
pub fn play_loop(input: &Path, events: Arc<OutputEvents>) -> Result<LoopPlayback, String> {
    let decoded = decode::decode_path(input).map_err(|err| err.to_string())?;
    let host_rate = open_rate()?;
    let stereo = resample_to_device(&decoded.frames, decoded.sample_rate, host_rate)?;
    if stereo.len() < 2 {
        return Err("track has no frames".into());
    }
    let capacity = output::ring_capacity(host_rate, 2);
    let (producer, consumer) = RingBuffer::new(capacity);
    let mut backend = CpalOutput::new();
    let request = StreamRequest { device_id: None, sample_rate: host_rate, channels: 2 };
    let stream = backend.open(request, consumer, events).map_err(|err| err.to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let thread = thread::Builder::new()
        .name("llamp-feeder".into())
        .spawn(move || feed_loop(producer, stereo, stop_thread))
        .map_err(|err| err.to_string())?;
    Ok(LoopPlayback { stop, thread: Some(thread), stream: Some(stream) })
}

fn feed_loop(mut producer: rtrb::Producer<f32>, stereo: Vec<f32>, stop: Arc<AtomicBool>) {
    let mut offset = 0usize;
    let mut window = [0f32; PCM_WINDOW];
    let mut cursor = 0usize;
    let mut since_publish = 0usize;
    while !stop.load(Ordering::Relaxed) {
        if producer.slots() < 2 {
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        let sample = stereo[offset];
        if producer.push(sample).is_err() {
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        offset += 1;
        if offset >= stereo.len() {
            offset = 0;
        }
        if offset % 2 == 1 {
            window[cursor % PCM_WINDOW] = sample;
            cursor += 1;
            since_publish += 1;
            if since_publish >= live::PCM_WINDOW / 2 {
                live::publish_pcm(&ordered(&window, cursor));
                since_publish = 0;
            }
        }
    }
}

fn ordered(window: &[f32; PCM_WINDOW], cursor: usize) -> [f32; PCM_WINDOW] {
    let mut out = [0f32; PCM_WINDOW];
    if cursor < PCM_WINDOW {
        out[PCM_WINDOW - cursor..].copy_from_slice(&window[..cursor]);
        return out;
    }
    let start = cursor % PCM_WINDOW;
    out[..PCM_WINDOW - start].copy_from_slice(&window[start..]);
    out[PCM_WINDOW - start..].copy_from_slice(&window[..start]);
    out
}

pub fn run() -> ExitCode {
    match dispatch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("decode") => {
            let input = args.next().ok_or("usage: llamp decode <path> <out.wav> [--pcm16]")?;
            let output = args.next().ok_or("usage: llamp decode <path> <out.wav> [--pcm16]")?;
            let pcm16 = args.any(|arg| arg == "--pcm16");
            decode_to_wav(Path::new(&input), Path::new(&output), pcm16)
        }
        Some("play") => {
            let input = args.next().ok_or("usage: llamp play <path>")?;
            play(Path::new(&input))
        }
        Some(other) => Err(format!("unknown command {other}")),
        None => Err("usage: llamp decode <path> <out.wav> [--pcm16] | llamp play <path>".into()),
    }
}

pub fn decode_to_wav(input: &Path, output: &Path, pcm16: bool) -> Result<(), String> {
    let decoded = decode::decode_path(input).map_err(|err| err.to_string())?;
    let bits = if pcm16 { WavBits::Pcm16 } else { WavBits::F32 };
    wav::write_wav(output, decoded.sample_rate, &decoded.frames, bits).map_err(|err| err.to_string())
}

pub fn play(input: &Path) -> Result<(), String> {
    let decoded = decode::decode_path(input).map_err(|err| err.to_string())?;
    let mut backend = CpalOutput::new();
    let events = OutputEvents::new();
    let host_rate = open_rate()?;
    let stereo = resample_to_device(&decoded.frames, decoded.sample_rate, host_rate)?;
    let capacity = output::ring_capacity(host_rate, 2);
    let (mut producer, consumer) = RingBuffer::new(capacity);
    let request = StreamRequest { device_id: None, sample_rate: host_rate, channels: 2 };
    let mut stream = backend.open(request, consumer, events.clone()).map_err(|err| err.to_string())?;
    let mut offset = 0;
    while offset < stereo.len() {
        if events.take_loss().is_some() {
            stream.stop();
            let new_rate = open_rate()?;
            if new_rate != host_rate {
                events.underruns.fetch_add(1, Ordering::Relaxed);
            }
            let (prod, cons) = RingBuffer::new(output::ring_capacity(new_rate, 2));
            producer = prod;
            stream = backend
                .open(
                    StreamRequest { device_id: None, sample_rate: new_rate, channels: 2 },
                    cons,
                    events.clone(),
                )
                .map_err(|err| err.to_string())?;
        }
        let space = producer.slots();
        let n = space.min(stereo.len() - offset);
        if n == 0 {
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        for sample in &stereo[offset..offset + n] {
            if producer.push(*sample).is_err() {
                break;
            }
        }
        offset += n;
    }
    thread::sleep(Duration::from_millis(200));
    stream.stop();
    Ok(())
}

fn open_rate() -> Result<u32, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no default output")?;
    let config = device.default_output_config().map_err(|err| err.to_string())?;
    Ok(config.sample_rate())
}

fn resample_to_device(interleaved: &[f32], from: u32, to: u32) -> Result<Vec<f32>, String> {
    if from == to || from == 0 || to == 0 {
        return Ok(interleaved.to_vec());
    }
    let frames = interleaved.len() / 2;
    let mut resampler = Fft::<f32>::new(from as usize, to as usize, 1024, 2, FixedSync::Input)
        .map_err(|err| err.to_string())?;
    let in_chunk = resampler.input_frames_next();
    let out_max = resampler.output_frames_max();
    let mut input = vec![0f32; in_chunk * 2];
    let mut output = vec![0f32; out_max * 2];
    let mut result = Vec::new();
    let mut offset = 0;
    while offset < frames {
        let n = in_chunk.min(frames - offset);
        input[..n * 2].copy_from_slice(&interleaved[offset * 2..offset * 2 + n * 2]);
        let input_adapter = InterleavedSlice::new(interleaved, 2, frames).map_err(|err| err.to_string())?;
        let mut output_adapter = InterleavedSlice::new_mut(&mut output, 2, out_max).map_err(|err| err.to_string())?;
        let mut indexing = Indexing::new().input_offset(offset);
        if n < in_chunk {
            indexing = indexing.partial_len(n);
        }
        let (_read, written) = resampler
            .process_into_buffer(&input_adapter, &mut output_adapter, Some(&indexing))
            .map_err(|err| err.to_string())?;
        result.extend_from_slice(&output[..written * 2]);
        offset += n;
        if n < in_chunk {
            break;
        }
    }
    let _ = input;
    Ok(result)
}
