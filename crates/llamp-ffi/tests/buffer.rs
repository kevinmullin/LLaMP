use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

const BUF_LEN: usize = 64 * 1024;

static LIVE_64K: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() && layout.size() == BUF_LEN {
            LIVE_64K.fetch_add(1, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() == BUF_LEN {
            LIVE_64K.fetch_sub(1, Ordering::SeqCst);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

fn live_64k() -> usize {
    LIVE_64K.load(Ordering::SeqCst)
}

#[test]
fn buffer_is_64kb_and_free_rejects_double_free() {
    let before = live_64k();
    let buf = llamp_ffi::llamp_buffer();
    assert_eq!(
        live_64k(),
        before + 1,
        "llamp_buffer did not allocate a 64KB block"
    );
    assert!(!buf.data.is_null());
    assert_eq!(buf.len, BUF_LEN);
    let bytes = unsafe { std::slice::from_raw_parts(buf.data, buf.len) };
    assert_eq!(bytes[0], 0x5A);
    assert_eq!(bytes[BUF_LEN - 1], 0x5A);

    assert_eq!(llamp_ffi::llamp_buffer_free(buf.data), llamp_ffi::LLAMP_OK);
    assert_eq!(
        live_64k(),
        before,
        "llamp_buffer_free leaked the 64KB block"
    );

    assert_eq!(
        llamp_ffi::llamp_buffer_free(buf.data),
        llamp_ffi::LLAMP_ERR_INVALID,
        "double-free was not rejected"
    );
    assert_eq!(
        live_64k(),
        before,
        "double-free deallocated or allocated a 64KB block"
    );
}
