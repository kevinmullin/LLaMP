use std::alloc::{alloc, dealloc, Layout};
use std::ffi::c_char;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

pub const LLAMP_OK: i32 = 0;
pub const LLAMP_ERR_INVALID: i32 = 1;

const BUF_LEN: usize = 64 * 1024;

static OUTSTANDING: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn buf_layout() -> Layout {
    Layout::from_size_align(BUF_LEN, 1).expect("64KB layout")
}

/// NUL-terminated crate version. The caller does not free this pointer.
#[no_mangle]
pub extern "C" fn llamp_version() -> *const c_char {
    static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    VERSION.as_ptr().cast()
}

/// 64 KiB dummy buffer owned by the core.
///
/// The caller frees `data` with `llamp_buffer_free` exactly once. `len` is 65536.
/// This struct is not a heap allocation. Do not pass it to `llamp_buffer_free`.
#[repr(C)]
pub struct LlampBuffer {
    pub data: *mut u8,
    pub len: usize,
}

/// Returns a 64 KiB dummy buffer. The caller frees `data` with `llamp_buffer_free` exactly once.
///
/// A null `data` and `len` of 0 means no buffer was returned. That happens on allocation failure,
/// or if a previous buffer has not been freed. The failed call does not leak.
#[no_mangle]
pub extern "C" fn llamp_buffer() -> LlampBuffer {
    let layout = buf_layout();
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        return LlampBuffer {
            data: std::ptr::null_mut(),
            len: 0,
        };
    }
    unsafe { std::ptr::write_bytes(ptr, 0x5A, BUF_LEN) };
    match OUTSTANDING.compare_exchange(
        std::ptr::null_mut(),
        ptr,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => LlampBuffer {
            data: ptr,
            len: BUF_LEN,
        },
        Err(_) => {
            unsafe { dealloc(ptr, layout) };
            LlampBuffer {
                data: std::ptr::null_mut(),
                len: 0,
            }
        }
    }
}

/// Frees `data` from `llamp_buffer`.
///
/// Returns `LLAMP_OK` after deallocating the 64 KiB block. Returns `LLAMP_ERR_INVALID` for a
/// null pointer, an unknown pointer, or a double-free, and does not deallocate in those cases.
#[no_mangle]
pub extern "C" fn llamp_buffer_free(data: *mut u8) -> i32 {
    if data.is_null() {
        return LLAMP_ERR_INVALID;
    }
    match OUTSTANDING.compare_exchange(
        data,
        std::ptr::null_mut(),
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(ptr) => {
            unsafe { dealloc(ptr, buf_layout()) };
            LLAMP_OK
        }
        Err(_) => LLAMP_ERR_INVALID,
    }
}

/// Increments the frame counter. Takes no lock.
#[no_mangle]
pub extern "C" fn llamp_counter_publish() {
    COUNTER.fetch_add(1, Ordering::Release);
}

/// Loads the frame counter. Does not wait.
#[no_mangle]
pub extern "C" fn llamp_counter_poll() -> u64 {
    COUNTER.load(Ordering::Acquire)
}
