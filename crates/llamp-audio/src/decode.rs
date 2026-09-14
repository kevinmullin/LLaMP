//! Decode to interleaved stereo `f32`. Reads at most 64 KiB of compressed bytes per `read`.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::OnceLock;

use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::well_known::{
    CODEC_ID_AAC, CODEC_ID_ALAC, CODEC_ID_FLAC, CODEC_ID_MP3, CODEC_ID_OPUS, CODEC_ID_VORBIS,
};
use symphonia::core::codecs::audio::AudioCodecId;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::registry::CodecRegistry;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

use crate::gapless::{self, trim_enabled};
use crate::opus;
use crate::pcm;

pub const READ_LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecId {
    Mp3,
    AacLc,
    Flac,
    Alac,
    Wav,
    Aiff,
    Vorbis,
    Opus,
}

#[derive(Clone, Debug)]
pub struct Decoded {
    pub sample_rate: u32,
    pub frames: Vec<f32>,
    pub source_channels: u16,
    pub delay: Option<u32>,
    pub padding: Option<u32>,
    pub codec: CodecId,
}

#[derive(Debug)]
pub enum DecodeError {
    Io(io::Error),
    Audio(String),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Io(err) => write!(f, "{err}"),
            DecodeError::Audio(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<io::Error> for DecodeError {
    fn from(value: io::Error) -> Self {
        DecodeError::Io(value)
    }
}

impl From<SymError> for DecodeError {
    fn from(value: SymError) -> Self {
        DecodeError::Audio(value.to_string())
    }
}

pub fn decode_path(path: &Path) -> Result<Decoded, DecodeError> {
    let file = File::open(path)?;
    let codec_hint = codec_from_extension(path);
    decode_file(file, codec_hint, path)
}

fn decode_file(file: File, hint_codec: Option<CodecId>, path: &Path) -> Result<Decoded, DecodeError> {
    let limited = LimitedFile { file, len: None };
    let len = limited.file.metadata().ok().map(|m| m.len());
    let limited = LimitedFile { file: limited.file, len };
    let mss = MediaSourceStream::new(Box::new(limited), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|err| DecodeError::Audio(format!("probe: {err}")))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| DecodeError::Audio("no audio track".into()))?;
    let audio = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .cloned()
        .ok_or_else(|| DecodeError::Audio("missing audio parameters".into()))?;
    let delay = track.delay;
    let padding = track.padding;
    let track_id = track.id;
    let codec = classify(audio.codec, hint_codec);
    let gapless = trim_enabled(codec);
    let opts = AudioDecoderOptions::default().gapless(gapless);
    let mut decoder = codecs()
        .make_audio_decoder(&audio, &opts)
        .map_err(|err| DecodeError::Audio(format!("decoder: {err}")))?;

    let mut planar = Vec::new();
    let mut rate = audio.sample_rate.unwrap_or(0);
    let mut channels = audio.channels.map(|c| c.count()).unwrap_or(0);
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymError::IoError(err)) if err.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(err) => return Err(DecodeError::Audio(err.to_string())),
        };
        while !format.metadata().is_latest() {
            format.metadata().pop();
        }
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(buf) => buf,
            Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
            Err(err) => return Err(DecodeError::Audio(err.to_string())),
        };
        append_buffer(&decoded, &mut planar, &mut rate, &mut channels)?;
    }
    if rate == 0 || channels == 0 {
        return Err(DecodeError::Audio("decoder produced no audio".into()));
    }
    let stereo = pcm::fold_to_stereo(&planar, channels as u16);
    let frames = stereo.len() / 2;
    let (start, end) = gapless::trim_counts(codec, delay, padding, gapless);
    let start = start.min(frames as u64) as usize;
    let end = end.min(frames as u64) as usize;
    let end = end.min(frames.saturating_sub(start));
    let kept = &stereo[start * 2..stereo.len() - end * 2];
    Ok(Decoded {
        sample_rate: rate,
        frames: kept.to_vec(),
        source_channels: channels as u16,
        delay,
        padding,
        codec,
    })
}

fn append_buffer(
    decoded: &GenericAudioBufferRef<'_>,
    out: &mut Vec<f32>,
    rate: &mut u32,
    channels: &mut usize,
) -> Result<(), DecodeError> {
    let spec = decoded.spec();
    *rate = spec.rate();
    *channels = spec.channels().count();
    let frames = decoded.frames();
    if frames == 0 || *channels == 0 {
        return Ok(());
    }
    let mut interleaved = vec![0f32; frames * *channels];
    decoded.copy_to_slice_interleaved(&mut interleaved);
    out.extend_from_slice(&interleaved);
    Ok(())
}

fn classify(id: AudioCodecId, hint: Option<CodecId>) -> CodecId {
    if id == CODEC_ID_MP3 {
        CodecId::Mp3
    } else if id == CODEC_ID_AAC {
        CodecId::AacLc
    } else if id == CODEC_ID_FLAC {
        CodecId::Flac
    } else if id == CODEC_ID_ALAC {
        CodecId::Alac
    } else if id == CODEC_ID_VORBIS {
        CodecId::Vorbis
    } else if id == CODEC_ID_OPUS {
        CodecId::Opus
    } else {
        hint.unwrap_or(CodecId::Wav)
    }
}

fn codec_from_extension(path: &Path) -> Option<CodecId> {
    match path.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("wav") => Some(CodecId::Wav),
        Some("aif") | Some("aiff") => Some(CodecId::Aiff),
        Some("mp3") => Some(CodecId::Mp3),
        Some("flac") => Some(CodecId::Flac),
        Some("ogg") | Some("oga") => Some(CodecId::Vorbis),
        Some("opus") => Some(CodecId::Opus),
        Some("m4a") | Some("mp4") | Some("aac") => None,
        _ => None,
    }
}

fn codecs() -> &'static CodecRegistry {
    static REGISTRY: OnceLock<CodecRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut registry);
        opus::register(&mut registry);
        registry
    })
}

pub struct SeekLanding {
    pub frame: u64,
    pub first_sample: f32,
}

pub fn seek_to(path: &Path, frame: u64, period: u32) -> Result<SeekLanding, DecodeError> {
    let file = File::open(path)?;
    let limited = LimitedFile { file, len: file_len(path) };
    let mss = MediaSourceStream::new(Box::new(limited), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|err| DecodeError::Audio(format!("{err:?}")))?;
    let track = format.default_track(TrackType::Audio).ok_or_else(|| DecodeError::Audio("no audio track".into()))?;
    let audio = track.codec_params.as_ref().and_then(|p| p.audio()).cloned().ok_or_else(|| DecodeError::Audio("missing audio parameters".into()))?;
    let track_id = track.id;
    let opts = AudioDecoderOptions::default().gapless(false);
    let mut decoder = codecs().make_audio_decoder(&audio, &opts).map_err(|err| DecodeError::Audio(format!("{err:?}")))?;
    let seeked = format.seek(
        symphonia::core::formats::SeekMode::Accurate,
        symphonia::core::formats::SeekTo::Timestamp {
            ts: symphonia::core::units::Timestamp::new(frame as i64),
            track_id,
        },
    )?;
    decoder.reset();
    let mut landed = seeked.actual_ts.get().max(0) as u64;
    let mut first = None;
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(err) => return Err(err.into()),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(buf) => buf,
            Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
            Err(err) => return Err(err.into()),
        };
        let spec = decoded.spec();
        let ch = spec.channels().count().max(1);
        let frames = decoded.frames();
        if frames == 0 {
            continue;
        }
        let mut interleaved = vec![0f32; frames * ch];
        decoded.copy_to_slice_interleaved(&mut interleaved);
        let stereo = pcm::fold_to_stereo(&interleaved, ch as u16);
        let packet_start = packet.pts.get().max(0) as u64;
        for i in 0..stereo.len() / 2 {
            let at = packet_start + i as u64;
            if at + u64::from(period) < frame {
                continue;
            }
            if first.is_none() {
                landed = at;
                first = Some(stereo[i * 2]);
            }
        }
        if first.is_some() {
            break;
        }
    }
    let first_sample = first.ok_or_else(|| DecodeError::Audio("seek produced no samples".into()))?;
    let err = (landed as i64 - frame as i64).abs();
    if err > i64::from(period) {
        return Err(DecodeError::Audio(format!("seek landed {err} frames away, period is {period}")));
    }
    Ok(SeekLanding { frame: landed, first_sample })
}

fn file_len(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

struct LimitedFile {
    file: File,
    len: Option<u64>,
}

impl Read for LimitedFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(READ_LIMIT);
        self.file.read(&mut buf[..n])
    }
}

impl Seek for LimitedFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.file.seek(pos)
    }
}

impl MediaSource for LimitedFile {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}
