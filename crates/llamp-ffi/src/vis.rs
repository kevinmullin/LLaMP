//! Visualizer surface lifetime, frame packet, and first-party presets.
//!
//! The shell owns the layer and the view. This module does not retain, free,
//! or release either pointer. A stale generation fails without reading them.

use std::ffi::{c_char, c_void};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::{
    image_from, session, skin_slot, LlampFrame, LlampImage, LlampSize, LLAMP_ERR_INVALID, LLAMP_OK,
};

pub const WGPU_VERSION: &str = "30.0.1";

struct Slot {
    layer: *mut c_void,
    view: *mut c_void,
    generation: u64,
    width: u32,
    height: u32,
    gpu: Option<Gpu>,
}

unsafe impl Send for Slot {}

struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn slot() -> &'static Mutex<Option<Slot>> {
    static SLOT: OnceLock<Mutex<Option<Slot>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

static SUBMITS: AtomicU64 = AtomicU64::new(0);
static OCCLUDED: AtomicU8 = AtomicU8::new(0);

#[no_mangle]
pub extern "C" fn llamp_vis_surface_reset() {
    if let Ok(mut guard) = slot().lock() {
        *guard = None;
    }
    SUBMITS.store(0, Ordering::Relaxed);
    OCCLUDED.store(0, Ordering::Relaxed);
}

/// Bind a shell-owned `CAMetalLayer`. Does not create a wgpu surface.
/// The pointer is stored only until invalidate or a newer bind.
#[no_mangle]
pub extern "C" fn llamp_vis_surface_bind(layer: *mut c_void, generation: u64) -> i32 {
    if layer.is_null() || generation == 0 {
        return LLAMP_ERR_INVALID;
    }
    let Ok(mut guard) = slot().lock() else {
        return LLAMP_ERR_INVALID;
    };
    *guard = Some(Slot {
        layer,
        view: std::ptr::null_mut(),
        generation,
        width: 0,
        height: 0,
        gpu: None,
    });
    LLAMP_OK
}

/// Bind the shell-owned `NSView` for wgpu 30 (`RawHandle`, not a layer pointer).
/// Drops any previous surface before storing the new view.
#[no_mangle]
pub extern "C" fn llamp_vis_surface_bind_view(view: *mut c_void, generation: u64) -> i32 {
    if view.is_null() || generation == 0 {
        return LLAMP_ERR_INVALID;
    }
    let Ok(mut guard) = slot().lock() else {
        return LLAMP_ERR_INVALID;
    };
    let Some(current) = guard.as_mut() else {
        return LLAMP_ERR_INVALID;
    };
    if current.generation != generation {
        return LLAMP_ERR_INVALID;
    }
    current.gpu = None;
    current.view = view;
    current.gpu = unsafe { open_gpu(view) };
    LLAMP_OK
}

#[no_mangle]
pub extern "C" fn llamp_vis_surface_resize(width: u32, height: u32, generation: u64) -> i32 {
    if width == 0 || height == 0 {
        return LLAMP_ERR_INVALID;
    }
    let Ok(mut guard) = slot().lock() else {
        return LLAMP_ERR_INVALID;
    };
    let Some(current) = guard.as_mut() else {
        return LLAMP_ERR_INVALID;
    };
    if current.generation != generation {
        return LLAMP_ERR_INVALID;
    }
    current.width = width;
    current.height = height;
    if let Some(gpu) = current.gpu.as_ref() {
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };
        gpu.surface.configure(&gpu.device, &config);
    }
    LLAMP_OK
}

/// Drops the wgpu surface before the slot forgets the pointers.
#[no_mangle]
pub extern "C" fn llamp_vis_surface_invalidate(generation: u64) -> i32 {
    let Ok(mut guard) = slot().lock() else {
        return LLAMP_ERR_INVALID;
    };
    match guard.as_ref() {
        Some(current) if current.generation == generation => {
            *guard = None;
            LLAMP_OK
        }
        _ => LLAMP_ERR_INVALID,
    }
}

#[no_mangle]
pub extern "C" fn llamp_vis_surface_generation() -> u64 {
    slot()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|current| current.generation))
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn llamp_vis_set_occluded(occluded: u8) {
    OCCLUDED.store(occluded, Ordering::Relaxed);
}

#[no_mangle]
pub extern "C" fn llamp_vis_gpu_submits() -> u64 {
    SUBMITS.load(Ordering::Relaxed)
}

/// Present one frame. Occluded or hidden submits zero GPU work.
#[no_mangle]
pub extern "C" fn llamp_vis_present(generation: u64) -> i32 {
    let Ok(mut guard) = slot().lock() else {
        return LLAMP_ERR_INVALID;
    };
    let Some(current) = guard.as_mut() else {
        return LLAMP_ERR_INVALID;
    };
    if current.generation != generation {
        return LLAMP_ERR_INVALID;
    }
    if OCCLUDED.load(Ordering::Relaxed) != 0 {
        return LLAMP_OK;
    }
    if let Some(gpu) = current.gpu.as_ref() {
        match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let hop = llamp_audio::analysis::hop_snapshot();
                    let color = onset_clear(hop.onset, hop.rms_l);
                    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(color),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                }
                gpu.queue.submit(Some(encoder.finish()));
                gpu.queue.present(frame);
                SUBMITS.fetch_add(1, Ordering::Relaxed);
            }
            wgpu::CurrentSurfaceTexture::Occluded => {}
            _ => {}
        }
    } else {
        SUBMITS.fetch_add(1, Ordering::Relaxed);
    }
    let _ = current.layer;
    LLAMP_OK
}

fn onset_clear(onset: f32, rms: f32) -> wgpu::Color {
    let pulse = (onset.max(0.0).min(1.0) * 0.4 + rms.max(0.0).min(1.0) * 0.2) as f64;
    wgpu::Color {
        r: 0.05 + pulse,
        g: 0.02,
        b: 0.12 + pulse * 0.5,
        a: 1.0,
    }
}

unsafe fn open_gpu(view: *mut c_void) -> Option<Gpu> {
    let view = NonNull::new(view)?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let handle = AppKitWindowHandle::new(view);
    let surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())),
                raw_window_handle: RawWindowHandle::AppKit(handle),
            })
            .ok()?
    };
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: Some(&surface),
        ..Default::default()
    }))
    .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .ok()?;
    Some(Gpu {
        surface,
        device,
        queue,
    })
}

#[repr(C)]
pub struct LlampVisPacket {
    pub pcm: [f32; 1024],
    pub fft_linear: [f32; 513],
    pub fft_bands: [f32; 32],
    pub fft_bands_len: u32,
    pub rms_l: f32,
    pub rms_r: f32,
    pub peak_l: f32,
    pub peak_r: f32,
    pub onset: f32,
    pub position_frames: u64,
    pub duration_frames: u64,
    pub produces_pcm: u8,
    pub title_len: u16,
    pub title: [u8; 256],
}

/// Pulls the latest hop. Log bands use the golden pane width and bar width.
#[no_mangle]
pub extern "C" fn llamp_vis_packet() -> LlampVisPacket {
    let snap = session().poll();
    let hop = llamp_audio::analysis::hop_snapshot();
    let pane = llamp_skin::control_rect(llamp_skin::Control::VisPane);
    let mut bands = [0.0f32; 32];
    let n = llamp_audio::analysis::log_bands(&hop.bins_db, pane.w, llamp_skin::VIS_BAR_W, &mut bands);
    let mut fft = [-120.0f32; 513];
    let copy = hop.bins_db.len().min(fft.len());
    fft[..copy].copy_from_slice(&hop.bins_db[..copy]);
    LlampVisPacket {
        pcm: hop.pcm,
        fft_linear: fft,
        fft_bands: bands,
        fft_bands_len: n as u32,
        rms_l: hop.rms_l,
        rms_r: hop.rms_r,
        peak_l: hop.peak_l,
        peak_r: hop.peak_r,
        onset: hop.onset,
        position_frames: snap.position_frames,
        duration_frames: snap.duration_frames,
        produces_pcm: snap.produces_pcm,
        title_len: snap.title_len,
        title: snap.title,
    }
}

#[no_mangle]
pub extern "C" fn llamp_vis_chrome_blit(width: u32, height: u32) -> LlampImage {
    let empty = || LlampImage {
        data: std::ptr::null_mut(),
        width: 0,
        height: 0,
        len: 0,
    };
    let w = width.max(llamp_core::VIS_MIN_W as u32);
    let h = height.max(llamp_core::VIS_MIN_H as u32);
    let Ok(slot) = skin_slot().lock() else {
        return empty();
    };
    let Some(skin) = slot.current() else {
        return empty();
    };
    image_from(llamp_skin::blit_gen(skin, w, h), w, h)
}

#[no_mangle]
pub extern "C" fn llamp_vis_client_rect(width: i32, height: i32, fullscreen: u8) -> LlampFrame {
    let rect = llamp_core::client_rect(width, height, fullscreen != 0);
    LlampFrame {
        x: rect.x,
        y: rect.y,
        w: rect.w,
        h: rect.h,
    }
}

#[no_mangle]
pub extern "C" fn llamp_vis_propose_size(width: i32, height: i32) -> LlampSize {
    let (w, h) = llamp_core::propose_size(width, height);
    LlampSize {
        width: w as u32,
        height: h as u32,
    }
}

#[no_mangle]
pub extern "C" fn llamp_vis_min_size() -> LlampSize {
    LlampSize {
        width: llamp_core::VIS_MIN_W as u32,
        height: llamp_core::VIS_MIN_H as u32,
    }
}

const FLUX_JSON: &str = include_str!("../../../assets/visualizers/flux/preset.json");
const FLUX_WGSL: &str = include_str!("../../../assets/visualizers/flux/shader.wgsl");
const PULSE_JSON: &str = include_str!("../../../assets/visualizers/pulse/preset.json");
const PULSE_WGSL: &str = include_str!("../../../assets/visualizers/pulse/shader.wgsl");

#[derive(Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    pub author: &'static str,
    pub wgsl: &'static str,
}

fn parse_preset(json: &'static str, wgsl: &'static str) -> Option<Preset> {
    let name = field(json, "name")?;
    let author = field(json, "author")?;
    if !json.contains("\"llamp_preset\": 1") && !json.contains("\"llamp_preset\":1") {
        return None;
    }
    if !wgsl.contains("fn vs_main") || !wgsl.contains("fn fs_main") {
        return None;
    }
    Some(Preset { name, author, wgsl })
}

fn field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let rest = json.split_once(&needle)?.1;
    let rest = rest.split_once(':')?.1;
    let rest = rest.trim();
    let rest = rest.strip_prefix('"')?;
    Some(rest.split_once('"')?.0)
}

pub fn first_party_presets() -> [Preset; 2] {
    [
        parse_preset(FLUX_JSON, FLUX_WGSL).expect("flux"),
        parse_preset(PULSE_JSON, PULSE_WGSL).expect("pulse"),
    ]
}

#[no_mangle]
pub extern "C" fn llamp_vis_preset_count() -> u32 {
    2
}

#[no_mangle]
pub extern "C" fn llamp_vis_preset_name(index: u32) -> *const c_char {
    match index {
        0 => b"Flux\0".as_ptr().cast(),
        1 => b"Pulse\0".as_ptr().cast(),
        _ => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn llamp_wgpu_version() -> *const c_char {
    concat!("30.0.1", "\0").as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn llamp_session_set_kbps(kbps: u16) {
    session().set_kbps(kbps);
}
