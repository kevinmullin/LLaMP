use std::alloc::{alloc, dealloc, Layout};
use std::ffi::{c_char, CStr, CString};
use std::path::Path;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use llamp_core::{PlaybackSnapshot, Session};
use llamp_plugin_api::{MediaSource, SourceItem};
use llamp_plugin_host::Registry;
use llamp_source_local::LocalSource;

mod vis;
pub use vis::*;

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

pub(crate) fn skin_slot() -> &'static Mutex<SkinSlot> {
    static SKIN: OnceLock<Mutex<SkinSlot>> = OnceLock::new();
    SKIN.get_or_init(|| Mutex::new(SkinSlot::new()))
}

pub(crate) fn session() -> &'static Session {
    static SESSION: OnceLock<Session> = OnceLock::new();
    SESSION.get_or_init(Session::new)
}

fn label_ptr(id: u32) -> *const c_char {
    static LABELS: OnceLock<Vec<CString>> = OnceLock::new();
    let labels = LABELS.get_or_init(|| {
        llamp_skin::controls()
            .iter()
            .map(|control| {
                CString::new(llamp_skin::control_label(*control)).expect("label has no NUL")
            })
            .collect()
    });
    labels
        .get(id as usize)
        .map(|label| label.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Main window size in skin pixels. The shell does not hard-code this.
#[repr(C)]
pub struct LlampSize {
    pub width: u32,
    pub height: u32,
}

#[no_mangle]
pub extern "C" fn llamp_main_size() -> LlampSize {
    LlampSize {
        width: llamp_skin::MAIN_WIDTH,
        height: llamp_skin::MAIN_HEIGHT,
    }
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
        Err(_) => {
            return LlampImage {
                data: std::ptr::null_mut(),
                width: 0,
                height: 0,
                len: 0,
            }
        }
    };
    let Some(skin) = slot.current() else {
        return LlampImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
            len: 0,
        };
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
    let hop = llamp_audio::analysis::hop_snapshot();
    let pane = llamp_skin::control_rect(llamp_skin::Control::VisPane);
    let mut bands = [0.0f32; 32];
    let band_n = llamp_audio::analysis::log_bands(
        &hop.bins_db,
        pane.w,
        llamp_skin::VIS_BAR_W,
        &mut bands,
    );
    let time = format_time(&snap);
    let title = String::from_utf8_lossy(&snap.title[..snap.title_len as usize]).into_owned();
    let slot = match skin_slot().lock() {
        Ok(slot) => slot,
        Err(_) => {
            return LlampImage {
                data: std::ptr::null_mut(),
                width: 0,
                height: 0,
                len: 0,
            }
        }
    };
    let Some(skin) = slot.current() else {
        return LlampImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
            len: 0,
        };
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
            kbps: snap.kbps,
            khz: (snap.sample_rate / 1000) as u16,
            spectrum: if snap.vis_mode != 0 && snap.produces_pcm != 0 {
                Some(&bands[..band_n])
            } else {
                None
            },
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
    let Ok(slot) = skin_slot().lock() else {
        return 0;
    };
    let Some(skin) = slot.current() else { return 0 };
    polygons(skin, mode).len() as u32
}

#[no_mangle]
pub extern "C" fn llamp_region_point_count(mode: u32, polygon: u32) -> u32 {
    let Ok(slot) = skin_slot().lock() else {
        return 0;
    };
    let Some(skin) = slot.current() else { return 0 };
    polygons(skin, mode)
        .get(polygon as usize)
        .map(|poly| poly.points.len() as u32)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn llamp_region_point(mode: u32, polygon: u32, index: u32) -> LlampPoint {
    let Ok(slot) = skin_slot().lock() else {
        return LlampPoint { x: 0, y: 0 };
    };
    let Some(skin) = slot.current() else {
        return LlampPoint { x: 0, y: 0 };
    };
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
    pub produces_pcm: u8,
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
pub extern "C" fn llamp_session_configure(
    sample_rate: u32,
    frames: u64,
    channels: u16,
    title: *const c_char,
) {
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
/// Oscilloscope stays the vis mode (0). Spectrum uses the analysis hop when the mode is 1.
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
    Some(
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}

pub(crate) fn image_from(rgba: Vec<u8>, width: u32, height: u32) -> LlampImage {
    let len = rgba.len();
    let data = Box::into_raw(rgba.into_boxed_slice()) as *mut u8;
    LlampImage {
        data,
        width,
        height,
        len,
    }
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
        produces_pcm: snap.produces_pcm,
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
    LlampSize {
        width: llamp_skin::EQ_WIDTH,
        height: llamp_skin::EQ_HEIGHT,
    }
}

#[no_mangle]
pub extern "C" fn llamp_eq_control_count() -> u32 {
    llamp_skin::eq_controls().len() as u32
}

#[no_mangle]
pub extern "C" fn llamp_eq_control_at(index: u32) -> LlampControl {
    let controls = llamp_skin::eq_controls();
    let Some(control) = controls.get(index as usize) else {
        return LlampControl {
            id: 0,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            label: std::ptr::null(),
        };
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
    let Some(name) = cstr_path(name) else {
        return LLAMP_ERR_INVALID;
    };
    match session().with_eq(|eq| eq.save_preset(&name)) {
        Ok(Ok(())) => LLAMP_OK,
        _ => LLAMP_ERR_INVALID,
    }
}

#[no_mangle]
pub extern "C" fn llamp_eq_load_preset(name: *const c_char) -> i32 {
    let Some(name) = cstr_path(name) else {
        return LLAMP_ERR_INVALID;
    };
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
    session()
        .with_eq(|eq| eq.preset_names().len() as u32)
        .unwrap_or(0)
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
    let Some(name) = name else {
        return LLAMP_ERR_INVALID;
    };
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
    let Some(path) = cstr_path(path) else {
        return LLAMP_ERR_INVALID;
    };
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
    let rgba = match skin_slot().lock() {
        Ok(slot) => llamp_skin::blit_eq_skin(slot.current(), &paint),
        Err(_) => llamp_skin::blit_eq(&paint),
    };
    image_from(rgba, llamp_skin::EQ_WIDTH, llamp_skin::EQ_HEIGHT)
}

#[no_mangle]
pub extern "C" fn llamp_playlist_blit() -> LlampImage {
    let empty = || LlampImage {
        data: std::ptr::null_mut(),
        width: 0,
        height: 0,
        len: 0,
    };
    let Ok(window) = playlist_window().lock() else {
        return empty();
    };
    let (w, h) = window.size();
    let Ok(slot) = skin_slot().lock() else {
        return empty();
    };
    let Some(skin) = slot.current() else {
        return empty();
    };
    image_from(
        llamp_skin::blit_playlist(skin, w as u32, h as u32),
        w as u32,
        h as u32,
    )
}

#[no_mangle]
pub extern "C" fn llamp_gen_blit() -> LlampImage {
    let empty = || LlampImage {
        data: std::ptr::null_mut(),
        width: 0,
        height: 0,
        len: 0,
    };
    let Ok(slot) = skin_slot().lock() else {
        return empty();
    };
    let Some(skin) = slot.current() else {
        return empty();
    };
    image_from(llamp_skin::blit_gen(skin, 275, 116), 275, 116)
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
        return LlampDock {
            main_x: 0,
            main_y: 0,
            main_w: 0,
            main_h: 0,
            eq_x: 0,
            eq_y: 0,
            eq_w: 0,
            eq_h: 0,
            docked: 0,
            group_on_top: 0,
        };
    };
    session()
        .with_eq(|eq| {
            let moved = eq.drag(which, dx, dy);
            dock_from(moved, eq.window_on_top(which, session_on_top()))
        })
        .unwrap_or(LlampDock {
            main_x: 0,
            main_y: 0,
            main_w: 0,
            main_h: 0,
            eq_x: 0,
            eq_y: 0,
            eq_w: 0,
            eq_h: 0,
            docked: 0,
            group_on_top: 0,
        })
}

#[no_mangle]
pub extern "C" fn llamp_eq_end_drag() -> LlampDock {
    session()
        .with_eq(|eq| {
            let moved = eq.end_drag();
            dock_from(
                moved,
                eq.window_on_top(llamp_core::Which::Eq, session_on_top()),
            )
        })
        .unwrap_or(LlampDock {
            main_x: 0,
            main_y: 0,
            main_w: 0,
            main_h: 0,
            eq_x: 0,
            eq_y: 0,
            eq_w: 0,
            eq_h: 0,
            docked: 0,
            group_on_top: 0,
        })
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
    llamp_core::Frame {
        x: frame.x,
        y: frame.y,
        w: frame.w,
        h: frame.h,
    }
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

fn playlist_window() -> &'static Mutex<llamp_core::PlaylistWindow> {
    static WINDOW: OnceLock<Mutex<llamp_core::PlaylistWindow>> = OnceLock::new();
    WINDOW.get_or_init(|| Mutex::new(llamp_core::PlaylistWindow::new()))
}

struct LibrarySlot {
    source: Option<std::sync::Arc<LocalSource>>,
    hits: Vec<SourceItem>,
}

fn library_slot() -> &'static Mutex<LibrarySlot> {
    static SLOT: OnceLock<Mutex<LibrarySlot>> = OnceLock::new();
    SLOT.get_or_init(|| {
        Mutex::new(LibrarySlot {
            source: None,
            hits: Vec::new(),
        })
    })
}

fn plugin_registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::new()))
}

#[no_mangle]
pub extern "C" fn llamp_plugin_count() -> u32 {
    plugin_registry()
        .lock()
        .map(|reg| reg.count() as u32)
        .unwrap_or(0)
}

/// Writes a NUL-terminated plugin id.
#[no_mangle]
pub extern "C" fn llamp_plugin_id(index: u32, out: *mut c_char, len: usize) -> i32 {
    if out.is_null() || len == 0 {
        return LLAMP_ERR_INVALID;
    }
    let Ok(registry) = plugin_registry().lock() else {
        return LLAMP_ERR_INVALID;
    };
    let Some(id) = registry.id_at(index as usize) else {
        return LLAMP_ERR_INVALID;
    };
    let bytes = id.as_bytes();
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
pub extern "C" fn llamp_plugin_enable(id: *const c_char) -> i32 {
    let Some(id) = cstr_path(id) else {
        return LLAMP_ERR_INVALID;
    };
    let Ok(mut registry) = plugin_registry().lock() else {
        return LLAMP_ERR_INVALID;
    };
    if registry.enable(&id).is_ok() {
        LLAMP_OK
    } else {
        LLAMP_ERR_INVALID
    }
}

#[no_mangle]
pub extern "C" fn llamp_plugin_disable(id: *const c_char) -> i32 {
    let Some(id) = cstr_path(id) else {
        return LLAMP_ERR_INVALID;
    };
    let Ok(mut registry) = plugin_registry().lock() else {
        return LLAMP_ERR_INVALID;
    };
    if registry.disable(&id).is_ok() {
        LLAMP_OK
    } else {
        LLAMP_ERR_INVALID
    }
}

/// Writes the recorded refusal reason. Empty if the plugin was not refused.
#[no_mangle]
pub extern "C" fn llamp_plugin_refused_reason(
    id: *const c_char,
    out: *mut c_char,
    len: usize,
) -> i32 {
    if out.is_null() || len == 0 {
        return LLAMP_ERR_INVALID;
    }
    let Some(id) = cstr_path(id) else {
        return LLAMP_ERR_INVALID;
    };
    let Ok(registry) = plugin_registry().lock() else {
        return LLAMP_ERR_INVALID;
    };
    let reason = registry.refused_reason(&id).unwrap_or("");
    let bytes = reason.as_bytes();
    if bytes.len() + 1 > len {
        return LLAMP_ERR_INVALID;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast(), bytes.len());
        *out.add(bytes.len()) = 0;
    }
    LLAMP_OK
}

/// Opens the library database. Music stays at granted paths.
#[no_mangle]
pub extern "C" fn llamp_library_open(path: *const c_char) -> i32 {
    let Some(path) = cstr_path(path) else {
        return LLAMP_ERR_INVALID;
    };
    let Ok(source) = LocalSource::open(Path::new(&path)) else {
        return LLAMP_ERR_INVALID;
    };
    let source = std::sync::Arc::new(source);
    if let Ok(mut registry) = plugin_registry().lock() {
        if let Ok(manifest) = llamp_plugin_api::Manifest::parse(include_str!(
            "../../../plugins/llamp-source-local/llamp-plugin.json"
        )) {
            let _ = registry.register_native(manifest, Some(source.clone()));
        }
    }
    let Ok(mut slot) = library_slot().lock() else {
        return LLAMP_ERR_INVALID;
    };
    slot.source = Some(source);
    slot.hits.clear();
    LLAMP_OK
}

#[no_mangle]
pub extern "C" fn llamp_library_grant(dir: *const c_char) -> i32 {
    let Some(dir) = cstr_path(dir) else {
        return LLAMP_ERR_INVALID;
    };
    let Ok(slot) = library_slot().lock() else {
        return LLAMP_ERR_INVALID;
    };
    let Some(source) = slot.source.as_ref() else {
        return LLAMP_ERR_INVALID;
    };
    match source.grant_folder(Path::new(&dir)) {
        Ok(n) => i32::try_from(n).unwrap_or(LLAMP_ERR_INVALID),
        Err(_) => LLAMP_ERR_INVALID,
    }
}

#[no_mangle]
pub extern "C" fn llamp_library_search(query: *const c_char) -> u32 {
    let query = cstr(query);
    let Ok(mut slot) = library_slot().lock() else {
        return 0;
    };
    let Some(source) = slot.source.as_ref() else {
        return 0;
    };
    let Ok(hits) = MediaSource::search(source.as_ref(), &query) else {
        slot.hits.clear();
        return 0;
    };
    slot.hits = hits;
    slot.hits.len() as u32
}

/// Writes a NUL-terminated granted path. The caller does not get a copy of the audio.
#[no_mangle]
pub extern "C" fn llamp_library_hit_path(index: u32, out: *mut c_char, len: usize) -> i32 {
    if out.is_null() || len == 0 {
        return LLAMP_ERR_INVALID;
    }
    let Ok(slot) = library_slot().lock() else {
        return LLAMP_ERR_INVALID;
    };
    let Some(hit) = slot.hits.get(index as usize) else {
        return LLAMP_ERR_INVALID;
    };
    let bytes = hit.label.as_bytes();
    if bytes.len() + 1 > len {
        return LLAMP_ERR_INVALID;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast(), bytes.len());
        *out.add(bytes.len()) = 0;
    }
    LLAMP_OK
}

/// Enqueues a search hit onto `playlist_items` and the playlist window. Does not copy the file.
#[no_mangle]
pub extern "C" fn llamp_library_enqueue_hit(index: u32) -> i32 {
    let Ok(slot) = library_slot().lock() else {
        return LLAMP_ERR_INVALID;
    };
    let Some(hit) = slot.hits.get(index as usize).cloned() else {
        return LLAMP_ERR_INVALID;
    };
    let Some(source) = slot.source.as_ref() else {
        return LLAMP_ERR_INVALID;
    };
    if source.enqueue(Path::new(&hit.id)).is_err() {
        return LLAMP_ERR_INVALID;
    }
    if let Ok(resolved) = MediaSource::resolve(source.as_ref(), &hit.id) {
        session().apply_source_flags(resolved.flags);
    }
    let path = std::path::PathBuf::from(&hit.id);
    drop(slot);
    let Ok(mut window) = playlist_window().lock() else {
        return LLAMP_ERR_INVALID;
    };
    window.enqueue(path.to_string_lossy().as_ref());
    LLAMP_OK
}

#[no_mangle]
pub extern "C" fn llamp_playlist_enqueue(path: *const c_char) -> i32 {
    let Some(path) = cstr_path(path) else {
        return LLAMP_ERR_INVALID;
    };
    let Ok(mut window) = playlist_window().lock() else {
        return LLAMP_ERR_INVALID;
    };
    window.enqueue(&path);
    LLAMP_OK
}

fn dock_group() -> &'static Mutex<llamp_core::DockGroup> {
    static GROUP: OnceLock<Mutex<llamp_core::DockGroup>> = OnceLock::new();
    GROUP.get_or_init(|| Mutex::new(llamp_core::DockGroup::new()))
}

#[repr(C)]
pub struct LlampGroup {
    pub main: LlampFrame,
    pub eq: LlampFrame,
    pub playlist: LlampFrame,
    pub browser: LlampFrame,
    pub vis: LlampFrame,
    pub main_docked: u8,
    pub eq_docked: u8,
    pub playlist_docked: u8,
    pub browser_docked: u8,
    pub vis_docked: u8,
}

/// Minimum playlist size. Resize is 25×29 from this. A mid-step size is rejected.
#[no_mangle]
pub extern "C" fn llamp_playlist_propose_size(w: i32, h: i32) -> i32 {
    let Ok(mut window) = playlist_window().lock() else {
        return LLAMP_ERR_INVALID;
    };
    if window.propose_size(w, h).is_err() {
        return LLAMP_ERR_INVALID;
    }
    LLAMP_OK
}

#[no_mangle]
pub extern "C" fn llamp_playlist_size() -> LlampSize {
    let Ok(window) = playlist_window().lock() else {
        return LlampSize {
            width: 275,
            height: 116,
        };
    };
    let (w, h) = window.size();
    LlampSize {
        width: w as u32,
        height: h as u32,
    }
}

#[no_mangle]
pub extern "C" fn llamp_playlist_visible_count(len: u32, scroll: u32) -> u32 {
    let Ok(window) = playlist_window().lock() else {
        return 0;
    };
    let range = window.visible_range(len as usize, scroll as usize);
    (range.end - range.start) as u32
}

/// Row index, or -1. Arithmetic on the visible window. Does not walk `len`.
#[no_mangle]
pub extern "C" fn llamp_playlist_hit_row(y: i32, scroll: u32, len: u32) -> i32 {
    let Ok(window) = playlist_window().lock() else {
        return -1;
    };
    window
        .hit_row(y, scroll as usize, len as usize)
        .map(|index| index as i32)
        .unwrap_or(-1)
}

/// 0 = every glyph is `text.bmp`. 1 = CoreText. Any missing glyph promotes the string.
#[no_mangle]
pub extern "C" fn llamp_playlist_row_font(text: *const c_char) -> u32 {
    match row_mode(&cstr(text)) {
        llamp_core::RowFont::Bitmap => 0,
        llamp_core::RowFont::CoreText => 1,
    }
}

/// 0 = every visible row is `text.bmp`. 1 = the whole list is CoreText.
/// `rows` is `count` NUL-terminated UTF-8 strings.
#[no_mangle]
pub extern "C" fn llamp_playlist_list_font(rows: *const *const c_char, count: u32) -> u32 {
    if rows.is_null() && count > 0 {
        return 1;
    }
    let mut texts = Vec::new();
    for index in 0..count as usize {
        let ptr = unsafe { *rows.add(index) };
        texts.push(cstr(ptr));
    }
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    match llamp_core::list_font(refs, glyph_exists) {
        llamp_core::RowFont::Bitmap => 0,
        llamp_core::RowFont::CoreText => 1,
    }
}

/// 0 = `text.bmp`. 1 = CoreText. `scalar` is a Unicode code point.
#[no_mangle]
pub extern "C" fn llamp_playlist_char_font(scalar: u32) -> u32 {
    let Some(ch) = char::from_u32(scalar) else {
        return 1;
    };
    u32::from(matches!(
        llamp_core::glyph_font(ch, glyph_exists),
        llamp_core::RowFont::CoreText
    ))
}

#[no_mangle]
pub extern "C" fn llamp_browser_row_font(text: *const c_char) -> u32 {
    let text = cstr(text);
    u32::from(matches!(
        llamp_core::browser_row_font(&text),
        llamp_core::RowFont::CoreText
    ))
}

#[no_mangle]
pub extern "C" fn llamp_browser_size() -> LlampSize {
    let (w, h) = llamp_core::browser_size();
    LlampSize {
        width: w as u32,
        height: h as u32,
    }
}

fn browser_list() -> &'static Mutex<llamp_core::BrowserList> {
    static LIST: OnceLock<Mutex<llamp_core::BrowserList>> = OnceLock::new();
    LIST.get_or_init(|| Mutex::new(llamp_core::BrowserList::new()))
}

/// Loads granted paths into the browser list. Always CoreText. Does not copy audio.
#[no_mangle]
pub extern "C" fn llamp_browser_load_granted() -> u32 {
    let Ok(slot) = library_slot().lock() else {
        return 0;
    };
    let Some(source) = slot.source.as_ref() else {
        return 0;
    };
    let Ok(mut list) = browser_list().lock() else {
        return 0;
    };
    if list.load_granted(source.as_ref()).is_err() {
        return 0;
    }
    list.rows().len() as u32
}

#[no_mangle]
pub extern "C" fn llamp_browser_row_count() -> u32 {
    let Ok(list) = browser_list().lock() else {
        return 0;
    };
    list.rows().len() as u32
}

/// Writes a NUL-terminated granted path. The caller does not free it.
#[no_mangle]
pub extern "C" fn llamp_browser_row_path(index: u32, out: *mut c_char, len: usize) -> i32 {
    if out.is_null() || len == 0 {
        return LLAMP_ERR_INVALID;
    }
    let Ok(list) = browser_list().lock() else {
        return LLAMP_ERR_INVALID;
    };
    let Some(path) = list.rows().get(index as usize) else {
        return LLAMP_ERR_INVALID;
    };
    let bytes = path.as_bytes();
    if bytes.len() + 1 > len {
        return LLAMP_ERR_INVALID;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast(), bytes.len());
        *out.add(bytes.len()) = 0;
    }
    LLAMP_OK
}

/// Bitmap row from the loaded atlas. Null data means the shell must use CoreText.
#[no_mangle]
pub extern "C" fn llamp_text_row_blit(text: *const c_char) -> LlampImage {
    let text = cstr(text);
    if row_mode(&text) != llamp_core::RowFont::Bitmap {
        return LlampImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
            len: 0,
        };
    }
    let Some(rgba) = blit_bitmap_row(&text) else {
        return LlampImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
            len: 0,
        };
    };
    let width = text.chars().count() as u32 * llamp_core::CELL_W as u32;
    image_from(rgba, width, llamp_core::ROW_H as u32)
}

/// One 5×7 `text.bmp` cell. Null data means the shell must use CoreText for this scalar.
#[no_mangle]
pub extern "C" fn llamp_text_char_blit(scalar: u32) -> LlampImage {
    let Some(ch) = char::from_u32(scalar) else {
        return LlampImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
            len: 0,
        };
    };
    if !glyph_exists(ch) {
        return LlampImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
            len: 0,
        };
    }
    let Some(rgba) = blit_bitmap_char(ch) else {
        return LlampImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
            len: 0,
        };
    };
    image_from(rgba, llamp_core::CELL_W as u32, llamp_core::ROW_H as u32)
}

#[no_mangle]
pub extern "C" fn llamp_playlist_button_at(index: u32) -> LlampControl {
    let Ok(window) = playlist_window().lock() else {
        return LlampControl {
            id: index,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            label: std::ptr::null(),
        };
    };
    let Some((label, x, y, w, h)) = window.menu_button(index as usize) else {
        return LlampControl {
            id: index,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            label: std::ptr::null(),
        };
    };
    LlampControl {
        id: index,
        x: x as u32,
        y: y as u32,
        w: w as u32,
        h: h as u32,
        label: menu_label(label),
    }
}

#[no_mangle]
pub extern "C" fn llamp_group_set_frame(which: u32, frame: LlampFrame) {
    let Ok(mut group) = dock_group().lock() else {
        return;
    };
    let Some(pane) = pane_from(which) else {
        return;
    };
    group.set_frame(pane, frame.x, frame.y, frame.w, frame.h);
}

#[no_mangle]
pub extern "C" fn llamp_group_reset() {
    if let Ok(mut group) = dock_group().lock() {
        *group = llamp_core::DockGroup::new();
    }
}

#[no_mangle]
pub extern "C" fn llamp_group_begin_drag() {
    if let Ok(mut group) = dock_group().lock() {
        group.begin_drag();
    }
}

#[no_mangle]
pub extern "C" fn llamp_group_drag(which: u32, dx: i32, dy: i32) -> LlampGroup {
    let Ok(mut group) = dock_group().lock() else {
        return empty_group();
    };
    let Some(pane) = pane_from(which) else {
        return empty_group();
    };
    group_from(group.drag(pane, dx, dy))
}

#[no_mangle]
pub extern "C" fn llamp_group_end_drag() -> LlampGroup {
    let Ok(mut group) = dock_group().lock() else {
        return empty_group();
    };
    group_from(group.end_drag())
}

fn pane_from(which: u32) -> Option<llamp_core::Pane> {
    match which {
        0 => Some(llamp_core::Pane::Main),
        1 => Some(llamp_core::Pane::Eq),
        2 => Some(llamp_core::Pane::Playlist),
        3 => Some(llamp_core::Pane::Browser),
        4 => Some(llamp_core::Pane::Vis),
        _ => None,
    }
}

fn group_from(moved: llamp_core::GroupMove) -> LlampGroup {
    let frame = |pane| {
        let item = moved.frame(pane);
        LlampFrame {
            x: item.x,
            y: item.y,
            w: item.w,
            h: item.h,
        }
    };
    LlampGroup {
        main: frame(llamp_core::Pane::Main),
        eq: frame(llamp_core::Pane::Eq),
        playlist: frame(llamp_core::Pane::Playlist),
        browser: frame(llamp_core::Pane::Browser),
        vis: frame(llamp_core::Pane::Vis),
        main_docked: u8::from(moved.docked(llamp_core::Pane::Main)),
        eq_docked: u8::from(moved.docked(llamp_core::Pane::Eq)),
        playlist_docked: u8::from(moved.docked(llamp_core::Pane::Playlist)),
        browser_docked: u8::from(moved.docked(llamp_core::Pane::Browser)),
        vis_docked: u8::from(moved.docked(llamp_core::Pane::Vis)),
    }
}

fn empty_group() -> LlampGroup {
    let zero = || LlampFrame {
        x: 0,
        y: 0,
        w: 0,
        h: 0,
    };
    LlampGroup {
        main: zero(),
        eq: zero(),
        playlist: zero(),
        browser: zero(),
        vis: zero(),
        main_docked: 0,
        eq_docked: 0,
        playlist_docked: 0,
        browser_docked: 0,
        vis_docked: 0,
    }
}

fn row_mode(text: &str) -> llamp_core::RowFont {
    llamp_core::row_font(text, glyph_exists)
}

fn glyph_exists(ch: char) -> bool {
    let Ok(slot) = skin_slot().lock() else {
        return fixture_glyph(ch);
    };
    let Some(skin) = slot.current() else {
        return fixture_glyph(ch);
    };
    if skin.glyphs.is_empty() {
        return fixture_glyph(ch);
    }
    skin.glyphs.iter().any(|(glyph, _)| *glyph == ch)
}

/// Fixture `text.bmp`: index 0 is U+0020, 95 cells, through `~`.
fn fixture_glyph(ch: char) -> bool {
    (' '..='~').contains(&ch)
}

fn blit_bitmap_row(text: &str) -> Option<Vec<u8>> {
    let slot = skin_slot().lock().ok()?;
    let skin = slot.current()?;
    let width = text.chars().count() as u32 * llamp_core::CELL_W as u32;
    let height = llamp_core::ROW_H as u32;
    let mut out = vec![0u8; (width * height * 4) as usize];
    for (index, ch) in text.chars().enumerate() {
        let (_, rect) = skin.glyphs.iter().find(|(glyph, _)| *glyph == ch)?;
        copy_atlas(
            &mut out,
            width,
            index as u32 * llamp_core::CELL_W as u32,
            skin,
            *rect,
        );
    }
    Some(out)
}

fn blit_bitmap_char(ch: char) -> Option<Vec<u8>> {
    let slot = skin_slot().lock().ok()?;
    let skin = slot.current()?;
    let (_, rect) = skin.glyphs.iter().find(|(glyph, _)| *glyph == ch)?;
    let mut out = vec![0u8; (llamp_core::CELL_W * llamp_core::ROW_H * 4) as usize];
    copy_atlas(&mut out, llamp_core::CELL_W as u32, 0, skin, *rect);
    Some(out)
}

fn copy_atlas(
    out: &mut [u8],
    dest_w: u32,
    dest_x: u32,
    skin: &llamp_skin::Skin,
    rect: llamp_skin::Rect,
) {
    let stride = skin.atlas_width;
    for y in 0..rect.h.min(7) {
        for x in 0..rect.w.min(5) {
            let src = ((rect.y + y) * stride + rect.x + x) as usize * 4;
            let dst = (y * dest_w + dest_x + x) as usize * 4;
            if src + 4 <= skin.atlas.len() && dst + 4 <= out.len() {
                out[dst..dst + 4].copy_from_slice(&skin.atlas[src..src + 4]);
            }
        }
    }
}

fn menu_label(label: &str) -> *const c_char {
    static LABELS: OnceLock<[CString; 5]> = OnceLock::new();
    let labels = LABELS.get_or_init(|| {
        ["Add", "Rem", "Sel", "Misc", "List"].map(|item| CString::new(item).expect("menu label"))
    });
    labels
        .iter()
        .find(|item| item.as_bytes() == label.as_bytes())
        .map(|item| item.as_ptr())
        .unwrap_or(std::ptr::null())
}

fn cstr(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

fn cstr_owned(title: *const c_char) -> Vec<u8> {
    if title.is_null() {
        return Vec::new();
    }
    unsafe { CStr::from_ptr(title) }.to_bytes().to_vec()
}
