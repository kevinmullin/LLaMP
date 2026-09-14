//! Generates tones at test time. No binary fixtures are committed.
#![allow(dead_code)]

use std::fs::File;
use std::io::Write;
use std::num::{NonZeroU8, NonZeroU32};
use std::path::{Path, PathBuf};
use std::process::Command;

use flacenc::bitsink::MemSink;
use flacenc::component::BitRepr;
use flacenc::config::Encoder;
use flacenc::error::Verify;
use flacenc::source::MemSource;
use mp3lame_encoder::{Builder, DualPcm, FlushGap};
use ogg::{PacketWriteEndInfo, PacketWriter};
use vorbis_rs::VorbisEncoderBuilder;

#[derive(Clone)]
pub struct Encoded {
    pub path: PathBuf,
    pub playable_frames: u64,
    pub encoder_delay: Option<u32>,
    pub encoder_padding: Option<u32>,
}

pub fn dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join("llamp-audio-fixtures").join(name);
    std::fs::create_dir_all(&path).expect("fixture dir");
    path
}

pub fn write_wav_tone(dir: &Path, name: &str, rate: u32, frames: u64, hz: f32) -> PathBuf {
    let path = dir.join(name);
    write_pcm16_wav(&path, rate, &tone(rate, frames, hz));
    path
}

pub fn write_aiff_tone(dir: &Path, name: &str, rate: u32, frames: u64, hz: f32) -> PathBuf {
    let path = dir.join(name);
    write_pcm16_aiff(&path, rate, &tone(rate, frames, hz));
    path
}

pub fn write_flac_tone(dir: &Path, name: &str, rate: u32, frames: u64, hz: f32) -> Encoded {
    let path = dir.join(name);
    let pcm = tone(rate, frames, hz);
    let samples: Vec<i32> = pcm.iter().map(|s| (s * 32767.0).round() as i32).collect();
    let source = MemSource::from_samples(&samples, 2, 16, rate as usize);
    let config = Encoder::default().into_verified().expect("flac config");
    let mut stream = flacenc::encode_with_fixed_block_size(&config, source, 4096).expect("flac encode");
    // Last frame is shorter, so flacenc records min != max, but the frames are fixed-block.
    // Symphonia rejects that combination. The spec allows the last frame to be shorter than min.
    stream.stream_info_mut().set_block_sizes(4096, 4096).expect("flac block size");
    let mut sink = MemSink::new();
    stream.write(&mut sink).expect("flac write");
    std::fs::write(&path, sink.as_slice()).expect("flac file");
    // flacenc writes the source length into STREAMINFO and adds no priming.
    Encoded { path, playable_frames: frames, encoder_delay: Some(0), encoder_padding: Some(0) }
}

pub fn write_mp3_tone(dir: &Path, name: &str, rate: u32, frames: u64, hz: f32) -> Encoded {
    let path = dir.join(name);
    let pcm = tone(rate, frames, hz);
    let mut left = Vec::with_capacity(frames as usize);
    let mut right = Vec::with_capacity(frames as usize);
    for frame in pcm.chunks_exact(2) {
        left.push((frame[0] * 32767.0).round() as i16);
        right.push((frame[1] * 32767.0).round() as i16);
    }
    let mut encoder = Builder::new()
        .expect("lame")
        .with_num_channels(2)
        .expect("channels")
        .with_sample_rate(rate)
        .expect("rate")
        .with_brate(mp3lame_encoder::Bitrate::Kbps128)
        .expect("bitrate")
        .with_quality(mp3lame_encoder::Quality::Best)
        .expect("quality")
        .with_to_write_vbr_tag(true)
        .expect("vbr tag")
        .build()
        .expect("build lame");
    let mut mp3 = Vec::new();
    mp3.reserve(mp3lame_encoder::max_required_buffer_size(left.len()));
    encoder.encode_to_vec(DualPcm { left: &left, right: &right }, &mut mp3).expect("encode mp3");
    mp3.reserve(7200);
    encoder.flush_to_vec::<FlushGap>(&mut mp3).expect("flush mp3");
    let mut tag = Vec::new();
    tag.reserve(encoder.lame_tag_size());
    if let Some(written) = encoder.lame_tag_encode_to_vec(&mut tag) {
        let boundary = encoder.id3v2_tag_size();
        let end = boundary + written.get();
        if end <= mp3.len() {
            mp3[boundary..end].copy_from_slice(&tag[..written.get()]);
        }
    }
    std::fs::write(&path, &mp3).expect("mp3 file");
    let (delay, padding) = parse_lame_delay(&mp3);
    Encoded { path, playable_frames: frames, encoder_delay: delay, encoder_padding: padding }
}

pub fn write_vorbis_tone(dir: &Path, name: &str, rate: u32, frames: u64, hz: f32) -> Encoded {
    let path = dir.join(name);
    let pcm = tone(rate, frames, hz);
    let mut left = Vec::with_capacity(frames as usize);
    let mut right = Vec::with_capacity(frames as usize);
    for frame in pcm.chunks_exact(2) {
        left.push(frame[0]);
        right.push(frame[1]);
    }
    let file = File::create(&path).expect("ogg");
    let mut encoder = VorbisEncoderBuilder::new(
        NonZeroU32::new(rate).unwrap(),
        NonZeroU8::new(2).unwrap(),
        file,
    )
    .expect("vorbis builder")
    .build()
    .expect("vorbis encoder");
    encoder.encode_audio_block([&left, &right]).expect("vorbis encode");
    encoder.finish().expect("vorbis finish");
    let granule = last_ogg_granule(&path);
    let extra = granule.saturating_sub(frames);
    Encoded {
        path,
        playable_frames: frames,
        encoder_delay: Some(extra.min(u32::MAX as u64) as u32),
        encoder_padding: Some(0),
    }
}

pub fn write_opus_tone(dir: &Path, name: &str, rate: u32, frames: u64, hz: f32) -> Encoded {
    let path = dir.join(name);
    let pcm = tone(rate, frames, hz);
    let file = File::create(&path).expect("opus");
    let pre_skip = write_ogg_opus(file, rate, &pcm);
    Encoded {
        path,
        playable_frames: frames,
        encoder_delay: Some(u32::from(pre_skip)),
        encoder_padding: Some(0),
    }
}

pub fn write_ramp_wav(dir: &Path, name: &str, rate: u32, frames: u64) -> PathBuf {
    let path = dir.join(name);
    let mut pcm = Vec::with_capacity(frames as usize * 2);
    for frame in 0..frames {
        let s = ramp_sample(frame);
        pcm.push(s);
        pcm.push(s);
    }
    write_pcm16_wav(&path, rate, &pcm);
    path
}

pub fn ramp_sample(frame: u64) -> f32 {
    (frame as f32 + 1.0) / 100_000.0
}

/// ALAC via `afconvert`. Playable length and priming come from `afinfo`, not from our decoder.
pub fn write_alac_tone(dir: &Path, name: &str, wav: &Path) -> Encoded {
    let path = afconvert(wav, &dir.join(name), &["-f", "m4af", "-d", "alac"]);
    let (valid, priming, remainder) = afinfo_counts(&path);
    Encoded {
        path,
        playable_frames: valid,
        encoder_delay: Some(priming),
        encoder_padding: Some(remainder),
    }
}

pub fn afconvert(input: &Path, output: &Path, args: &[&str]) -> PathBuf {
    let status = Command::new("afconvert").arg(input).args(args).arg(output).status().expect("afconvert");
    assert!(status.success(), "afconvert {:?} failed", args);
    output.to_path_buf()
}

pub fn write_replaygain(path: &Path, track: &str, peak: &str, album: &str, album_peak: &str, album_id: &str) {
    use lofty::config::WriteOptions;
    use lofty::file::{AudioFile, TaggedFileExt};
    use lofty::tag::{ItemKey, Tag, TagType};

    let mut tagged = lofty::read_from_path(path).expect("read for tags");
    if tagged.primary_tag().is_none() {
        tagged.insert_tag(Tag::new(TagType::Id3v2));
    }
    {
        let tag = tagged.primary_tag_mut().expect("tag");
        tag.insert_text(ItemKey::ReplayGainTrackGain, track.to_string());
        tag.insert_text(ItemKey::ReplayGainTrackPeak, peak.to_string());
        tag.insert_text(ItemKey::ReplayGainAlbumGain, album.to_string());
        tag.insert_text(ItemKey::ReplayGainAlbumPeak, album_peak.to_string());
        tag.insert_text(ItemKey::MusicBrainzReleaseId, album_id.to_string());
    }
    tagged.save_to_path(path, WriteOptions::default()).expect("save tags");
}

pub fn source_sample(rate: u32, frame: u64, hz: f32) -> f32 {
    0.2 * (2.0 * std::f32::consts::PI * hz * frame as f32 / rate as f32).sin()
}

fn tone(rate: u32, frames: u64, hz: f32) -> Vec<f32> {
    (0..frames).flat_map(|i| {
        let s = source_sample(rate, i, hz);
        [s, s]
    }).collect()
}

fn last_ogg_granule(path: &Path) -> u64 {
    let bytes = std::fs::read(path).expect("ogg bytes");
    let mut last = None;
    for (i, window) in bytes.windows(4).enumerate() {
        if window == b"OggS" && i + 14 <= bytes.len() {
            let granule = u64::from_le_bytes(bytes[i + 6..i + 14].try_into().unwrap());
            if granule != u64::MAX {
                last = Some(granule);
            }
        }
    }
    last.expect("ogg granule")
}

fn afinfo_counts(path: &Path) -> (u64, u32, u32) {
    let output = Command::new("afinfo").arg(path).output().expect("afinfo");
    assert!(output.status.success(), "afinfo failed");
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("audio ") else {
            continue;
        };
        let mut nums = rest.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty());
        if let (Some(valid), Some(priming), Some(remainder)) = (nums.next(), nums.next(), nums.next()) {
            return (
                valid.parse().expect("valid frames"),
                priming.parse().expect("priming"),
                remainder.parse().expect("remainder"),
            );
        }
    }
    panic!("afinfo did not report valid/priming/remainder for {}", path.display());
}

fn write_pcm16_wav(path: &Path, rate: u32, interleaved: &[f32]) {
    let data_bytes = interleaved.len() * 2;
    let mut file = File::create(path).expect("wav");
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + data_bytes as u32).to_le_bytes()).unwrap();
    file.write_all(b"WAVEfmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&2u16.to_le_bytes()).unwrap();
    file.write_all(&rate.to_le_bytes()).unwrap();
    file.write_all(&(rate * 4).to_le_bytes()).unwrap();
    file.write_all(&4u16.to_le_bytes()).unwrap();
    file.write_all(&16u16.to_le_bytes()).unwrap();
    file.write_all(b"data").unwrap();
    file.write_all(&(data_bytes as u32).to_le_bytes()).unwrap();
    for sample in interleaved {
        let q = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        file.write_all(&q.to_le_bytes()).unwrap();
    }
}

fn write_pcm16_aiff(path: &Path, rate: u32, interleaved: &[f32]) {
    let frames = interleaved.len() / 2;
    let ssnd_data = interleaved.len() * 2;
    let ssnd_size = 8 + ssnd_data;
    let form_size = 4 + 8 + 18 + 8 + ssnd_size;
    let mut file = File::create(path).expect("aiff");
    file.write_all(b"FORM").unwrap();
    file.write_all(&(form_size as u32).to_be_bytes()).unwrap();
    file.write_all(b"AIFFCOMM").unwrap();
    file.write_all(&18u32.to_be_bytes()).unwrap();
    file.write_all(&2u16.to_be_bytes()).unwrap();
    file.write_all(&(frames as u32).to_be_bytes()).unwrap();
    file.write_all(&16u16.to_be_bytes()).unwrap();
    file.write_all(&extended_rate(rate)).unwrap();
    file.write_all(b"SSND").unwrap();
    file.write_all(&(ssnd_size as u32).to_be_bytes()).unwrap();
    file.write_all(&0u32.to_be_bytes()).unwrap();
    file.write_all(&0u32.to_be_bytes()).unwrap();
    for sample in interleaved {
        let q = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        file.write_all(&q.to_be_bytes()).unwrap();
    }
}

fn extended_rate(rate: u32) -> [u8; 10] {
    match rate {
        48_000 => [0x40, 0x0e, 0xbb, 0x80, 0, 0, 0, 0, 0, 0],
        44_100 => [0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0],
        other => panic!("no 80-bit rate for {other}"),
    }
}

fn parse_lame_delay(bytes: &[u8]) -> (Option<u32>, Option<u32>) {
    let Some(at) = bytes.windows(4).position(|w| w == b"LAME") else {
        return (None, None);
    };
    if at + 24 > bytes.len() {
        return (None, None);
    }
    let trim = u32::from_be_bytes([0, bytes[at + 21], bytes[at + 22], bytes[at + 23]]);
    Some((trim >> 12, trim & 0x0fff)).map_or((None, None), |(d, p)| (Some(d), Some(p)))
}

fn write_ogg_opus(mut file: File, rate: u32, interleaved: &[f32]) -> u16 {
    let mut error = 0;
    let encoder = unsafe { opusic_sys::opus_encoder_create(48_000, 2, opusic_sys::OPUS_APPLICATION_AUDIO, &mut error) };
    assert_eq!(error, opusic_sys::OPUS_OK);
    let mut pre_skip = 0i32;
    unsafe {
        opusic_sys::opus_encoder_ctl(encoder, opusic_sys::OPUS_GET_LOOKAHEAD_REQUEST, &mut pre_skip);
    }
    let head = opus_head(2, pre_skip as u16, rate);
    let tags = opus_tags();
    let mut writer = PacketWriter::new(&mut file);
    writer.write_packet(head, 1, PacketWriteEndInfo::EndPage, 0).expect("opus head");
    writer.write_packet(tags, 1, PacketWriteEndInfo::EndPage, 0).expect("opus tags");
    let frame = 960usize;
    let content = interleaved.len() / 2;
    // Feed lookahead zeros so the encoder delay line emits the last content samples.
    let total = content + pre_skip as usize;
    let end_granule = pre_skip as u64 + content as u64;
    let mut granule = pre_skip as u64;
    let mut packet = vec![0u8; 4000];
    let mut cursor = 0;
    while cursor < total {
        let n = frame.min(total - cursor);
        let mut block = vec![0f32; frame * 2];
        let from_content = cursor.min(content);
        let content_n = content.saturating_sub(cursor).min(n);
        if content_n > 0 {
            block[..content_n * 2].copy_from_slice(&interleaved[from_content * 2..(from_content + content_n) * 2]);
        }
        let written = unsafe {
            opusic_sys::opus_encode_float(encoder, block.as_ptr(), frame as i32, packet.as_mut_ptr(), packet.len() as i32)
        };
        assert!(written > 0, "opus encode {written}");
        cursor += n;
        granule = (granule + n as u64).min(end_granule);
        let end = if cursor >= total { PacketWriteEndInfo::EndStream } else { PacketWriteEndInfo::NormalPacket };
        writer.write_packet(packet[..written as usize].to_vec(), 1, end, granule).expect("opus page");
    }
    unsafe { opusic_sys::opus_encoder_destroy(encoder) };
    pre_skip as u16
}

fn opus_head(channels: u8, pre_skip: u16, rate: u32) -> Vec<u8> {
    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");
    head.push(1);
    head.push(channels);
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&rate.to_le_bytes());
    head.extend_from_slice(&0i16.to_le_bytes());
    head.push(0);
    head
}

fn opus_tags() -> Vec<u8> {
    let vendor = b"llamp-fixtures";
    let mut tags = Vec::new();
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes());
    tags
}
