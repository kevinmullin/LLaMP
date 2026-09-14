use llamp_core::Session;
use llamp_plugin_api::SourceFlags;

#[test]
fn session_supports_eq_comes_from_the_provider() {
    let session = Session::new();
    assert_eq!(session.poll().supports_eq, 1, "idle is local / Tier A");
    session.apply_source_flags(SourceFlags::tier_b_remote());
    let snap = session.poll();
    assert_eq!(snap.supports_eq, 0);
    assert_eq!(snap.produces_pcm, 0);
    session.apply_source_flags(SourceFlags::local_file());
    let snap = session.poll();
    assert_eq!(snap.supports_eq, 1);
    assert_eq!(snap.produces_pcm, 1);
}
