//! Tag read and write. `lofty` 0.25.2 is the first implementation, not the type callers use.

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{ItemKey, Tag, TagType};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackTags {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
}

pub trait TagStore {
    fn read(&self, path: &Path) -> Result<TrackTags, String>;
    fn write(&self, path: &Path, tags: &TrackTags) -> Result<(), String>;
}

#[derive(Clone, Copy, Default)]
pub struct LoftyTags;

impl TagStore for LoftyTags {
    fn read(&self, path: &Path) -> Result<TrackTags, String> {
        let file = lofty::read_from_path(path).map_err(|err| err.to_string())?;
        let Some(tag) = file.primary_tag() else {
            return Ok(TrackTags::default());
        };
        Ok(TrackTags {
            title: text(tag, ItemKey::TrackTitle),
            artist: text(tag, ItemKey::TrackArtist),
            album: text(tag, ItemKey::AlbumTitle),
            genre: text(tag, ItemKey::Genre),
        })
    }

    fn write(&self, path: &Path, tags: &TrackTags) -> Result<(), String> {
        let mut tagged = lofty::read_from_path(path).map_err(|err| err.to_string())?;
        if tagged.primary_tag().is_none() {
            tagged.insert_tag(Tag::new(TagType::Id3v2));
        }
        {
            let tag = tagged.primary_tag_mut().ok_or("tag")?;
            tag.insert_text(ItemKey::TrackTitle, tags.title.clone());
            tag.insert_text(ItemKey::TrackArtist, tags.artist.clone());
            tag.insert_text(ItemKey::AlbumTitle, tags.album.clone());
            tag.insert_text(ItemKey::Genre, tags.genre.clone());
        }
        tagged
            .save_to_path(path, WriteOptions::default())
            .map_err(|err| err.to_string())
    }
}

fn text(tag: &Tag, key: ItemKey) -> String {
    tag.get_string(key).unwrap_or("").to_string()
}
