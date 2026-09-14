//! Disable and enable a first-party plugin while the callback keeps filling.
//! Native dylib reload is a dev-build demo, not a 1.0 release feature.

use llamp_audio::graph::Callback;
use llamp_audio::realtime;
use llamp_plugin_api::Manifest;
use llamp_plugin_host::{Registry, NATIVE_RELOAD_POLICY};
use llamp_source_local::LocalSource;

llamp_audio::install_realtime_hook!();

fn local_manifest() -> Manifest {
    Manifest::parse(
        r#"{
          "id": "llamp.source.local",
          "name": "Local files",
          "version": "0.1.0",
          "abi": "^1.0.0",
          "kind": "MediaSource",
          "capabilities": {
            "network_hosts": [],
            "filesystem": ["user-selected"]
          }
        }"#,
    )
    .expect("manifest")
}

#[test]
fn native_hot_reload_is_dev_builds_only() {
    assert!(
        NATIVE_RELOAD_POLICY.contains("dev-build") && NATIVE_RELOAD_POLICY.contains("hardened"),
        "{NATIVE_RELOAD_POLICY}"
    );
}

#[test]
fn disable_and_enable_first_party_plugin_does_not_drop_the_callback() {
    let root = std::env::temp_dir().join(format!("llamp-reload-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    let source = LocalSource::open(&root.join("library.sqlite")).expect("open");
    let mut registry = Registry::new();
    registry
        .register_native(local_manifest(), Some(std::sync::Arc::new(source)))
        .expect("register");
    assert!(registry.is_enabled("llamp.source.local"));

    let mut callback = Callback::new(256, 48_000);
    let mut out = [0f32; 512];
    let pcm = vec![0.25f32; 512 * 4];
    let mut last = 0u64;
    callback.push_pcm(&pcm);
    realtime::assert_section_clean(|| {
        callback.fill(&mut out);
        assert_eq!(callback.underruns(), 0);
        last = callback.played_frames();
        assert!(last > 0, "callback must publish frames");
    });

    registry.disable("llamp.source.local").expect("disable");
    callback.set_dsp(llamp_audio::graph::dsp_noop);
    callback.push_pcm(&pcm);
    realtime::assert_section_clean(|| {
        callback.fill(&mut out);
        assert_eq!(callback.underruns(), 0, "reload must not starve the ring");
        assert!(
            callback.played_frames() > last,
            "callback must keep publishing frames after disable"
        );
        last = callback.played_frames();
    });

    registry.enable("llamp.source.local").expect("enable");
    callback.set_dsp(llamp_audio::graph::dsp_noop);
    callback.push_pcm(&pcm);
    realtime::assert_section_clean(|| {
        callback.fill(&mut out);
        assert_eq!(callback.underruns(), 0, "reload must not starve the ring");
        assert!(
            callback.played_frames() > last,
            "callback must keep publishing frames after enable"
        );
    });
    let _ = std::fs::remove_dir_all(&root);
}
