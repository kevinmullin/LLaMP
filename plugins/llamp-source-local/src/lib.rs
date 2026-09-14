//! First-party MediaSource for granted local files. Paths stay on disk.

use std::path::Path;
use std::sync::Arc;

use llamp_library::Library;
use llamp_plugin_api::{item_from_path, MediaSource, Resolved, SourceFlags, SourceItem};

pub struct LocalSource {
    lib: Arc<Library>,
}

impl LocalSource {
    pub fn open(path: &Path) -> Result<Self, String> {
        Ok(Self {
            lib: Arc::new(Library::open(path)?),
        })
    }

    pub fn grant_folder(&self, dir: &Path) -> Result<usize, String> {
        self.lib.grant_folder(dir)
    }

    pub fn insert_referenced_dir(&self, dir: &Path) -> Result<usize, String> {
        self.lib.insert_referenced_dir(dir)
    }

    pub fn enqueue(&self, path: &Path) -> Result<(), String> {
        self.lib.enqueue(path)
    }

    pub fn library(&self) -> &Library {
        &self.lib
    }
}

impl MediaSource for LocalSource {
    fn browse(&self) -> Result<Vec<SourceItem>, String> {
        Ok(self
            .lib
            .granted_paths()?
            .into_iter()
            .map(item_from_path)
            .collect())
    }

    fn search(&self, query: &str) -> Result<Vec<SourceItem>, String> {
        Ok(self
            .lib
            .search(query)?
            .into_iter()
            .map(|hit| item_from_path(hit.path))
            .collect())
    }

    fn resolve(&self, id: &str) -> Result<Resolved, String> {
        let item = item_from_path(Path::new(id).to_path_buf());
        Ok(Resolved {
            item,
            flags: SourceFlags::local_file(),
        })
    }
}
