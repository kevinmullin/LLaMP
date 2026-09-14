//! Float and 16-bit WAV writer. No extra crate.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

#[derive(Clone, Copy)]
pub enum WavBits {
    F32,
    Pcm16,
}

pub fn write_wav(path: &Path, sample_rate: u32, interleaved: &[f32], bits: WavBits) -> io::Result<()> {
    let channels = 2u16;
    let (format, bytes_per_sample): (u16, u16) = match bits {
        WavBits::Pcm16 => (1, 2),
        WavBits::F32 => (3, 4),
    };
    let block_align = channels * bytes_per_sample;
    let byte_rate = sample_rate * u32::from(block_align);
    let data_bytes = interleaved.len() * bytes_per_sample as usize;
    let mut file = File::create(path)?;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_bytes as u32).to_le_bytes())?;
    file.write_all(b"WAVE")?;
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&format.to_le_bytes())?;
    file.write_all(&channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&(bytes_per_sample * 8).to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&(data_bytes as u32).to_le_bytes())?;
    match bits {
        WavBits::Pcm16 => {
            for sample in interleaved {
                let q = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
                file.write_all(&q.to_le_bytes())?;
            }
        }
        WavBits::F32 => {
            for sample in interleaved {
                file.write_all(&sample.to_le_bytes())?;
            }
        }
    }
    Ok(())
}
