pub mod analysis;
pub mod cli;
pub mod cpal_output;
pub mod decode;
pub mod eq;
pub mod gapless;
pub mod graph;
pub mod limiter;
pub mod live;
pub mod opus;
pub mod output;
pub mod pcm;
pub mod realtime;
pub mod tags;
pub mod wav;

pub use eq::{
    band_target_db, drag_band, drag_preamp, preamp_target_db, set_eq_enabled, set_eq_sweep_band, sweep_band_at,
};
pub use graph::{band_sweep_ratios, Stage};

/// Graph the CLI and the cpal callback run. Allocated before the stream starts.
pub fn playback_stage(sample_rate: u32) -> Stage {
    Stage::for_playback(sample_rate)
}
