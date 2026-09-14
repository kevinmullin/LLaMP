//! Live kbps, kHz, and spectrum on `llamp_skin_blit_display`.

use llamp_audio::analysis::{log_bands, publish_hop, FftTap, FrameHop};

#[test]
fn blit_display_stamps_live_kbps_khz_and_spectrum() {
    let wsz = xtask::fixture_wsz();
    assert_eq!(
        llamp_ffi::llamp_skin_load(wsz.as_ptr(), wsz.len()),
        llamp_ffi::LLAMP_OK
    );
    let title = std::ffi::CString::new("tone").unwrap();
    llamp_ffi::llamp_session_configure(48_000, 48_000, 2, title.as_ptr());
    llamp_ffi::llamp_session_set_kbps(320);
    llamp_ffi::llamp_transport_press(8);

    let mut tap = FftTap::new();
    let mut pcm = [0f32; 2048];
    for i in 0..1024 {
        pcm[i * 2] = 1.0;
        pcm[i * 2 + 1] = 0.25;
    }
    let spec = tap.push(&pcm).expect("hop");
    let mut packed = [0f32; 1024];
    packed.copy_from_slice(&pcm[..1024]);
    publish_hop(FrameHop {
        pcm: packed,
        bins_db: spec.bins_db,
        rms_l: 1.0,
        rms_r: 0.25,
        peak_l: 1.0,
        peak_r: 0.25,
        onset: 0.0,
    });

    let image = llamp_ffi::llamp_skin_blit_display(0);
    assert!(!image.data.is_null());
    let rgba = unsafe { std::slice::from_raw_parts(image.data, image.len) };
    let pane = llamp_skin::control_rect(llamp_skin::Control::VisPane);
    let n = llamp_audio::analysis::band_count(pane.w, llamp_skin::VIS_BAR_W);
    let mut bands = [0.0f32; 32];
    let filled = log_bands(&spec.bins_db, pane.w, llamp_skin::VIS_BAR_W, &mut bands);
    assert_eq!(filled, n);
    assert_eq!(n, (pane.w / llamp_skin::VIS_BAR_W) as usize);

    let kbps = pixel(rgba, 111, 43);
    let khz = pixel(rgba, 156, 43);
    let bar = pixel(rgba, pane.x, pane.y + pane.h - 1);
    assert_ne!(kbps, [0, 0, 0, 0]);
    assert_ne!(khz, [0, 0, 0, 0]);
    assert_ne!(bar, [0, 0, 0, 0]);
    llamp_ffi::llamp_image_free(image.data, image.len);

    let packet = llamp_ffi::llamp_vis_packet();
    assert_eq!(packet.fft_linear.len(), 513);
    assert_eq!(packet.fft_bands_len, n as u32);
}

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * 275 + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}
