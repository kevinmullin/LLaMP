//! wasmtime component host. No ambient network or filesystem.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use llamp_plugin_api::Manifest;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::p2::add_to_linker_sync;
use wasmtime_wasi::p2::bindings::sync::Command as WasiCommand;
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{FsPerms, WasiCtx, WasiCtxView, WasiView};

/// Custom section that may hold `llamp-plugin.json` inside a component.
pub const MANIFEST_SECTION: &str = "llamp-plugin.json";

const CONNECT_SRC: &str = r#"
fn main() {
    let addr = std::env::args().nth(1).expect("addr");
    match std::net::TcpStream::connect(addr.as_str()) {
        Ok(_) => {
            eprintln!("connected");
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("connect-error:{err}");
            std::process::exit(2);
        }
    }
}
"#;

const READ_SRC: &str = r#"
fn main() {
    let path = std::env::args().nth(1).expect("path");
    match std::fs::read(&path) {
        Ok(bytes) => {
            eprintln!("{}", String::from_utf8_lossy(&bytes));
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("read-error:{err}");
            std::process::exit(2);
        }
    }
}
"#;

pub struct SandboxRequest<'a> {
    pub manifest: &'a Manifest,
    pub wasm: &'a [u8],
    pub args: &'a [String],
    pub fs_grants: &'a [PathBuf],
}

pub struct SandboxOutcome {
    pub ok: bool,
    pub stderr: String,
    pub error: Option<String>,
    pub denied_at_socket: bool,
    pub denied_at_path: bool,
}

struct Ctx {
    wasi: WasiCtx,
    table: ResourceTable,
}

impl WasiView for Ctx {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

pub fn fixture_connect_wasm() -> Result<Vec<u8>, String> {
    static WASM: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();
    WASM.get_or_init(|| compile_fixture("connect", CONNECT_SRC))
        .clone()
}

pub fn fixture_read_wasm() -> Result<Vec<u8>, String> {
    static WASM: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();
    WASM.get_or_init(|| compile_fixture("read", READ_SRC))
        .clone()
}

pub fn run_user_wasm(req: SandboxRequest<'_>) -> SandboxOutcome {
    match run_user_wasm_inner(req) {
        Ok(outcome) => outcome,
        Err(error) => SandboxOutcome {
            ok: false,
            stderr: String::new(),
            error: Some(error),
            denied_at_socket: false,
            denied_at_path: false,
        },
    }
}

fn run_user_wasm_inner(req: SandboxRequest<'_>) -> Result<SandboxOutcome, String> {
    if !req.manifest.abi_matches(&llamp_plugin_api::HOST_ABI) {
        return Err("abi refused".into());
    }
    if req.manifest.capabilities.audio_callback || req.manifest.capabilities.gpu_surface {
        return Err("user plugin may not set audio_callback or gpu_surface".into());
    }

    let mut config = Config::new();
    config.wasm_component_model(true);
    config.epoch_interruption(true);
    let engine = Engine::new(&config).map_err(|err| err.to_string())?;
    let component = Component::from_binary(&engine, req.wasm).map_err(|err| err.to_string())?;

    let mut linker = Linker::new(&engine);
    add_to_linker_sync(&mut linker).map_err(|err| err.to_string())?;

    let stderr = MemoryOutputPipe::new(16 * 1024);
    let mut builder = WasiCtx::builder();
    builder.arg("plugin");
    for arg in req.args {
        builder.arg(arg);
    }
    builder.stderr(stderr.clone());
    builder.allow_tcp(true);
    builder.socket_addr_check(|_addr, _reason| Box::pin(async { false }));
    for grant in req.fs_grants {
        if !is_user_selected(grant) {
            continue;
        }
        builder
            .preopened_dir(grant, grant.to_string_lossy().as_ref(), FsPerms::ReadOnly)
            .map_err(|err| err.to_string())?;
    }

    let mut store = Store::new(
        &engine,
        Ctx {
            wasi: builder.build(),
            table: ResourceTable::new(),
        },
    );
    store.set_epoch_deadline(1);
    let ticker = engine.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        ticker.increment_epoch();
    });

    let command =
        WasiCommand::instantiate(&mut store, &component, &linker).map_err(|err| err.to_string())?;
    let ran = command.wasi_cli_run().call_run(&mut store);
    let stderr = String::from_utf8_lossy(&stderr.contents()).into_owned();
    let (ok, error) = match ran {
        Ok(Ok(())) => (true, None),
        Ok(Err(())) => (false, None),
        Err(err) => (false, Some(err.to_string())),
    };
    Ok(SandboxOutcome {
        denied_at_socket: stderr.contains("connect-error"),
        denied_at_path: stderr.contains("read-error"),
        ok,
        stderr,
        error,
    })
}

fn is_user_selected(path: &Path) -> bool {
    if path == Path::new("/") {
        return false;
    }
    if let Some(home) = std::env::var_os("HOME") {
        if path == Path::new(&home) {
            return false;
        }
    }
    path.is_dir()
}

fn compile_fixture(name: &str, src: &str) -> Result<Vec<u8>, String> {
    ensure_wasm_target()?;
    let dir = std::env::temp_dir().join(format!("llamp-wasm-{name}-{}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let rs = dir.join("main.rs");
    let wasm = dir.join("out.wasm");
    fs::write(&rs, src).map_err(|err| err.to_string())?;
    let status = Command::new("rustc")
        .args([
            "--target",
            "wasm32-wasip2",
            "--edition",
            "2021",
            "-C",
            "opt-level=1",
            "-o",
        ])
        .arg(&wasm)
        .arg(&rs)
        .status()
        .map_err(|err| err.to_string())?;
    if !status.success() {
        return Err(format!("rustc --target wasm32-wasip2 failed for {name}"));
    }
    fs::read(&wasm).map_err(|err| err.to_string())
}

fn ensure_wasm_target() -> Result<(), String> {
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .map_err(|err| err.to_string())?;
    let sysroot = String::from_utf8_lossy(&sysroot.stdout);
    let lib = PathBuf::from(sysroot.trim()).join("lib/rustlib/wasm32-wasip2");
    if lib.exists() {
        return Ok(());
    }
    let status = Command::new("rustup")
        .args(["target", "add", "wasm32-wasip2"])
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("wasm32-wasip2 target is missing".into())
    }
}
