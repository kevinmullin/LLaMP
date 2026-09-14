//! Load rules: optional sheets, failed main, zip-slip, caps. No archive path is opened.

mod common;

use std::sync::atomic::{AtomicBool, Ordering};

use llamp_skin::{Control, Sprite, FALLBACK_RGBA};

#[test]
fn missing_optional_sheet_loads_with_a_defect() {
    let main = common::solid_main([0x1A, 0x1A, 0x1A]);
    let wsz = common::zip_stored(&[("main.bmp", &main)]);
    let mut slot = llamp_skin::SkinSlot::new();
    let skin = slot.load_wsz(&wsz).expect("optional sheet is not fatal");
    assert!(
        skin.defects
            .iter()
            .any(|defect| defect.contains("titlebar.bmp")),
        "{:?}",
        skin.defects
    );
    let rect = skin.sprite(Sprite::Titlebar).expect("fallback sprite");
    assert_eq!(rect.w, 275);
    assert_eq!(rect.h, 14);
    assert_eq!(skin.sprite_px(Sprite::Titlebar, 1, 1), FALLBACK_RGBA);
}

#[test]
fn main_bmp_not_275_by_116_keeps_the_previous_skin() {
    let good = common::solid_main([0x1A, 0x1A, 0x1A]);
    let good_wsz = common::zip_stored(&[("main.bmp", &good)]);
    let mut pixels = vec![0u8; 274 * 116 * 3];
    for chunk in pixels.chunks_exact_mut(3) {
        chunk.copy_from_slice(&[0xFF, 0x00, 0x00]);
    }
    let bad = common::bmp24(274, 116, false, &pixels, 0);
    let bad_wsz = common::zip_stored(&[("main.bmp", &bad)]);
    let mut slot = llamp_skin::SkinSlot::new();
    let first = slot.load_wsz(&good_wsz).expect("first").id;
    let err = slot.load_wsz(&bad_wsz).expect_err("wrong size");
    assert!(err.to_string().contains("275"), "{err}");
    assert_eq!(slot.current().expect("previous").id, first);
}

#[test]
fn zip_slip_is_not_opened_on_the_filesystem() {
    let probe = std::env::temp_dir().join("llamp-zipslip-probe");
    let _ = std::fs::remove_file(&probe);
    let main = common::solid_main([0x11, 0x22, 0x33]);
    let wsz = common::zip_stored(&[
        ("../llamp-zipslip-probe", b"nope"),
        ("/tmp/llamp-zipslip-abs", b"nope"),
        ("main.bmp", &main),
    ]);
    let mut slot = llamp_skin::SkinSlot::new();
    let skin = slot
        .load_wsz(&wsz)
        .expect("slip entry is skipped, main remains");
    assert!(!probe.exists());
    assert!(!std::path::Path::new("/tmp/llamp-zipslip-abs").exists());
    assert!(
        skin.defects
            .iter()
            .any(|defect| defect.contains("zip-slip")),
        "{:?}",
        skin.defects
    );
    let only_slip = common::zip_stored(&[("../main.bmp", &main)]);
    let mut rejected = llamp_skin::SkinSlot::new();
    let err = rejected
        .load_wsz(&only_slip)
        .expect_err("only copy was rejected");
    assert!(err.to_string().contains("main.bmp"), "{err}");
    assert!(!probe.exists());
}

#[test]
fn declared_uncompressed_size_over_32_mib_is_rejected() {
    let payload = b"tiny";
    let mut wsz = common::zip_stored(&[("main.bmp", payload)]);
    common::patch_uncompressed_sizes(&mut wsz, 32 * 1024 * 1024 + 1);
    HUGE.store(false, Ordering::SeqCst);
    let mut slot = llamp_skin::SkinSlot::new();
    let err = slot.load_wsz(&wsz).expect_err("size cap");
    assert!(err.to_string().contains("uncompressed"), "{err}");
    assert!(!HUGE.load(Ordering::SeqCst), "allocated the declared bomb");
}

#[test]
fn compression_ratio_over_10000_is_rejected_before_inflate() {
    let payload = [0u8; 8];
    let mut wsz = common::zip_stored(&[("main.bmp", &payload)]);
    common::patch_uncompressed_sizes(&mut wsz, 8 * 10_000 + 8);
    HUGE.store(false, Ordering::SeqCst);
    let mut slot = llamp_skin::SkinSlot::new();
    let err = slot.load_wsz(&wsz).expect_err("ratio");
    assert!(err.to_string().contains("compression ratio"), "{err}");
    assert!(!HUGE.load(Ordering::SeqCst));
}

#[test]
fn deflate_method_loads() {
    let main = common::solid_main([0x22, 0x22, 0x22]);
    let wsz = common::zip_deflate("Skins/Foo/MAIN.BMP", &main);
    let mut slot = llamp_skin::SkinSlot::new();
    let skin = slot
        .load_wsz(&wsz)
        .expect("deflate and case and leading dir");
    assert_eq!(skin.sprite_px(Sprite::Main, 0, 0), [0x22, 0x22, 0x22, 255]);
}

#[test]
fn unsupported_compression_method_fails_the_skin() {
    let mut wsz = common::zip_stored(&[("main.bmp", b"xx")]);
    // method field is at local offset 8 and central offset 10.
    wsz[8] = 12;
    wsz[8 + 1] = 0;
    let central = wsz
        .windows(4)
        .position(|w| w == 0x0201_4B50u32.to_le_bytes())
        .expect("central");
    wsz[central + 10] = 12;
    wsz[central + 11] = 0;
    let mut slot = llamp_skin::SkinSlot::new();
    let previous = common::solid_main([1, 2, 3]);
    let good = common::zip_stored(&[("main.bmp", &previous)]);
    let id = slot.load_wsz(&good).expect("good").id;
    let err = slot.load_wsz(&wsz).expect_err("bzip");
    assert!(err.to_string().contains("compression"), "{err}");
    assert_eq!(slot.current().unwrap().id, id);
}

#[test]
fn shallowest_duplicate_wins_and_a_defect_is_recorded() {
    let shallow = common::solid_main([1, 2, 3]);
    let deep = common::solid_main([9, 9, 9]);
    let wsz = common::zip_stored(&[("dir/main.bmp", &deep), ("main.bmp", &shallow)]);
    let mut slot = llamp_skin::SkinSlot::new();
    let skin = slot.load_wsz(&wsz).expect("dup");
    assert_eq!(skin.sprite_px(Sprite::Main, 0, 0), [1, 2, 3, 255]);
    assert!(
        skin.defects.iter().any(|d| d.contains("duplicate")),
        "{:?}",
        skin.defects
    );
}

#[test]
fn play_button_is_not_at_the_contradicted_114_origin() {
    let rect = llamp_skin::control_rect(Control::Play);
    assert_ne!(rect.x, 114);
    assert_eq!(rect.x, 31);
    assert_eq!(rect.w, 23);
    assert_eq!(rect.h, 18);
}

struct HugeAlloc;

static HUGE: AtomicBool = AtomicBool::new(false);

unsafe impl std::alloc::GlobalAlloc for HugeAlloc {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        if layout.size() > 2_000_000 {
            HUGE.store(true, Ordering::SeqCst);
        }
        unsafe { std::alloc::System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: HugeAlloc = HugeAlloc;
