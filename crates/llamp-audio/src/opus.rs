//! Opus via `symphonia-adapter-libopus`. Symphonia 0.6 has no opus feature.

use symphonia::core::codecs::registry::CodecRegistry;
use symphonia_adapter_libopus::OpusDecoder;

pub fn register(registry: &mut CodecRegistry) {
    registry.register_audio_decoder::<OpusDecoder>();
}
