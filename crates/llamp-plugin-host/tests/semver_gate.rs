use llamp_plugin_api::HOST_ABI;
use llamp_plugin_host::Registry;

#[test]
fn semver_incompatible_plugin_is_refused_with_a_reason() {
    let json = r#"{
      "id": "example.old",
      "name": "Old",
      "version": "0.1.0",
      "abi": "^0.9.0",
      "kind": "LyricsProvider",
      "capabilities": { "network_hosts": [], "filesystem": [] }
    }"#;
    let mut registry = Registry::new();
    let err = registry
        .load_json(json)
        .expect_err("incompatible abi must not load");
    assert!(
        err.reason.contains("abi") || err.reason.contains('^') || err.reason.contains("0.9"),
        "reason should name the abi gate: {}",
        err.reason
    );
    assert_eq!(
        registry.refused_reason("example.old").expect("recorded"),
        err.reason
    );
    assert!(!registry.is_enabled("example.old"));
    assert_eq!(HOST_ABI.to_string(), "1.0.0");
}

#[test]
fn compatible_plugin_can_be_disabled_and_enabled() {
    let json = r#"{
      "id": "example.ok",
      "name": "Ok",
      "version": "0.1.0",
      "abi": "^1.0.0",
      "kind": "LyricsProvider",
      "capabilities": { "network_hosts": ["lrclib.net"], "filesystem": [] }
    }"#;
    let mut registry = Registry::new();
    registry.load_json(json).expect("compatible");
    assert!(registry.is_enabled("example.ok"));
    registry.disable("example.ok").expect("disable");
    assert!(!registry.is_enabled("example.ok"));
    registry.enable("example.ok").expect("enable");
    assert!(registry.is_enabled("example.ok"));
}
