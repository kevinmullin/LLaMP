use std::ffi::CStr;

#[test]
fn every_control_has_a_non_empty_label_and_the_idle_blit_is_the_main_window() {
    let wsz = xtask::fixture_wsz();
    assert_eq!(llamp_ffi::llamp_skin_load(wsz.as_ptr(), wsz.len()), llamp_ffi::LLAMP_OK);
    let image = llamp_ffi::llamp_skin_blit_main();
    assert_eq!(image.width, 275);
    assert_eq!(image.height, 116);
    assert_eq!(image.len, 275 * 116 * 4);
    assert!(!image.data.is_null());
    let rgba = unsafe { std::slice::from_raw_parts(image.data, image.len) };
    assert_eq!(&rgba[(13 * 275 + 8) * 4..][..4], &[0x4A, 0x4A, 0x4A, 255]);
    llamp_ffi::llamp_image_free(image.data, image.len);

    let count = llamp_ffi::llamp_control_count();
    assert!(count > 0);
    for index in 0..count {
        let control = llamp_ffi::llamp_control_at(index);
        assert!(!control.label.is_null(), "control {index}");
        let label = unsafe { CStr::from_ptr(control.label) };
        assert!(!label.to_bytes().is_empty(), "control {index}");
        assert!(control.w > 0 && control.h > 0);
    }
    assert_eq!(llamp_ffi::llamp_shade_height(), 14);
    let size = llamp_ffi::llamp_main_size();
    assert_eq!((size.width, size.height), (275, 116));
    assert_eq!(llamp_ffi::llamp_region_polygon_count(0), 1);
    assert_eq!(llamp_ffi::llamp_region_point_count(0, 0), 4);
}
