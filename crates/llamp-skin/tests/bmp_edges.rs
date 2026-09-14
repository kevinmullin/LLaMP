//! One test per BMP edge. A single "does not panic" is not the contract.

mod common;

use llamp_skin::{Sprite, FALLBACK_RGBA};

fn px(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    rgba[i..i + 4].try_into().unwrap()
}

#[test]
fn bottom_up_rows_are_flipped_to_top_down() {
    let mut rgb = vec![0u8; 2 * 2 * 3];
    rgb[0..3].copy_from_slice(&[255, 0, 0]);
    rgb[3..6].copy_from_slice(&[0, 255, 0]);
    rgb[6..9].copy_from_slice(&[0, 0, 255]);
    rgb[9..12].copy_from_slice(&[255, 255, 0]);
    let file = common::bmp24(2, 2, false, &rgb, 0);
    let decoded = llamp_skin::decode_bmp(&file).expect("bottom-up");
    assert_eq!(px(&decoded.rgba, 2, 0, 0), [255, 0, 0, 255]);
    assert_eq!(px(&decoded.rgba, 2, 1, 0), [0, 255, 0, 255]);
    assert_eq!(px(&decoded.rgba, 2, 0, 1), [0, 0, 255, 255]);
    assert_eq!(px(&decoded.rgba, 2, 1, 1), [255, 255, 0, 255]);
}

#[test]
fn row_stride_is_four_byte_aligned() {
    let mut rgb = vec![0u8; 1 * 2 * 3];
    rgb[0..3].copy_from_slice(&[1, 2, 3]);
    rgb[3..6].copy_from_slice(&[4, 5, 6]);
    let file = common::bmp24(1, 2, false, &rgb, 0xAB);
    let decoded = llamp_skin::decode_bmp(&file).expect("stride");
    assert_eq!(px(&decoded.rgba, 1, 0, 0), [1, 2, 3, 255]);
    assert_eq!(px(&decoded.rgba, 1, 0, 1), [4, 5, 6, 255]);
    assert!(!decoded.rgba.contains(&0xAB));
}

#[test]
fn eight_bit_uses_the_palette() {
    let file = common::bmp8(2, 1, &[0, 1], &[[0, 0, 255, 0], [0, 255, 0, 0]]);
    let decoded = llamp_skin::decode_bmp(&file).expect("palette");
    assert_eq!(px(&decoded.rgba, 2, 0, 0), [255, 0, 0, 255]);
    assert_eq!(px(&decoded.rgba, 2, 1, 0), [0, 255, 0, 255]);
}

#[test]
fn four_bit_is_two_pixels_per_byte() {
    let file = common::bmp4(2, 1, &[0x01], &[[0, 0, 255, 0], [0, 255, 0, 0]]);
    let decoded = llamp_skin::decode_bmp(&file).expect("4-bit");
    assert_eq!(decoded.width, 2);
    assert_eq!(px(&decoded.rgba, 2, 0, 0), [255, 0, 0, 255]);
    assert_eq!(px(&decoded.rgba, 2, 1, 0), [0, 255, 0, 255]);
}

#[test]
fn rle8_run_and_absolute_mode() {
    // Absolute 3 pixels, word pad, then a run of 3. One row so row order is not the assertion.
    let rle = [0x00, 0x03, 0x01, 0x02, 0x03, 0x00, 0x03, 0x04, 0x00, 0x01];
    let palette = [
        [0, 0, 0, 0],
        [0, 0, 255, 0],
        [0, 255, 0, 0],
        [255, 0, 0, 0],
        [255, 255, 255, 0],
    ];
    let file = common::bmp_rle8(6, 1, &palette, &rle);
    let decoded = llamp_skin::decode_bmp(&file).expect("rle8");
    assert_eq!(px(&decoded.rgba, 6, 0, 0), [255, 0, 0, 255]);
    assert_eq!(px(&decoded.rgba, 6, 1, 0), [0, 255, 0, 255]);
    assert_eq!(px(&decoded.rgba, 6, 2, 0), [0, 0, 255, 255]);
    assert_eq!(px(&decoded.rgba, 6, 3, 0), [255, 255, 255, 255]);
    assert_eq!(px(&decoded.rgba, 6, 4, 0), [255, 255, 255, 255]);
    assert_eq!(px(&decoded.rgba, 6, 5, 0), [255, 255, 255, 255]);
}

#[test]
fn rle8_decompression_bomb_does_not_allocate_past_the_cap() {
    let file = common::bmp_rle8(4096, 4096, &[[0, 0, 0, 0]], &[0xFF, 0x01, 0x00, 0x01]);
    HUGE.store(false, std::sync::atomic::Ordering::SeqCst);
    let err = llamp_skin::decode_bmp(&file).expect_err("bomb");
    assert!(
        err.contains("cap") || err.contains("4096") || err.contains("uncompressed"),
        "{err}"
    );
    assert!(
        !HUGE.load(std::sync::atomic::Ordering::SeqCst),
        "allocated past the cap"
    );
}

#[test]
fn declared_dimensions_that_overflow_width_times_height_times_4_are_rejected() {
    let file = common::bmp_header_only(i32::MAX, i32::MAX, 24, 0);
    HUGE.store(false, std::sync::atomic::Ordering::SeqCst);
    let err = llamp_skin::decode_bmp(&file).expect_err("overflow");
    assert!(err.contains("overflow"), "{err}");
    assert!(
        !HUGE.load(std::sync::atomic::Ordering::SeqCst),
        "allocated before the overflow check"
    );
}

#[test]
fn color_key_is_the_top_left_of_a_sprite_sheet_not_main() {
    let main = common::main_with_pixel([0x1A, 0x1A, 0x1A], (10, 10), [0xFF, 0x00, 0xFF]);
    let mut sheet = vec![0xFFu8; 139 * 37 * 3];
    for chunk in sheet.chunks_exact_mut(3) {
        chunk.copy_from_slice(&[0xFF, 0x00, 0xFF]);
    }
    let green = (1 * 139 + 1) * 3;
    sheet[green..green + 3].copy_from_slice(&[0x00, 0xFF, 0x00]);
    let buttons = common::bmp24(139, 37, false, &sheet, 0);
    let wsz = common::zip_stored(&[("main.bmp", &main), ("cbuttons.bmp", &buttons)]);
    let mut slot = llamp_skin::SkinSlot::new();
    let skin = slot.load_wsz(&wsz).expect("load");
    assert_eq!(
        skin.sprite_px(Sprite::Main, 10, 10),
        [0xFF, 0x00, 0xFF, 255]
    );
    assert_eq!(
        skin.sprite_px(Sprite::Previous, 0, 0),
        [0x00, 0xFF, 0x00, 255]
    );
    assert_eq!(skin.sprite_px(Sprite::Previous, 1, 0)[3], 0);
    let _ = FALLBACK_RGBA;
}

struct HugeAlloc;

static HUGE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

unsafe impl std::alloc::GlobalAlloc for HugeAlloc {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        if layout.size() > 2_000_000 {
            HUGE.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        unsafe { std::alloc::System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: HugeAlloc = HugeAlloc;
