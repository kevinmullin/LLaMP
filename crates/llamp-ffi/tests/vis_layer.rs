//! Layer lifetime: the shell owns the pointer. Use after invalidate is loud.

#[test]
fn resize_and_replace_then_stale_generation_fails_without_touching_the_old_pointer() {
    llamp_ffi::llamp_vis_surface_reset();
    let first = 0x1000 as *mut std::ffi::c_void;
    let second = 0x2000 as *mut std::ffi::c_void;
    assert_eq!(llamp_ffi::llamp_vis_surface_bind(first, 1), llamp_ffi::LLAMP_OK);
    assert_eq!(
        llamp_ffi::llamp_vis_surface_resize(320, 200, 1),
        llamp_ffi::LLAMP_OK
    );
    assert_eq!(llamp_ffi::llamp_vis_surface_bind(second, 2), llamp_ffi::LLAMP_OK);
    assert_eq!(
        llamp_ffi::llamp_vis_surface_resize(640, 400, 2),
        llamp_ffi::LLAMP_OK
    );
    assert_eq!(
        llamp_ffi::llamp_vis_surface_resize(320, 200, 1),
        llamp_ffi::LLAMP_ERR_INVALID,
        "old generation after replace must fail loudly"
    );
    assert_eq!(
        llamp_ffi::llamp_vis_surface_invalidate(2),
        llamp_ffi::LLAMP_OK
    );
    assert_eq!(
        llamp_ffi::llamp_vis_surface_resize(640, 400, 2),
        llamp_ffi::LLAMP_ERR_INVALID,
        "use after invalidate must fail loudly"
    );
    assert_eq!(
        llamp_ffi::llamp_vis_present(2),
        llamp_ffi::LLAMP_ERR_INVALID
    );
    assert_eq!(llamp_ffi::llamp_vis_surface_generation(), 0);
}
