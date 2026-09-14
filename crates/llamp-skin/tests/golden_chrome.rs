//! Fixture-locked EQ, playlist, gen, kbps, and spectrum goldens.

use llamp_skin::{blit_eq_skin, blit_gen, blit_playlist, EqPaint};

fn load_fixture() -> llamp_skin::Skin {
    let wsz = xtask::fixture_wsz();
    let mut slot = llamp_skin::SkinSlot::new();
    slot.load_wsz(&wsz).expect("fixture").clone()
}

fn golden_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden")
}

fn assert_png(name: &str, rgba: &[u8], width: u32, height: u32) {
    let path = golden_dir().join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("committed golden {name}"));
    let png = llamp_skin::read_golden_png(&bytes).expect("png");
    assert_eq!(png.width, width);
    assert_eq!(png.height, height);
    assert_eq!(png.color_type, 6);
    assert!(png.srgb);
    assert_eq!(png.rgba, rgba, "{name} drifted");
}

fn write_png(name: &str, rgba: &[u8], width: u32, height: u32) {
    let bytes = llamp_skin::write_golden_png(rgba, width, height).expect("png");
    std::fs::write(golden_dir().join(name), bytes).expect("write");
}

#[test]
fn eq_window_matches_committed_png() {
    let skin = load_fixture();
    let mut paint = EqPaint::default();
    paint.on = true;
    paint.preamp_db = 0.0;
    let rgba = blit_eq_skin(Some(&skin), &paint);
    assert_eq!(rgba.len(), 275 * 116 * 4);
    assert_eq!(pixel(&rgba, 275, 10, 18), [0x20, 0x20, 0x50, 255]);
    assert_eq!(pixel(&rgba, 275, 10, 70)[0], 0xE0);
    assert_ne!(pixel(&rgba, 275, 10, 70)[0], 0x80);
    assert_png("eq-275x116.png", &rgba, 275, 116);
}

#[test]
fn playlist_min_matches_committed_png() {
    let skin = load_fixture();
    let rgba = blit_playlist(&skin, 275, 116);
    assert_eq!(pixel(&rgba, 275, 20, 0)[0], 0x8A);
    assert_eq!(pixel(&rgba, 275, 0, 103)[0], 0xC0);
    assert_png("playlist-275x116.png", &rgba, 275, 116);
}

#[test]
fn gen_window_matches_committed_png() {
    let skin = load_fixture();
    let rgba = blit_gen(&skin, 275, 116);
    assert_eq!(pixel(&rgba, 275, 20, 0)[0], 0x30);
    assert_eq!(pixel(&rgba, 275, 10, 20)[0], 0x10);
    assert_png("gen-275x116.png", &rgba, 275, 116);
}

#[test]
fn kbps_khz_display_matches_committed_png() {
    let skin = load_fixture();
    let rgba = llamp_skin::blit_display(
        &skin,
        llamp_skin::Display {
            time: "1:00",
            kbps: 320,
            khz: 44,
            ..llamp_skin::Display::default()
        },
    );
    assert_ne!(pixel(&rgba, 275, 111, 43), [0x1A, 0x1A, 0x1A, 255]);
    assert_ne!(pixel(&rgba, 275, 156, 43), [0x1A, 0x1A, 0x1A, 255]);
    assert_png("main-kbps-khz.png", &rgba, 275, 116);
}

#[test]
fn spectrum_bar_width_is_four() {
    let skin = load_fixture();
    let mut bands = [0.0f32; 32];
    bands[0] = 1.0;
    let rgba = llamp_skin::blit_display(
        &skin,
        llamp_skin::Display {
            time: "1:00",
            spectrum: Some(&bands),
            ..llamp_skin::Display::default()
        },
    );
    let plot = [0, 255, 0, 255];
    let bg = [0, 0, 0, 255];
    let x0 = 24u32;
    assert_eq!(pixel(&rgba, 275, x0, 52 + 15), plot);
    assert_eq!(pixel(&rgba, 275, x0 + 3, 52 + 15), plot);
    assert_eq!(pixel(&rgba, 275, x0 + 4, 52 + 15), bg);
    assert_png("vis-spectrum-76x16.png", &rgba, 275, 116);
}

#[test]
#[ignore]
fn write_chrome_goldens() {
    let skin = load_fixture();
    let mut paint = EqPaint::default();
    paint.on = true;
    write_png("eq-275x116.png", &blit_eq_skin(Some(&skin), &paint), 275, 116);
    write_png("playlist-275x116.png", &blit_playlist(&skin, 275, 116), 275, 116);
    write_png("gen-275x116.png", &blit_gen(&skin, 275, 116), 275, 116);
    write_png(
        "main-kbps-khz.png",
        &llamp_skin::blit_display(
            &skin,
            llamp_skin::Display {
                time: "1:00",
                kbps: 320,
                khz: 44,
                ..llamp_skin::Display::default()
            },
        ),
        275,
        116,
    );
    let mut spectrum = [0.0f32; 32];
    spectrum[0] = 1.0;
    write_png(
        "vis-spectrum-76x16.png",
        &llamp_skin::blit_display(
            &skin,
            llamp_skin::Display {
                time: "1:00",
                spectrum: Some(&spectrum),
                ..llamp_skin::Display::default()
            },
        ),
        275,
        116,
    );
}

fn pixel(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    buf[i..i + 4].try_into().unwrap()
}
