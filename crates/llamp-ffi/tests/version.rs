use std::ffi::CStr;

#[test]
fn version_matches_cargo_pkg_version() {
    let ptr = llamp_ffi::llamp_version();
    assert!(!ptr.is_null(), "llamp_version returned a null pointer");
    let version = unsafe { CStr::from_ptr(ptr) };
    let version = version.to_str().expect("version is UTF-8");
    assert_eq!(version, env!("CARGO_PKG_VERSION"));
}
