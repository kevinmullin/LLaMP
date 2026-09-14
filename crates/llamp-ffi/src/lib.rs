use std::alloc::{alloc, dealloc, Layout};
use std::ffi::{c_char, CStr, CString};
use std::path::Path;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use llamp_core::{PlaybackSnapshot, Session};

const _: () = assert!(llamp_core::TITLE_CAP == 256);
use llamp_skin::SkinSlot;

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

fn skin_slot() -> &'static Mutex<SkinSlot> {
    static SKIN: OnceLock<Mutex<SkinSlot>> = OnceLock::new();
    SKIN.get_or_init(|| Mutex::new(SkinSlot::new()))
}

fn session() -> &'static Session {
    static SESSION: OnceLock<Session> = OnceLock::new();
    SESSION.get_or_init(Session::new)
}

fn label_ptr(id: u32) -> *const c_char {
    static LABELS: OnceLock<Vec<CString>> = OnceLock::new();
    let labels = LABELS.get_or_init(|| {
        llamp_skin::controls()
            .iter()
            .map(|control| CString::new(llamp_skin::control_label(*control)).expect("label has no NUL"))
            .collect()
    });
    labels.get(id as usize).map(|label| label.as_ptr()).unwrap_or(std::ptr::null())
}

/// Main window size in skin pixels. The shell does not hard-code this.
#[repr(C)]
pub struct LlampSize {
    pub width: u32,
    pub height: u32,
}

#[no_mangle]
pub extern "C" fn llamp_main_size() -> LlampSize {
    LlampSize { width: llamp_skin::MAIN_WIDTH, height: llamp_skin::MAIN_HEIGHT }
}

/// Shade height in skin pixels.
#[no_mangle]
pub extern "C" fn llamp_shade_height() -> u32 {
    llamp_skin::SHADE_HEIGHT
}

/// RGBA image owned by the core. Free `data` with `llamp_image_free` exactly once.
///
/// A null `data` means no image. `len` is `width * height * 4`.
#[repr(C)]
pub struct LlampImage {
    pub data: *mut u8,
    pub width: u32,
    pub height: u32,
    pub len: usize,
}

/// Loads a `.wsz`. The shell does not parse it. Returns `LLAMP_OK` or `LLAMP_ERR_INVALID`.
#[no_mangle]
pub extern "C" fn llamp_skin_load(bytes: *const u8, len: usize) -> i32 {
    if bytes.is_null() || len == 0 {
        return LLAMP_ERR_INVALID;
    }
    let slice = unsafe { std::slice::from_raw_parts(bytes, len) };
    let mut slot = match skin_slot().lock() {
        Ok(slot) => slot,
        Err(_) => return LLAMP_ERR_INVALID,
    };
    match slot.load_wsz(slice) {
        Ok(_) => LLAMP_OK,
        Err(_) => LLAMP_ERR_INVALID,
    }
}

/// Idle main-window blit. Matches the phase 2 PNG for the fixture skin.
///
/// The caller frees `data` with `llamp_image_free`.
#[no_mangle]
pub extern "C" fn llamp_skin_blit_main() -> LlampImage {
    let slot = match skin_slot().lock() {
        Ok(slot) => slot,
        Err(_) => return LlampImage { data: std::ptr::null_mut(), width: 0, height: 0, len: 0 },
    };
    let Some(skin) = slot.current() else {
        return LlampImage { data: std::ptr::null_mut(), width: 0, height: 0, len: 0 };
    };
    let rgba = llamp_skin::blit_main(skin);
    image_from(rgba, llamp_skin::MAIN_WIDTH, llamp_skin::MAIN_HEIGHT)
}

/// Display blit. Polls playback, then stamps time and the marquee. Does not wait.
///
/// Idle session values are not applied here; the caller keeps `llamp_skin_blit_main`
/// until a poll is not the idle snapshot. The caller frees `data` with `llamp_image_free`.
#[no_mangle]
pub extern "C" fn llamp_skin_blit_display(marquee_skip: u32) -> LlampImage {
    let snap = session().poll();
    let pcm = llamp_audio::live::pcm_snapshot();
    let time = format_time(&snap);
    let title = String::from_utf8_lossy(&snap.title[..snap.title_len as usize]).into_owned();
    let slot = match skin_slot().lock() {
        Ok(slot) => slot,
        Err(_) => return LlampImage { data: std::ptr::null_mut(), width: 0, height: 0, len: 0 },
    };
    let Some(skin) = slot.current() else {
        return LlampImage { data: std::ptr::null_mut(), width: 0, height: 0, len: 0 };
    };
    let rgba = llamp_skin::blit_display(
        skin,
        llamp_skin::Display {
            time: &time,
            marquee: &title,
            marquee_skip: marquee_skip as usize,
            volume_ppm: snap.volume_ppm,
            balance_ppm: snap.balance_ppm,
            seek_ppm: seek_ppm(&snap),
            scope: scope_samples(&snap, &pcm),
        },
    );
    image_from(rgba, llamp_skin::MAIN_WIDTH, llamp_skin::MAIN_HEIGHT)
}

fn scope_samples<'a>(snap: &PlaybackSnapshot, pcm: &'a [f32]) -> Option<&'a [f32]> {
    if snap.vis_mode == 0 && snap.transport == 1 {
        Some(pcm)
    } else {
        None
    }
}

fn seek_ppm(snap: &PlaybackSnapshot) -> u16 {
    if snap.duration_frames == 0 {
        return 0;
    }
    ((snap.position_frames.saturating_mul(1000)) / snap.duration_frames).min(1000) as u16
}

fn format_time(snap: &PlaybackSnapshot) -> String {
    let rate = u64::from(snap.sample_rate.max(1));
    let frames = if snap.time_remaining == 0 {
        snap.position_frames
    } else {
        snap.duration_frames.saturating_sub(snap.position_frames)
    };
    let secs = frames / rate;
    let minutes = secs / 60;
    let seconds = secs % 60;
    if snap.time_remaining == 0 {
        format!("{minutes}:{seconds:02}")
    } else {
        format!("-{minutes}:{seconds:02}")
    }
}

/// Frees `data` from `llamp_skin_blit_main`. `len` is the returned length.
#[no_mangle]
pub extern "C" fn llamp_image_free(data: *mut u8, len: usize) {
    if data.is_null() || len == 0 {
        return;
    }
    unsafe { drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len))) };
}

#[repr(C)]
pub struct LlampControl {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Static string. The caller does not free it.
    pub label: *const c_char,
}

#[no_mangle]
pub extern "C" fn llamp_control_count() -> u32 {
    llamp_skin::controls().len() as u32
}

/// A null label means `index` is out of range.
#[no_mangle]
pub extern "C" fn llamp_control_at(index: u32) -> LlampControl {
    let Some(control) = llamp_skin::controls().get(index as usize).copied() else {
        return LlampControl {
            id: 0,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            label: std::ptr::null(),
        };
    };
    let rect = llamp_skin::control_rect(control);
    LlampControl {
        id: control as u32,
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
        label: label_ptr(control as u32),
    }
}

#[repr(C)]
pub struct LlampPoint {
    pub x: i32,
    pub y: i32,
}

/// `mode` 0 is the normal mask. `mode` 1 is the window-shade mask.
#[no_mangle]
pub extern "C" fn llamp_region_polygon_count(mode: u32) -> u32 {
    let Ok(slot) = skin_slot().lock() else { return 0 };
    let Some(skin) = slot.current() else { return 0 };
    polygons(skin, mode).len() as u32
}

#[no_mangle]
pub extern "C" fn llamp_region_point_count(mode: u32, polygon: u32) -> u32 {
    let Ok(slot) = skin_slot().lock() else { return 0 };
    let Some(skin) = slot.current() else { return 0 };
    polygons(skin, mode)
        .get(polygon as usize)
        .map(|poly| poly.points.len() as u32)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn llamp_region_point(mode: u32, polygon: u32, index: u32) -> LlampPoint {
    let Ok(slot) = skin_slot().lock() else { return LlampPoint { x: 0, y: 0 } };
    let Some(skin) = slot.current() else { return LlampPoint { x: 0, y: 0 } };
    polygons(skin, mode)
        .get(polygon as usize)
        .and_then(|poly| poly.points.get(index as usize).copied())
        .map(|(x, y)| LlampPoint { x, y })
        .unwrap_or(LlampPoint { x: 0, y: 0 })
}

/// Playback snapshot. Returned by value. The caller does not free it.
///
/// Does not wait. Takes no lock. The audio callback does not publish this
/// struct; it only increments a frame counter the poll reads.
#[repr(C)]
pub struct LlampPlayback {
    pub position_frames: u64,
    pub duration_frames: u64,
    pub sample_rate: u32,
    pub source_channels: u16,
    pub transport: u8,
    pub shuffle: u8,
    pub repeat_mode: u8,
    pub time_remaining: u8,
    pub vis_mode: u8,
    pub always_on_top: u8,
    pub double_size: u8,
    pub supports_eq: u8,
    pub volume_ppm: u16,
    pub balance_ppm: u16,
    pub title_len: u16,
    pub title: [u8; 256],
}

#[no_mangle]
pub extern "C" fn llamp_playback_poll() -> LlampPlayback {
    snapshot_to_c(session().poll())
}

/// Sets duration and title for a session that does not open a device.
/// `title` may be null. A missing NUL is truncated at `TITLE_CAP`.
#[no_mangle]
pub extern "C" fn llamp_session_configure(sample_rate: u32, frames: u64, channels: u16, title: *const c_char) {
    let owned = cstr_owned(title);
    session().configure(sample_rate, frames, channels, &owned);
}

#[no_mangle]
pub extern "C" fn llamp_transport_toggle_play() {
    session().toggle_play();
}

#[no_mangle]
pub extern "C" fn llamp_transport_seek_by(frames: i32) {
    session().seek_by(frames);
}

#[no_mangle]
pub extern "C" fn llamp_transport_press(id: u32) {
    session().press(id);
}

/// Seek (id 15) jumps to `duration * ppm / 1000`. Volume and balance are visual only and do not apply a gain law.
#[no_mangle]
pub extern "C" fn llamp_transport_set_slider(id: u32, ppm: u16) {
    session().set_slider(id, ppm);
}

/// Retains every file in `refs_dir` and loops `track` through the output.
/// Oscilloscope stays the vis mode (0). Spectrum bars are not drawn.
/// The callback still only `fetch_add`s. Returns `LLAMP_OK` or `LLAMP_ERR_INVALID`.
#[no_mangle]
pub extern "C" fn llamp_budget_prepare(track: *const c_char, refs_dir: *const c_char) -> i32 {
    let track = cstr_path(track);
    let refs_dir = cstr_path(refs_dir);
    let (Some(track), Some(refs_dir)) = (track, refs_dir) else {
        return LLAMP_ERR_INVALID;
    };
    if session().retain_references(Path::new(&refs_dir)).is_err() {
        return LLAMP_ERR_INVALID;
    }
    if session().start_loop(Path::new(&track)).is_err() {
        return LLAMP_ERR_INVALID;
    }
    LLAMP_OK
}

#[no_mangle]
pub extern "C" fn llamp_reference_count() -> u32 {
    session().reference_count() as u32
}

fn cstr_path(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned())
}

fn image_from(rgba: Vec<u8>, width: u32, height: u32) -> LlampImage {
    let len = rgba.len();
    let data = Box::into_raw(rgba.into_boxed_slice()) as *mut u8;
    LlampImage { data, width, height, len }
}

fn polygons(skin: &llamp_skin::Skin, mode: u32) -> &[llamp_skin::Polygon] {
    match mode {
        1 => &skin.regions.window_shade,
        2 => &skin.regions.equalizer,
        3 => &skin.regions.equalizer_ws,
        _ => &skin.regions.normal,
    }
}

fn snapshot_to_c(snap: PlaybackSnapshot) -> LlampPlayback {
    LlampPlayback {
        position_frames: snap.position_frames,
        duration_frames: snap.duration_frames,
        sample_rate: snap.sample_rate,
        source_channels: snap.source_channels,
        transport: snap.transport,
        shuffle: snap.shuffle,
        repeat_mode: snap.repeat_mode,
        time_remaining: snap.time_remaining,
        vis_mode: snap.vis_mode,
        always_on_top: snap.always_on_top,
        double_size: snap.double_size,
        supports_eq: snap.supports_eq,
        volume_ppm: snap.volume_ppm,
        balance_ppm: snap.balance_ppm,
        title_len: snap.title_len,
        title: snap.title,
    }
}

#[repr(C)]
pub struct LlampFrame {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[repr(C)]
pub struct LlampDock {
    pub main_x: i32,
    pub main_y: i32,
    pub main_w: i32,
    pub main_h: i32,
    pub eq_x: i32,
    pub eq_y: i32,
    pub eq_w: i32,
    pub eq_h: i32,
    pub docked: u8,
    pub group_on_top: u8,
}

/// Equalizer size in skin pixels. Same locked rectangle as the main window.
#[no_mangle]
pub extern "C" fn llamp_eq_size() -> LlampSize {
    LlampSize { width: llamp_skin::EQ_WIDTH, height: llamp_skin::EQ_HEIGHT }
}

#[no_mangle]
pub extern "C" fn llamp_eq_control_count() -> u32 {
    llamp_skin::eq_controls().len() as u32
}

#[no_mangle]
pub extern "C" fn llamp_eq_control_at(index: u32) -> LlampControl {
    let controls = llamp_skin::eq_controls();
    let Some(control) = controls.get(index as usize) else {
        return LlampControl { id: 0, x: 0, y: 0, w: 0, h: 0, label: std::ptr::null() };
    };
    LlampControl {
        id: control.id,
        x: control.rect.x,
        y: control.rect.y,
        w: control.rect.w,
        h: control.rect.h,
        label: eq_label(control.label),
    }
}

/// Magnitude at a band center after the slew settles. Bound to the window targets.
#[no_mangle]
pub extern "C" fn llamp_eq_center_db(band: u32) -> f32 {
    if band >= 10 {
        return f32::NAN;
    }
    let mut eq = llamp_audio::eq::Eq::new(48_000);
    eq.bind_ui_sliders();
    llamp_audio::set_eq_enabled(true);
    eq.impulse_center_db(band as usize, 8192)
}

/// Slider drag. `id` 6 is preamp, 7..16 are bands. `millidb` is thousandths of a dB.
/// Writes the atomic target. Does not redesign coefficients.
#[no_mangle]
pub extern "C" fn llamp_eq_drag(id: u32, millidb: i32) {
    let _ = session().with_eq(|eq| {
        if id == 6 {
            eq.drag_slider(true, 0, millidb);
        } else if (7..17).contains(&id) {
            eq.drag_slider(false, (id - 7) as usize, millidb);
        }
    });
}

#[no_mangle]
pub extern "C" fn llamp_eq_press(id: u32) {
    let _ = session().with_eq(|eq| match id {
        1 => eq.toggle_on(),
        2 => eq.toggle_auto(),
        _ => {}
    });
}

#[no_mangle]
pub extern "C" fn llamp_eq_save_preset(name: *const c_char) -> i32 {
    let Some(name) = cstr_path(name) else { return LLAMP_ERR_INVALID };
    match session().with_eq(|eq| eq.save_preset(&name)) {
        Ok(Ok(())) => LLAMP_OK,
        _ => LLAMP_ERR_INVALID,
    }
}

#[no_mangle]
pub extern "C" fn llamp_eq_load_preset(name: *const c_char) -> i32 {
    let Some(name) = cstr_path(name) else { return LLAMP_ERR_INVALID };
    match session().with_eq(|eq| eq.load_preset(&name)) {
        Ok(Ok(())) => LLAMP_OK,
        _ => LLAMP_ERR_INVALID,
    }
}

#[no_mangle]
pub extern "C" fn llamp_eq_save_autoload() -> i32 {
    match session().with_eq(|eq| eq.save_autoload()) {
        Ok(Ok(())) => LLAMP_OK,
        _ => LLAMP_ERR_INVALID,
    }
}

#[no_mangle]
pub extern "C" fn llamp_eq_save_default() -> i32 {
    match session().with_eq(|eq| eq.save_default()) {
        Ok(Ok(())) => LLAMP_OK,
        _ => LLAMP_ERR_INVALID,
    }
}

#[no_mangle]
pub extern "C" fn llamp_eq_preset_count() -> u32 {
    session().with_eq(|eq| eq.preset_names().len() as u32).unwrap_or(0)
}

/// Writes a NUL-terminated name into `out`. Returns `LLAMP_ERR_INVALID` if it does not fit.
#[no_mangle]
pub extern "C" fn llamp_eq_preset_name(index: u32, out: *mut c_char, len: usize) -> i32 {
    if out.is_null() || len == 0 {
        return LLAMP_ERR_INVALID;
    }
    let name = session()
        .with_eq(|eq| eq.preset_names().get(index as usize).cloned())
        .ok()
        .flatten();
    let Some(name) = name else { return LLAMP_ERR_INVALID };
    let bytes = name.as_bytes();
    if bytes.len() + 1 > len {
        return LLAMP_ERR_INVALID;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast(), bytes.len());
        *out.add(bytes.len()) = 0;
    }
    LLAMP_OK
}

#[no_mangle]
pub extern "C" fn llamp_eq_set_store(path: *const c_char) -> i32 {
    let Some(path) = cstr_path(path) else { return LLAMP_ERR_INVALID };
    match session().with_eq(|eq| eq.set_store(Path::new(&path))) {
        Ok(Ok(())) => LLAMP_OK,
        _ => LLAMP_ERR_INVALID,
    }
}

/// Static string. The caller does not free it.
#[no_mangle]
pub extern "C" fn llamp_eq_caption() -> *const c_char {
    let applies = session().poll().supports_eq != 0;
    if applies {
        static ON: &[u8] = b"Equalizer\0";
        ON.as_ptr().cast()
    } else {
        static OFF: &[u8] = b"Equalizer does not apply \xe2\x80\x94 playback is remote\0";
        OFF.as_ptr().cast()
    }
}

#[no_mangle]
pub extern "C" fn llamp_eq_blit() -> LlampImage {
    let snap = session().poll();
    let (on, auto_on) = session()
        .with_eq(|eq| (eq.on(), eq.auto()))
        .unwrap_or((false, false));
    let mut curve = vec![0.0f32; 64];
    if snap.supports_eq != 0 && on && snap.sample_rate > 0 {
        llamp_audio::eq::curve_db(snap.sample_rate, &mut curve);
    }
    let vis = skin_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.current().map(|skin| skin.vis_colors))
        .unwrap_or_else(llamp_skin::default_vis_colors);
    let paint = llamp_skin::EqPaint {
        on,
        auto_on,
        applies: snap.supports_eq != 0,
        preamp_db: llamp_audio::preamp_target_db(),
        bands: std::array::from_fn(llamp_audio::band_target_db),
        curve_db: curve,
        vis,
    };
    image_from(llamp_skin::blit_eq(&paint), llamp_skin::EQ_WIDTH, llamp_skin::EQ_HEIGHT)
}

#[no_mangle]
pub extern "C" fn llamp_eq_set_frames(main: LlampFrame, eq: LlampFrame) {
    let _ = session().with_eq(|window| {
        window.set_frames(frame_from(main), frame_from(eq));
    });
}

#[no_mangle]
pub extern "C" fn llamp_eq_begin_drag() {
    let _ = session().with_eq(|eq| eq.begin_drag());
}

#[no_mangle]
pub extern "C" fn llamp_eq_drag_window(which: u32, dx: i32, dy: i32) -> LlampDock {
    let Some(which) = llamp_core::Which::from_u32(which) else {
        return LlampDock { main_x: 0, main_y: 0, main_w: 0, main_h: 0, eq_x: 0, eq_y: 0, eq_w: 0, eq_h: 0, docked: 0, group_on_top: 0 };
    };
    session()
        .with_eq(|eq| {
            let moved = eq.drag(which, dx, dy);
            dock_from(moved, eq.window_on_top(which, session_on_top()))
        })
        .unwrap_or(LlampDock { main_x: 0, main_y: 0, main_w: 0, main_h: 0, eq_x: 0, eq_y: 0, eq_w: 0, eq_h: 0, docked: 0, group_on_top: 0 })
}

#[no_mangle]
pub extern "C" fn llamp_eq_end_drag() -> LlampDock {
    session()
        .with_eq(|eq| {
            let moved = eq.end_drag();
            dock_from(moved, eq.window_on_top(llamp_core::Which::Eq, session_on_top()))
        })
        .unwrap_or(LlampDock { main_x: 0, main_y: 0, main_w: 0, main_h: 0, eq_x: 0, eq_y: 0, eq_w: 0, eq_h: 0, docked: 0, group_on_top: 0 })
}

#[no_mangle]
pub extern "C" fn llamp_text_color() -> u32 {
    let rgb = skin_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.current().map(|skin| skin.playlist_colors.normal))
        .unwrap_or_else(|| llamp_skin::default_playlist_colors().normal);
    u32::from(rgb.r) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.b)
}

#[no_mangle]
pub extern "C" fn llamp_text_bg() -> u32 {
    let rgb = skin_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.current().map(|skin| skin.playlist_colors.normal_bg))
        .unwrap_or_else(|| llamp_skin::default_playlist_colors().normal_bg);
    u32::from(rgb.r) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.b)
}

fn session_on_top() -> bool {
    session().poll().always_on_top != 0
}

fn frame_from(frame: LlampFrame) -> llamp_core::Frame {
    llamp_core::Frame { x: frame.x, y: frame.y, w: frame.w, h: frame.h }
}

fn dock_from(dock: llamp_core::DockMove, main_on_top: bool) -> LlampDock {
    let _ = main_on_top;
    LlampDock {
        main_x: dock.main.x,
        main_y: dock.main.y,
        main_w: dock.main.w,
        main_h: dock.main.h,
        eq_x: dock.eq.x,
        eq_y: dock.eq.y,
        eq_w: dock.eq.w,
        eq_h: dock.eq.h,
        docked: u8::from(dock.docked),
        group_on_top: u8::from(main_on_top),
    }
}

fn eq_label(label: &str) -> *const c_char {
    static LABELS: OnceLock<Vec<CString>> = OnceLock::new();
    let labels = LABELS.get_or_init(|| {
        llamp_skin::eq_controls()
            .into_iter()
            .map(|control| CString::new(control.label).expect("eq label has no NUL"))
            .collect()
    });
    labels
        .iter()
        .find(|item| item.as_bytes() == label.as_bytes())
        .map(|item| item.as_ptr())
        .unwrap_or(std::ptr::null())
}

fn cstr_owned(title: *const c_char) -> Vec<u8> {
    if title.is_null() {
        return Vec::new();
    }
    unsafe { CStr::from_ptr(title) }.to_bytes().to_vec()
}
