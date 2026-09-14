//! ReplayGain seam. `lofty` is the 0.25.2 adapter, not the type the session depends on.
//!
//! ReplayGain 2.0 uses the same field names as v1 (`REPLAYGAIN_*`). lofty 0.25.2 has one
//! `ItemKey` set for those names, plus separate `R128*` keys. This adapter fills v2 from the
//! ReplayGain keys and leaves v1 empty. A source that can tell them apart still reports both;
//! selection prefers v2.

use std::path::Path;

use lofty::file::TaggedFileExt;
use lofty::tag::ItemKey;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GainTags {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
    pub album_id: Option<String>,
    pub v2: bool,
    pub v1_gain_db: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppliedGain {
    pub db: f32,
    pub used_replaygain: bool,
    pub used_album: bool,
}

pub trait ReplayGainSource {
    fn read(&self, path: &Path) -> Result<GainTags, TagError>;
}

#[derive(Debug)]
pub struct TagError(pub String);

impl std::fmt::Display for TagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TagError {}

#[derive(Clone, Copy, Default)]
pub struct LoftySource;

impl ReplayGainSource for LoftySource {
    fn read(&self, path: &Path) -> Result<GainTags, TagError> {
        let file = lofty::read_from_path(path).map_err(|err| TagError(err.to_string()))?;
        let Some(tag) = file.primary_tag() else {
            return Ok(GainTags::default());
        };
        let track = tag.get_string(ItemKey::ReplayGainTrackGain).map(parse_db);
        let album = tag.get_string(ItemKey::ReplayGainAlbumGain).map(parse_db);
        Ok(GainTags {
            track_gain_db: track.flatten(),
            track_peak: tag
                .get_string(ItemKey::ReplayGainTrackPeak)
                .and_then(parse_peak),
            album_gain_db: album.flatten(),
            album_peak: tag
                .get_string(ItemKey::ReplayGainAlbumPeak)
                .and_then(parse_peak),
            album_id: tag
                .get_string(ItemKey::MusicBrainzReleaseId)
                .map(str::to_string),
            v2: track.is_some() || album.is_some(),
            v1_gain_db: None,
        })
    }
}

pub fn select_gain(current: &GainTags, next: Option<&GainTags>, rg_preamp_db: f32) -> AppliedGain {
    let track_db = current
        .v2
        .then_some(current.track_gain_db)
        .flatten()
        .or(current.v1_gain_db)
        .or(current.track_gain_db);
    let album_ok = match (current.album_id.as_deref(), next) {
        (Some(id), Some(next)) => {
            next.album_id.as_deref() == Some(id)
                && current.album_gain_db.is_some()
                && next.album_gain_db.is_some()
        }
        _ => false,
    };
    let (db, peak, used_album, used) = if album_ok {
        (current.album_gain_db, current.album_peak, true, true)
    } else if let Some(db) = track_db {
        (Some(db), current.track_peak, false, true)
    } else {
        (None, None, false, false)
    };
    let mut applied = db.unwrap_or(0.0) + if used { rg_preamp_db } else { 0.0 };
    if let Some(peak) = peak.filter(|p| *p > 0.0) {
        let ceiling = 10f32.powf(-1.0 / 20.0);
        let linear = 10f32.powf(applied / 20.0);
        let max_linear = ceiling / peak;
        if linear > max_linear {
            applied = 20.0 * max_linear.log10();
        }
    }
    AppliedGain {
        db: applied,
        used_replaygain: used,
        used_album,
    }
}

fn parse_db(text: &str) -> Option<f32> {
    let number = text.split_whitespace().next()?;
    number.parse().ok()
}

fn parse_peak(text: &str) -> Option<f32> {
    text.split_whitespace().next()?.parse().ok()
}
