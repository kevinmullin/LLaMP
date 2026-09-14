//! Guard for the audio callback and the FFT hop.
//! Allocation is a global allocator. Lock and I/O are a dylib interposer loaded
//! before the probe process starts. Either one fails the test.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Violation {
    Alloc,
    Lock,
    Io,
}

const ALLOC: u64 = 1;
const LOCK: u64 = 2;
const IO: u64 = 4;

static FLAGS: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static IN_SECTION: Cell<bool> = const { Cell::new(false) };
}

pub struct Hook;

impl Hook {
    pub const fn system() -> Self {
        Self
    }
}

unsafe impl GlobalAlloc for Hook {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if IN_SECTION.with(Cell::get) {
            FLAGS.fetch_or(ALLOC, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if IN_SECTION.with(Cell::get) {
            FLAGS.fetch_or(ALLOC, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[macro_export]
macro_rules! install_realtime_hook {
    () => {
        #[global_allocator]
        static LLAMP_REALTIME_HOOK: $crate::realtime::Hook = $crate::realtime::Hook::system();
    };
}

pub fn clear_violations() {
    FLAGS.store(0, Ordering::Relaxed);
    if let Some(reset) = dylib_reset() {
        unsafe { reset() };
    }
}

pub fn violations() -> Vec<Violation> {
    let bits = FLAGS.load(Ordering::Relaxed) | dylib_kind();
    let mut out = Vec::new();
    if bits & ALLOC != 0 {
        out.push(Violation::Alloc);
    }
    if bits & LOCK != 0 {
        out.push(Violation::Lock);
    }
    if bits & IO != 0 {
        out.push(Violation::Io);
    }
    out
}

pub fn saw(kind: Violation) -> bool {
    violations().contains(&kind)
}

pub fn with_realtime<R>(body: impl FnOnce() -> R) -> R {
    if let Some(enter) = dylib_enter() {
        unsafe { enter() };
    }
    IN_SECTION.with(|flag| flag.set(true));
    let result = body();
    IN_SECTION.with(|flag| flag.set(false));
    if let Some(exit) = dylib_exit() {
        unsafe { exit() };
    }
    result
}

pub fn assert_section_catches(kind: Violation, body: impl FnOnce()) {
    if env::var_os("LLAMP_RT_CHILD").is_none() {
        let mode = match kind {
            Violation::Alloc => "alloc",
            Violation::Lock => "lock",
            Violation::Io => "io",
        };
        let status = spawn_child(mode);
        assert!(status, "realtime hook did not catch {kind:?}");
        return;
    }
    clear_violations();
    with_realtime(body);
    assert!(saw(kind), "realtime hook did not record {kind:?}; saw {:?}", violations());
}

pub fn assert_section_clean(body: impl FnOnce()) {
    if env::var_os("LLAMP_RT_CHILD").is_none() {
        let status = spawn_child("clean");
        assert!(status, "callback or FFT hop allocated, locked, or did I/O");
        return;
    }
    clear_violations();
    with_realtime(body);
    assert!(violations().is_empty(), "saw {:?}", violations());
}

fn spawn_child(mode: &str) -> bool {
    let exe = env::current_exe().expect("test exe");
    let name = std::thread::current().name().unwrap_or("").to_string();
    let mut cmd = Command::new(&exe);
    cmd.env("LLAMP_RT_CHILD", mode)
        .args(["--exact", &name, "--test-threads=1", "--nocapture"]);
    if mode != "alloc" {
        cmd.env("DYLD_INSERT_LIBRARIES", hook_dylib());
    }
    let status = cmd.output().expect("spawn realtime probe");
    if !status.status.success() {
        let stdout = String::from_utf8_lossy(&status.stdout);
        let stderr = String::from_utf8_lossy(&status.stderr);
        panic!("realtime child failed ({}):\n{stdout}\n{stderr}", status.status);
    }
    true
}

fn hook_dylib() -> PathBuf {
    let exe = env::current_exe().expect("test exe");
    let path = exe.parent().unwrap().join("libllamp_rt_hook.dylib");
    if !path.exists() {
        let src = env::temp_dir().join("llamp_rt_hook.c");
        fs::write(&src, HOOK_C).expect("write hook source");
        let status = Command::new("clang")
            .args(["-shared", "-fPIC", "-O2", "-o"])
            .arg(&path)
            .arg(&src)
            .status()
            .expect("clang");
        assert!(status.success(), "failed to build the realtime hook dylib");
    }
    path
}

type VoidFn = unsafe extern "C" fn();
type KindFn = unsafe extern "C" fn() -> i32;

fn dylib_enter() -> Option<VoidFn> {
    symbol(c"llamp_rt_enter")
}

fn dylib_exit() -> Option<VoidFn> {
    symbol(c"llamp_rt_exit")
}

fn dylib_reset() -> Option<VoidFn> {
    symbol(c"llamp_rt_reset")
}

fn dylib_kind() -> u64 {
    let Some(kind) = symbol::<KindFn>(c"llamp_rt_kind") else {
        return 0;
    };
    unsafe { kind() as u64 }
}

fn symbol<T>(name: &std::ffi::CStr) -> Option<T> {
    let ptr = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute_copy(&ptr) })
    }
}

const HOOK_C: &str = r#"
#include <dlfcn.h>
#include <pthread.h>
#include <stdatomic.h>
#include <unistd.h>
#include <fcntl.h>
#include <stdio.h>

static atomic_int in_rt = 0;
static atomic_int kind = 0;

void llamp_rt_enter(void) { atomic_store(&in_rt, 1); }
void llamp_rt_exit(void) { atomic_store(&in_rt, 0); }
void llamp_rt_reset(void) { atomic_store(&kind, 0); }
int llamp_rt_kind(void) { return atomic_load(&kind); }

static void note(int bit) {
    if (atomic_load(&in_rt)) atomic_fetch_or(&kind, bit);
}

/* The interposing image is not itself interposed, so these calls reach libSystem. */
static int hooked_lock(pthread_mutex_t *m) {
    note(2);
    return pthread_mutex_lock(m);
}
static int hooked_trylock(pthread_mutex_t *m) {
    note(2);
    return pthread_mutex_trylock(m);
}
static int hooked_open(const char *path, int flags, mode_t mode) {
    note(4);
    return open(path, flags, mode);
}
static int hooked_openat(int fd, const char *path, int flags, mode_t mode) {
    note(4);
    return openat(fd, path, flags, mode);
}

#define DYLD_INTERPOSE(_repl, _orig) \
    __attribute__((used)) static struct { const void *r; const void *o; } _interpose_##_orig \
    __attribute__((section("__DATA,__interpose"))) = { (const void *)_repl, (const void *)_orig };

DYLD_INTERPOSE(hooked_lock, pthread_mutex_lock)
DYLD_INTERPOSE(hooked_trylock, pthread_mutex_trylock)
DYLD_INTERPOSE(hooked_open, open)
DYLD_INTERPOSE(hooked_openat, openat)
"#;
