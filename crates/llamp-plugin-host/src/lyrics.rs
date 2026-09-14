//! Host HTTP for LyricsProvider. The guest does not open a socket.

use std::sync::Arc;

use llamp_plugin_api::{LyricQuery, LyricsProvider, Manifest};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::p2::add_to_linker_sync;
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

mod bindings {
    wasmtime::component::bindgen!({
        path: "wit/lyrics.wit",
        world: "lyrics",
    });
}

pub const LRCLIB_HOST: &str = "lrclib.net";
pub const LRCLIB_PATH: &str = "/api/get";
pub const USER_AGENT: &str = "LLaMP/0.1.0 (https://github.com/kevinmullin/LLaMP)";

pub trait HttpGet: Send + Sync {
    fn get(&self, host: &str, path: &str, query: &str) -> Result<String, String>;
}

pub struct HostHttp {
    pub manifest: Manifest,
    pub opted_in: bool,
    pub inner: Arc<dyn HttpGet>,
}

impl HostHttp {
    pub fn get(&self, host: &str, path: &str, query: &str) -> Result<String, String> {
        if !self.opted_in {
            return Err("opt-in required".into());
        }
        if host.is_empty() || host.contains('/') || host.contains('*') || host.contains(':') {
            return Err("invalid host".into());
        }
        if !self
            .manifest
            .capabilities
            .network_hosts
            .iter()
            .any(|allowed| allowed == host)
        {
            return Err("undeclared host".into());
        }
        if !path.starts_with('/') || path.contains("://") {
            return Err("invalid path".into());
        }
        self.inner.get(host, path, query)
    }
}

pub struct UreqGet;

impl HttpGet for UreqGet {
    fn get(&self, host: &str, path: &str, query: &str) -> Result<String, String> {
        let url = format!("https://{host}{path}?{query}");
        ureq::get(&url)
            .set("User-Agent", USER_AGENT)
            .call()
            .map_err(|err| err.to_string())?
            .into_string()
            .map_err(|err| err.to_string())
    }
}

/// WASM LyricsProvider. The guest builds the query and parses the body.
/// The host is the only socket.
pub struct WasmLyrics {
    pub http: HostHttp,
    pub wasm: Vec<u8>,
}

impl LyricsProvider for WasmLyrics {
    fn fetch(&self, query: &LyricQuery) -> Result<String, String> {
        run_lyrics_provider(&self.wasm, &self.http, query)
    }
}

struct Ctx {
    wasi: WasiCtx,
    table: ResourceTable,
    http: HostHttp,
}

impl WasiView for Ctx {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl bindings::llamp::lyrics::host_http::Host for Ctx {
    fn get(&mut self, host: String, path: String, query: String) -> Result<String, String> {
        self.http.get(&host, &path, &query)
    }
}

pub fn run_lyrics_provider(
    wasm: &[u8],
    http: &HostHttp,
    query: &LyricQuery,
) -> Result<String, String> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.epoch_interruption(true);
    let engine = Engine::new(&config).map_err(|err| err.to_string())?;
    let component = Component::from_binary(&engine, wasm).map_err(|err| err.to_string())?;
    let mut linker = Linker::new(&engine);
    add_to_linker_sync(&mut linker).map_err(|err| err.to_string())?;
    bindings::Lyrics::add_to_linker::<Ctx, HasSelf<_>>(&mut linker, |ctx| ctx)
        .map_err(|err| err.to_string())?;

    let stderr = MemoryOutputPipe::new(16 * 1024);
    let mut builder = WasiCtx::builder();
    builder.stderr(stderr.clone());
    let store_http = HostHttp {
        manifest: http.manifest.clone(),
        opted_in: http.opted_in,
        inner: Arc::clone(&http.inner),
    };
    let mut store = Store::new(
        &engine,
        Ctx {
            wasi: builder.build(),
            table: ResourceTable::new(),
            http: store_http,
        },
    );
    store.set_epoch_deadline(1);
    let lyrics = bindings::Lyrics::instantiate(&mut store, &component, &linker)
        .map_err(|err| err.to_string())?;
    lyrics
        .llamp_lyrics_lyrics_provider()
        .call_fetch_match(
            &mut store,
            &query.artist,
            &query.title,
            &query.album,
            query.duration_seconds,
        )
        .map_err(|err| err.to_string())?
}

pub fn compile_lrclib_wasm() -> Result<Vec<u8>, String> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let workspace = std::path::Path::new(&manifest)
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("plugins").exists())
        .ok_or("workspace")?;
    let status = std::process::Command::new("cargo")
        .args([
            "build",
            "-p",
            "llamp-lyrics-lrclib",
            "--target",
            "wasm32-wasip2",
        ])
        .current_dir(workspace)
        .status()
        .map_err(|err| err.to_string())?;
    if !status.success() {
        return Err("llamp-lyrics-lrclib wasm build failed".into());
    }
    let path = workspace
        .join("target/wasm32-wasip2/debug/llamp_lyrics_lrclib.wasm");
    std::fs::read(path).map_err(|err| err.to_string())
}
