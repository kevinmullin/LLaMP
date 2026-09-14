//! Native first-party host and wasmtime user sandbox.

use std::sync::Arc;

use llamp_plugin_api::{Manifest, MediaSource, Refuse, HOST_ABI};

pub mod lyrics;
pub mod sandbox;
pub mod vis_budget;

/// Native first-party hot reload is a dev-build demo. A notarized binary with a
/// hardened runtime does not dlopen a rebuilt dylib. 1.0 is disable and enable
/// without an audio drop.
pub const NATIVE_RELOAD_POLICY: &str =
    "Native first-party hot reload is a dev-build demo. A notarized binary with a hardened runtime does not dlopen a rebuilt dylib.";

pub const WASMTIME_VERSION: &str = "48.0.2";

struct Entry {
    manifest: Manifest,
    enabled: bool,
    refuse: Option<Refuse>,
    source: Option<Arc<dyn MediaSource>>,
}

pub struct Registry {
    entries: Vec<Entry>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn load_json(&mut self, json: &str) -> Result<(), Refuse> {
        let manifest = Manifest::parse(json)?;
        self.insert(manifest, None)
    }

    pub fn register_native(
        &mut self,
        manifest: Manifest,
        source: Option<Arc<dyn MediaSource>>,
    ) -> Result<(), Refuse> {
        self.insert(manifest, source)
    }

    fn insert(
        &mut self,
        manifest: Manifest,
        source: Option<Arc<dyn MediaSource>>,
    ) -> Result<(), Refuse> {
        if !manifest.abi_matches(&HOST_ABI) {
            let refuse = Refuse {
                reason: format!("abi {} does not include host {}", manifest.abi, HOST_ABI),
            };
            self.entries.push(Entry {
                manifest,
                enabled: false,
                refuse: Some(refuse.clone()),
                source: None,
            });
            return Err(refuse);
        }
        self.entries.push(Entry {
            manifest,
            enabled: true,
            refuse: None,
            source,
        });
        Ok(())
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.manifest.id == id)
            .map(|entry| entry.enabled)
            .unwrap_or(false)
    }

    pub fn refused_reason(&self, id: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.manifest.id == id)
            .and_then(|entry| entry.refuse.as_ref().map(|refuse| refuse.reason.as_str()))
    }

    pub fn enable(&mut self, id: &str) -> Result<(), String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.manifest.id == id)
            .ok_or_else(|| format!("unknown plugin {id}"))?;
        if entry.refuse.is_some() {
            return Err("refused plugin cannot be enabled".into());
        }
        entry.enabled = true;
        Ok(())
    }

    pub fn disable(&mut self, id: &str) -> Result<(), String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.manifest.id == id)
            .ok_or_else(|| format!("unknown plugin {id}"))?;
        entry.enabled = false;
        Ok(())
    }

    pub fn enabled_source(&self) -> Option<Arc<dyn MediaSource>> {
        self.entries
            .iter()
            .find(|entry| entry.enabled)
            .and_then(|entry| entry.source.clone())
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn id_at(&self, index: usize) -> Option<&str> {
        self.entries
            .get(index)
            .map(|entry| entry.manifest.id.as_str())
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
