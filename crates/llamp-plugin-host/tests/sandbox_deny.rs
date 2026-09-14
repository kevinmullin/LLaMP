//! Denial is at the WASI socket and preopen, not a URL string check.

use std::io::Write;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use llamp_plugin_api::Manifest;
use llamp_plugin_host::sandbox::{self, SandboxRequest};

fn lyrics_manifest() -> Manifest {
    Manifest::parse(
        r#"{
          "id": "fixture.lyrics.deny",
          "name": "Deny fixture",
          "version": "0.1.0",
          "abi": "^1.0.0",
          "kind": "LyricsProvider",
          "capabilities": { "network_hosts": ["lrclib.net"], "filesystem": [] }
        }"#,
    )
    .expect("manifest")
}

#[test]
fn undeclared_host_fails_at_the_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.set_nonblocking(true).expect("nonblocking");
    let addr = listener.local_addr().expect("addr");
    let accepted = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&accepted);
    thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if let Ok((mut stream, _)) = listener.accept() {
                flag.store(true, Ordering::SeqCst);
                let _ = stream.write_all(b"leak");
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
    });

    let wasm = sandbox::fixture_connect_wasm().expect("compile connect fixture");
    let manifest = lyrics_manifest();
    let outcome = sandbox::run_user_wasm(SandboxRequest {
        manifest: &manifest,
        wasm: &wasm,
        args: &[addr.to_string()],
        fs_grants: &[],
    });

    assert!(
        !accepted.load(Ordering::SeqCst),
        "guest reached the undeclared socket; deny must happen before connect completes"
    );
    assert!(
        !outcome.ok,
        "guest must not report success; stderr={}",
        outcome.stderr
    );
    assert!(
        !outcome.stderr.to_ascii_lowercase().contains("url"),
        "denial must not be a URL string comparison: {}",
        outcome.stderr
    );
    assert!(
        outcome.denied_at_socket,
        "expected a socket-layer deny, got ok={} stderr={} error={:?}",
        outcome.ok, outcome.stderr, outcome.error
    );
}

#[test]
fn undeclared_path_fails_at_the_grant() {
    let root = std::env::temp_dir().join(format!("llamp-sandbox-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let grant = root.join("grant");
    let outside = root.join("outside");
    std::fs::create_dir_all(&grant).expect("grant");
    std::fs::create_dir_all(&outside).expect("outside");
    let secret = outside.join("secret.txt");
    std::fs::write(&secret, b"top-secret").expect("secret");
    let wasm = sandbox::fixture_read_wasm().expect("compile read fixture");
    let manifest = lyrics_manifest();
    let outcome = sandbox::run_user_wasm(SandboxRequest {
        manifest: &manifest,
        wasm: &wasm,
        args: &[secret.to_string_lossy().into_owned()],
        fs_grants: &[grant],
    });
    assert!(
        !outcome.ok,
        "guest read an undeclared path: {}",
        outcome.stderr
    );
    assert!(
        !outcome.stderr.contains("top-secret"),
        "secret bytes must not come back: {}",
        outcome.stderr
    );
    assert!(
        outcome.denied_at_path,
        "expected a filesystem grant deny, got ok={} stderr={} error={:?}",
        outcome.ok, outcome.stderr, outcome.error
    );
    let _ = std::fs::remove_dir_all(&root);
}
