//! Delay and padding trim. AAC-LC is not gapless. Do not guess an AAC trim.

use crate::decode::CodecId;

/// AAC-LC is excluded from gapless. Status is "Great", not gapless.
pub fn trim_enabled(codec: CodecId) -> bool {
    !matches!(codec, CodecId::AacLc)
}

/// Samples to drop at each end when the decoder did not already trim them.
/// AAC always returns `(0, 0)` even if a container field is present.
pub fn trim_counts(
    codec: CodecId,
    delay: Option<u32>,
    padding: Option<u32>,
    already_trimmed: bool,
) -> (u64, u64) {
    if already_trimmed || !trim_enabled(codec) {
        return (0, 0);
    }
    (
        u64::from(delay.unwrap_or(0)),
        u64::from(padding.unwrap_or(0)),
    )
}
