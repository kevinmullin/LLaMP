//! Lyrics window ABI. Core chrome, shell CoreText, decoder clock.

use std::ffi::{c_char, CString};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use llamp_plugin_api::LyricsProvider;

use llamp_core::{lyrics_client_rect, lyrics_propose_size, line_rect, mark_rect, text_origin};
use llamp_library::{
    resolve_lyrics, LrclibConsent, LyricDoc, LyricKind, LyricQuery, LyricSource, OffsetSave,
};

use crate::{
    image_from, library_slot, session, skin_slot, cstr, cstr_path, LlampFrame, LlampImage, LlampSize,
    LLAMP_ERR_INVALID, LLAMP_OK,
};

struct LyricsState {
    doc: Option<LyricDoc>,
    path: Option<PathBuf>,
    user_offset_ms: i32,
    width: i32,
    height: i32,
    sidecar_written: u8,
    attribution: u8,
}

fn provider_slot() -> &'static Mutex<Option<Arc<dyn LyricsProvider>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<dyn LyricsProvider>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub(crate) fn lyrics_provider() -> Option<Arc<dyn LyricsProvider>> {
    provider_slot().lock().ok().and_then(|slot| slot.clone())
}

pub fn install_lyrics_provider(provider: Arc<dyn LyricsProvider>) {
    if let Ok(mut slot) = provider_slot().lock() {
        *slot = Some(provider);
    }
}

fn state() -> &'static Mutex<LyricsState> {
    static STATE: OnceLock<Mutex<LyricsState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(LyricsState {
            doc: None,
            path: None,
            user_offset_ms: 0,
            width: llamp_core::LYRICS_MIN_W,
            height: llamp_core::LYRICS_MIN_H,
            sidecar_written: 0,
            attribution: 0,
        })
    })
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_min_size() -> LlampSize {
    LlampSize {
        width: llamp_core::LYRICS_MIN_W as u32,
        height: llamp_core::LYRICS_MIN_H as u32,
    }
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_propose_size(width: i32, height: i32) -> LlampSize {
    let (w, h) = lyrics_propose_size(width, height);
    if let Ok(mut state) = state().lock() {
        state.width = w;
        state.height = h;
    }
    LlampSize {
        width: w as u32,
        height: h as u32,
    }
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_chrome_blit(width: u32, height: u32) -> LlampImage {
    let empty = || LlampImage {
        data: std::ptr::null_mut(),
        width: 0,
        height: 0,
        len: 0,
    };
    let w = width.max(llamp_core::LYRICS_MIN_W as u32);
    let h = height.max(llamp_core::LYRICS_MIN_H as u32);
    let Ok(slot) = skin_slot().lock() else {
        return empty();
    };
    let Some(skin) = slot.current() else {
        return empty();
    };
    image_from(llamp_skin::blit_gen(skin, w, h), w, h)
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_client_rect(width: i32, height: i32) -> LlampFrame {
    let rect = lyrics_client_rect(width, height);
    LlampFrame {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
    }
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_text_origin(scale: i32) -> LlampFrame {
    let (x, y) = text_origin(scale);
    LlampFrame {
        x,
        y,
        w: 0,
        h: 0,
    }
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_line_rect(index: i32, scroll_px: i32, scale: i32) -> LlampFrame {
    let (w, _) = state()
        .lock()
        .map(|s| (s.width, s.height))
        .unwrap_or((llamp_core::LYRICS_MIN_W, llamp_core::LYRICS_MIN_H));
    let client = lyrics_client_rect(w, llamp_core::LYRICS_MIN_H);
    let rect = line_rect(index, scroll_px, client.w, scale);
    LlampFrame {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
    }
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_mark_rect(scale: i32) -> LlampFrame {
    let (w, h) = state()
        .lock()
        .map(|s| (s.width, s.height))
        .unwrap_or((llamp_core::LYRICS_MIN_W, llamp_core::LYRICS_MIN_H));
    let rect = mark_rect(w, h, scale);
    LlampFrame {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
    }
}

/// 0 empty, 1 unsynced, 2 synced.
#[no_mangle]
pub extern "C" fn llamp_lyrics_kind() -> u32 {
    match state().lock().ok().and_then(|s| s.doc.as_ref().map(|d| d.kind)) {
        None => 0,
        Some(LyricKind::Unsynced) => 1,
        Some(LyricKind::Synced) => 2,
    }
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_line_count() -> u32 {
    state()
        .lock()
        .ok()
        .and_then(|s| s.doc.as_ref().map(|d| match d.kind {
            LyricKind::Synced => d.lines.len() as u32,
            LyricKind::Unsynced => 1,
        }))
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_line_text(index: u32, out: *mut c_char, len: usize) -> i32 {
    write_owned(
        state().lock().ok().and_then(|s| {
            let doc = s.doc.as_ref()?;
            match doc.kind {
                LyricKind::Synced => doc.lines.get(index as usize).map(|l| l.text.clone()),
                LyricKind::Unsynced if index == 0 => Some(doc.plain.clone()),
                _ => None,
            }
        }),
        out,
        len,
    )
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_active_line() -> i32 {
    let Ok(state) = state().lock() else {
        return -1;
    };
    let Some(doc) = state.doc.as_ref() else {
        return -1;
    };
    let snap = session().poll();
    llamp_core::cursor_for(doc, snap.position_frames, snap.sample_rate, state.user_offset_ms)
        .line
        .map(|i| i as i32)
        .unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_active_word() -> i32 {
    let Ok(state) = state().lock() else {
        return -1;
    };
    let Some(doc) = state.doc.as_ref() else {
        return -1;
    };
    let snap = session().poll();
    llamp_core::cursor_for(doc, snap.position_frames, snap.sample_rate, state.user_offset_ms)
        .word
        .map(|i| i as i32)
        .unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_word_count(line: u32) -> u32 {
    state()
        .lock()
        .ok()
        .and_then(|s| s.doc.as_ref()?.lines.get(line as usize).map(|l| l.words.len() as u32))
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_word_text(line: u32, word: u32, out: *mut c_char, len: usize) -> i32 {
    write_owned(
        state().lock().ok().and_then(|s| {
            s.doc.as_ref()?
                .lines
                .get(line as usize)?
                .words
                .get(word as usize)
                .map(|w| w.text.clone())
        }),
        out,
        len,
    )
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_load(path: *const c_char) -> i32 {
    let Some(path) = cstr_path(path).map(PathBuf::from) else {
        return LLAMP_ERR_INVALID;
    };
    let doc = resolve_lyrics(&path, None);
    let attribution = u8::from(matches!(doc.as_ref().map(|d| d.source), Some(LyricSource::Remote | LyricSource::Cache)));
    if let Ok(mut state) = state().lock() {
        state.doc = doc;
        state.path = Some(path);
        state.user_offset_ms = 0;
        state.sidecar_written = 0;
        state.attribution = attribution;
    }
    LLAMP_OK
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_privacy_note() -> *const c_char {
    static NOTE: OnceLock<CString> = OnceLock::new();
    NOTE.get_or_init(|| CString::new(llamp_core::PRIVACY_NOTE).expect("note"))
        .as_ptr()
}

/// 0 unknown, 1 accept, 2 decline.
#[no_mangle]
pub extern "C" fn llamp_lyrics_consent() -> u32 {
    match library_opt(|lib| lib.lyrics_consent()) {
        Some(Ok(LrclibConsent::Accept)) => 1,
        Some(Ok(LrclibConsent::Decline)) => 2,
        _ => 0,
    }
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_set_consent(value: u32) -> i32 {
    let consent = match value {
        1 => LrclibConsent::Accept,
        2 => LrclibConsent::Decline,
        _ => LrclibConsent::Unknown,
    };
    match library_opt(|lib| lib.set_lyrics_consent(consent)) {
        Some(Ok(())) => LLAMP_OK,
        _ => LLAMP_ERR_INVALID,
    }
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_offset_ms() -> i32 {
    state().lock().map(|s| s.user_offset_ms).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_step_offset(steps: i32) -> i32 {
    let Ok(mut state) = state().lock() else {
        return 0;
    };
    state.user_offset_ms = llamp_core::step_offset(state.user_offset_ms, steps);
    state.user_offset_ms
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_save_offset() -> i32 {
    let Ok(mut state) = state().lock() else {
        return LLAMP_ERR_INVALID;
    };
    let (Some(path), Some(doc)) = (state.path.clone(), state.doc.clone()) else {
        return LLAMP_ERR_INVALID;
    };
    let offset = state.user_offset_ms;
    let result = library_opt(|lib| lib.save_offset(&path, &doc, offset, None));
    match result {
        Some(Ok(OffsetSave::Sidecar(_))) => {
            state.sidecar_written = 1;
            LLAMP_OK
        }
        Some(Ok(OffsetSave::Sqlite)) => {
            state.sidecar_written = 0;
            LLAMP_OK
        }
        _ => LLAMP_ERR_INVALID,
    }
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_sidecar_written() -> u8 {
    state().lock().map(|s| s.sidecar_written).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_sidecar_note() -> *const c_char {
    static NOTE: OnceLock<CString> = OnceLock::new();
    NOTE.get_or_init(|| CString::new(llamp_core::SIDECAR_NOT_WRITTEN).expect("note"))
        .as_ptr()
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_from_lrclib() -> u8 {
    state().lock().map(|s| s.attribution).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn llamp_lyrics_lookup() -> i32 {
    if llamp_lyrics_consent() != 1 {
        return LLAMP_ERR_INVALID;
    }
    let path = state().lock().ok().and_then(|s| s.path.clone());
    let Some(path) = path else {
        return LLAMP_ERR_INVALID;
    };
    let tags = library_opt(|lib| lib.read_tags(&path)).and_then(Result::ok);
    let snap = session().poll();
    let duration = if snap.sample_rate == 0 {
        0
    } else {
        (snap.duration_frames / u64::from(snap.sample_rate.max(1))) as u32
    };
    let query = LyricQuery {
        artist: tags.as_ref().map(|t| t.artist.clone()).unwrap_or_default(),
        title: tags.as_ref().map(|t| t.title.clone()).unwrap_or_default(),
        album: tags.as_ref().map(|t| t.album.clone()).unwrap_or_default(),
        duration_seconds: duration,
    };
    let plugin_query = llamp_plugin_api::LyricQuery {
        artist: query.artist.clone(),
        title: query.title.clone(),
        album: query.album.clone(),
        duration_seconds: query.duration_seconds,
    };
    if let Some(Ok(true)) = library_opt(|lib| lib.cached_miss_fresh(Some(&path), &query)) {
        return LLAMP_ERR_INVALID;
    }
    if let Some(Ok(Some(doc))) = library_opt(|lib| lib.cached_lyrics(Some(&path), &query)) {
        if let Ok(mut state) = state().lock() {
            state.attribution = 1;
            state.doc = Some(doc);
        }
        return LLAMP_OK;
    }
    let Some(provider) = crate::lyrics_provider() else {
        return LLAMP_ERR_INVALID;
    };
    match provider.fetch(&plugin_query) {
        Ok(body) => {
            let kind = if body.contains('[') {
                Some(LyricKind::Synced)
            } else {
                Some(LyricKind::Unsynced)
            };
            let _ = library_opt(|lib| {
                lib.cache_lyrics(Some(&path), &query, Some(&body), LyricSource::Remote, kind)
            });
            let doc = llamp_library::parse_lrc(&body).or_else(|| {
                Some(llamp_library::LyricDoc {
                    kind: LyricKind::Unsynced,
                    source: LyricSource::Remote,
                    tags: llamp_library::LyricTags::default(),
                    offset_ms: 0,
                    lines: Vec::new(),
                    plain: body,
                })
            });
            if let Ok(mut state) = state().lock() {
                if let Some(mut doc) = doc {
                    doc.source = LyricSource::Remote;
                    state.doc = Some(doc);
                    state.attribution = 1;
                }
            }
            LLAMP_OK
        }
        Err(_) => {
            let _ = library_opt(|lib| lib.cache_lyrics(Some(&path), &query, None, LyricSource::Remote, None));
            LLAMP_ERR_INVALID
        }
    }
}

#[no_mangle]
pub extern "C" fn llamp_text_current() -> u32 {
    pack_color(|c| c.current)
}

#[no_mangle]
pub extern "C" fn llamp_text_selected_bg() -> u32 {
    pack_color(|c| c.selected_bg)
}

fn pack_color(pick: fn(&llamp_skin::PlaylistColors) -> llamp_skin::Rgb) -> u32 {
    let rgb = skin_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.current().map(|skin| pick(&skin.playlist_colors)))
        .unwrap_or_else(|| pick(&llamp_skin::default_playlist_colors()));
    u32::from(rgb.r) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.b)
}

fn library_opt<T>(f: impl FnOnce(&llamp_library::Library) -> T) -> Option<T> {
    let slot = library_slot().lock().ok()?;
    Some(f(slot.source.as_ref()?.library()))
}

fn write_owned(text: Option<String>, out: *mut c_char, len: usize) -> i32 {
    if out.is_null() || len == 0 {
        return LLAMP_ERR_INVALID;
    }
    let Some(text) = text else {
        return LLAMP_ERR_INVALID;
    };
    let bytes = text.as_bytes();
    if bytes.len() + 1 > len {
        return LLAMP_ERR_INVALID;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast(), bytes.len());
        *out.add(bytes.len()) = 0;
    }
    LLAMP_OK
}

#[allow(dead_code)]
fn _cstr_used(text: *const c_char) -> String {
    cstr(text)
}
