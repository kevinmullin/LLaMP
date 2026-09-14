//! Stereo fold. Extra channels share one equal-power coefficient.
//! L and R stay at unity, so silent extras do not quiet the file.
//! Peaks above ±1 are the limiter's job, not this fold.

/// Equal-power coefficient for each channel past the stereo pair: `1/sqrt(2)`.
pub const EXTRA_CHANNEL_COEFF: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Applied after the fold. `1.0` keeps a stereo pair inside a wider file at unity.
pub const DOWNMIX_TRIM: f32 = 1.0;

/// Fold interleaved PCM to interleaved stereo.
///
/// One channel is duplicated. Two are copied. Further channels are added to both
/// sides at [`EXTRA_CHANNEL_COEFF`], then multiplied by [`DOWNMIX_TRIM`].
pub fn fold_to_stereo(interleaved: &[f32], channels: u16) -> Vec<f32> {
    let channels = channels.max(1) as usize;
    let frames = interleaved.len() / channels;
    let mut out = Vec::with_capacity(frames * 2);
    if channels == 1 {
        for &s in interleaved.iter().take(frames) {
            out.push(s);
            out.push(s);
        }
        return out;
    }
    let k = if channels > 2 { EXTRA_CHANNEL_COEFF } else { 0.0 };
    for frame in 0..frames {
        let base = frame * channels;
        let mut left = interleaved[base];
        let mut right = interleaved[base + 1];
        if channels > 2 {
            let mut extras = 0.0f32;
            for sample in &interleaved[base + 2..base + channels] {
                extras += *sample;
            }
            left += k * extras;
            right += k * extras;
            left *= DOWNMIX_TRIM;
            right *= DOWNMIX_TRIM;
        }
        out.push(left);
        out.push(right);
    }
    out
}
