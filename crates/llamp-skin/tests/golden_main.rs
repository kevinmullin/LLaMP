//! The phase 2 PNG is the contract phases 3, 13, and 14 compare against.
//!
//! Byte layout: 275×116, RGBA8, straight alpha, row-major, top to bottom, no row padding.
//! Color space: sRGB, PNG color type 6, 8-bit, an sRGB chunk, no iCCP, gAMA, or cHRM.

mod common;

use llamp_skin::Rgb;

fn pinned_ramp() -> [Rgb; 24] {
    let mut colors = [Rgb { r: 0, g: 0, b: 0 }; 24];
    colors[1] = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let ramp = [
        (0, 255),
        (17, 238),
        (34, 221),
        (51, 204),
        (68, 187),
        (85, 170),
        (102, 153),
        (119, 136),
        (136, 119),
        (153, 102),
        (170, 85),
        (187, 68),
        (204, 51),
        (221, 34),
        (238, 17),
        (255, 0),
    ];
    for (i, (r, g)) in ramp.into_iter().enumerate() {
        colors[2 + i] = Rgb { r, g, b: 0 };
    }
    for slot in 18..23 {
        colors[slot] = Rgb { r: 0, g: 255, b: 0 };
    }
    colors[23] = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    colors
}

#[test]
fn default_viscolor_ramp_is_this_table() {
    assert_eq!(llamp_skin::default_vis_colors(), pinned_ramp());
}

#[test]
fn fixture_region_matches_region_txt_after_clamp() {
    let wsz = xtask::fixture_wsz();
    let mut slot = llamp_skin::SkinSlot::new();
    let skin = slot.load_wsz(&wsz).expect("fixture");
    assert_eq!(
        skin.regions.normal,
        vec![llamp_skin::Polygon {
            points: vec![(0, 0), (274, 0), (274, 115), (0, 115)],
        }]
    );
    assert!(
        skin.defects.iter().any(|d| d.contains("clamped")),
        "{:?}",
        skin.defects
    );
    assert_eq!(skin.vis_colors, pinned_ramp());
}

#[test]
fn main_window_matches_committed_png() {
    let wsz = xtask::fixture_wsz();
    let mut slot = llamp_skin::SkinSlot::new();
    let skin = slot.load_wsz(&wsz).expect("fixture");
    let rgba = llamp_skin::blit_main(skin);
    assert_eq!(rgba.len(), 275 * 116 * 4);
    assert_eq!(pixel(&rgba, 275, 8, 13), [0x4A, 0x4A, 0x4A, 255]);
    assert_eq!(pixel(&rgba, 275, 8, 14), [0x1A, 0x1A, 0x1A, 255]);
    assert_eq!(pixel(&rgba, 275, 31, 98), [0x40, 0xC0, 0x40, 255]);
    assert_eq!(pixel(&rgba, 275, 8, 42), [0xF0, 0xF0, 0xF0, 255]);
    assert_eq!(pixel(&rgba, 275, 72, 22), [0x10, 0x10, 0x10, 255]);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/golden/main-275x116.png");
    let bytes = std::fs::read(&path).expect("committed golden png");
    let png = llamp_skin::read_golden_png(&bytes).expect("png");
    assert_eq!(png.width, 275);
    assert_eq!(png.height, 116);
    assert_eq!(png.color_type, 6);
    assert_eq!(png.bit_depth, 8);
    assert!(png.srgb, "sRGB chunk required");
    assert!(!png.iccp, "no display profile");
    assert!(!png.gama, "sRGB chunk only, no gAMA");
    assert!(!png.chrm, "sRGB chunk only, no cHRM");
    assert_eq!(png.rgba, rgba);
}

#[test]
fn double_size_is_nearest_neighbor_not_a_blend() {
    let wsz = xtask::fixture_wsz();
    let mut slot = llamp_skin::SkinSlot::new();
    let skin = slot.load_wsz(&wsz).expect("fixture");
    let rgba = llamp_skin::blit_main(skin);
    let doubled = llamp_skin::scale_nearest(&rgba, 275, 116, 2);
    assert_eq!(doubled.len(), 550 * 232 * 4);
    // Titlebar fills y=13 except the color-key pixel at x=0. Sample x=8, the edge with y=14.
    let top = pixel(&rgba, 275, 8, 13);
    let bottom = pixel(&rgba, 275, 8, 14);
    assert_ne!(
        top, bottom,
        "edge must be a hard skin pixel, not already blended"
    );
    assert_eq!(pixel(&doubled, 550, 16, 26), top);
    assert_eq!(pixel(&doubled, 550, 17, 27), top);
    assert_eq!(pixel(&doubled, 550, 16, 28), bottom);
    assert_eq!(pixel(&doubled, 550, 17, 29), bottom);
    let blend = blend4(top, bottom);
    assert_ne!(pixel(&doubled, 550, 16, 27), blend);
    assert_ne!(pixel(&doubled, 550, 16, 28), blend);
}

fn pixel(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    buf[i..i + 4].try_into().unwrap()
}

fn blend4(a: [u8; 4], b: [u8; 4]) -> [u8; 4] {
    [
        ((u16::from(a[0]) + u16::from(b[0])) / 2) as u8,
        ((u16::from(a[1]) + u16::from(b[1])) / 2) as u8,
        ((u16::from(a[2]) + u16::from(b[2])) / 2) as u8,
        ((u16::from(a[3]) + u16::from(b[3])) / 2) as u8,
    ]
}

#[test]
fn custom_region_punches_alpha_outside_the_union() {
    let main = common::solid_main([0x1A, 0x1A, 0x1A]);
    let region = b"[Normal]\nNumPoints=4\nPointList=0,0,9,0,9,9,0,9\n";
    let wsz = common::zip_stored(&[("main.bmp", &main), ("region.txt", region)]);
    let mut slot = llamp_skin::SkinSlot::new();
    let skin = slot.load_wsz(&wsz).expect("region");
    let rgba = llamp_skin::blit_main(skin);
    assert_eq!(pixel(&rgba, 275, 0, 0)[3], 255);
    assert_eq!(pixel(&rgba, 275, 9, 9)[3], 255);
    assert_eq!(pixel(&rgba, 275, 10, 10)[3], 0);
}
