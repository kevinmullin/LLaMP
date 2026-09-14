//! Plugin kinds, manifest, and MediaSource capability flags. This crate does not load.

use std::path::PathBuf;

use semver::{Version, VersionReq};
use serde::Deserialize;

/// Host plugin ABI. Distinct from the app version.
pub const HOST_ABI: Version = Version::new(1, 0, 0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refuse {
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginKind {
    MediaSource,
    Decoder,
    Dsp,
    Output,
    Visualizer,
    LyricsProvider,
    Metadata,
    UiPanel,
}

impl PluginKind {
    fn parse(name: &str) -> Result<Self, Refuse> {
        match name {
            "MediaSource" => Ok(Self::MediaSource),
            "Decoder" => Ok(Self::Decoder),
            "DSP" => Ok(Self::Dsp),
            "Output" => Ok(Self::Output),
            "Visualizer" => Ok(Self::Visualizer),
            "LyricsProvider" => Ok(Self::LyricsProvider),
            "Metadata" => Ok(Self::Metadata),
            "UIPanel" => Ok(Self::UiPanel),
            other => Err(Refuse {
                reason: format!("unknown kind {other}"),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Capabilities {
    pub network_hosts: Vec<String>,
    pub filesystem: Vec<String>,
    pub audio_callback: bool,
    pub gpu_surface: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: Version,
    pub abi: VersionReq,
    pub kind: PluginKind,
    pub capabilities: Capabilities,
}

#[derive(Deserialize)]
struct RawManifest {
    id: String,
    name: String,
    version: String,
    abi: String,
    kind: Option<String>,
    #[serde(default)]
    capabilities: RawCapabilities,
}

#[derive(Default, Deserialize)]
struct RawCapabilities {
    #[serde(default)]
    network_hosts: Vec<String>,
    #[serde(default)]
    filesystem: Vec<String>,
    #[serde(default)]
    audio_callback: bool,
    #[serde(default)]
    gpu_surface: bool,
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Self, Refuse> {
        let raw: RawManifest = serde_json::from_str(json).map_err(|err| Refuse {
            reason: err.to_string(),
        })?;
        let kind = raw.kind.ok_or_else(|| Refuse {
            reason: "missing kind".into(),
        })?;
        Ok(Self {
            id: raw.id,
            name: raw.name,
            version: Version::parse(&raw.version).map_err(|err| Refuse {
                reason: err.to_string(),
            })?,
            abi: VersionReq::parse(&raw.abi).map_err(|err| Refuse {
                reason: err.to_string(),
            })?,
            kind: PluginKind::parse(&kind)?,
            capabilities: Capabilities {
                network_hosts: raw.capabilities.network_hosts,
                filesystem: raw.capabilities.filesystem,
                audio_callback: raw.capabilities.audio_callback,
                gpu_surface: raw.capabilities.gpu_surface,
            },
        })
    }

    /// User-installed plugins. Native-only capability bits are a refusal.
    pub fn parse_user(json: &str) -> Result<Self, Refuse> {
        let manifest = Self::parse(json)?;
        if manifest.capabilities.audio_callback || manifest.capabilities.gpu_surface {
            return Err(Refuse {
                reason:
                    "user plugin may not set audio_callback or gpu_surface; those are native-only"
                        .into(),
            });
        }
        Ok(manifest)
    }

    pub fn abi_matches(&self, host: &Version) -> bool {
        self.abi.matches(host)
    }
}

/// Flags on a resolved track. Tier A is `produces_pcm`. The shell does not branch on a provider name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceFlags {
    pub produces_pcm: bool,
    pub supports_seek: bool,
    pub supports_volume: bool,
    pub supports_balance: bool,
    pub supports_eq: bool,
    pub supports_gapless: bool,
    pub supports_replaygain: bool,
    pub duration_known: bool,
    pub position_authoritative: bool,
    pub requires_network: bool,
}

impl SourceFlags {
    pub fn local_file() -> Self {
        Self {
            produces_pcm: true,
            supports_seek: true,
            supports_volume: true,
            supports_balance: true,
            supports_eq: true,
            supports_gapless: true,
            supports_replaygain: true,
            duration_known: true,
            position_authoritative: true,
            requires_network: false,
        }
    }

    pub fn tier_b_remote() -> Self {
        Self {
            produces_pcm: false,
            supports_seek: true,
            supports_volume: true,
            supports_balance: false,
            supports_eq: false,
            supports_gapless: false,
            supports_replaygain: false,
            duration_known: true,
            position_authoritative: false,
            requires_network: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceItem {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub item: SourceItem,
    pub flags: SourceFlags,
}

pub trait MediaSource: Send + Sync {
    fn browse(&self) -> Result<Vec<SourceItem>, String>;
    fn search(&self, query: &str) -> Result<Vec<SourceItem>, String>;
    fn resolve(&self, id: &str) -> Result<Resolved, String>;
}

pub fn item_from_path(path: PathBuf) -> SourceItem {
    let label = path.to_string_lossy().into_owned();
    SourceItem {
        id: label.clone(),
        label,
    }
}
