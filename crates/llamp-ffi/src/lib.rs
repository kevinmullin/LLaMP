use std::ffi::c_char;

/// NUL-terminated crate version. The caller does not free this pointer.
#[no_mangle]
pub extern "C" fn llamp_version() -> *const c_char {
    static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    VERSION.as_ptr().cast()
}
