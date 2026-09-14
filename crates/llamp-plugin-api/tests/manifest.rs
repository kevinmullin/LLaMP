use llamp_plugin_api::{Manifest, PluginKind, HOST_ABI};

const EXAMPLE: &str = r#"{
  "id": "example.lyrics",
  "name": "Example lyrics",
  "version": "0.1.0",
  "abi": "^1.0.0",
  "kind": "LyricsProvider",
  "capabilities": {
    "network_hosts": ["lrclib.net"],
    "filesystem": []
  }
}"#;

#[test]
fn spec_example_parses() {
    let manifest = Manifest::parse(EXAMPLE).expect("parse");
    assert_eq!(manifest.id, "example.lyrics");
    assert_eq!(manifest.kind, PluginKind::LyricsProvider);
    assert_eq!(manifest.capabilities.network_hosts, ["lrclib.net"]);
    assert!(manifest.abi_matches(&HOST_ABI));
}

#[test]
fn unknown_fields_are_ignored() {
    let json = r#"{
      "id": "example.extra",
      "name": "Extra",
      "version": "1.0.0",
      "abi": "^1.0.0",
      "kind": "Metadata",
      "capabilities": { "network_hosts": [], "filesystem": [] },
      "future": { "x": 1 }
    }"#;
    let manifest = Manifest::parse(json).expect("unknown fields are ignored");
    assert_eq!(manifest.id, "example.extra");
}

#[test]
fn missing_kind_is_refused() {
    let json = r#"{
      "id": "example.nokind",
      "name": "No kind",
      "version": "1.0.0",
      "abi": "^1.0.0",
      "capabilities": { "network_hosts": [], "filesystem": [] }
    }"#;
    let err = Manifest::parse(json).expect_err("missing kind");
    assert!(!err.reason.is_empty());
}

#[test]
fn unknown_kind_is_refused() {
    let json = r#"{
      "id": "example.badkind",
      "name": "Bad kind",
      "version": "1.0.0",
      "abi": "^1.0.0",
      "kind": "WinampDll",
      "capabilities": { "network_hosts": [], "filesystem": [] }
    }"#;
    let err = Manifest::parse(json).expect_err("unknown kind");
    assert!(!err.reason.is_empty());
}

#[test]
fn user_audio_callback_is_refused() {
    let json = r#"{
      "id": "example.dsp",
      "name": "User DSP",
      "version": "1.0.0",
      "abi": "^1.0.0",
      "kind": "LyricsProvider",
      "capabilities": {
        "network_hosts": [],
        "filesystem": [],
        "audio_callback": true
      }
    }"#;
    let err = Manifest::parse_user(json).expect_err("user audio_callback");
    assert!(err.reason.contains("audio_callback") || err.reason.contains("native"));
}
