#[test]
fn occluded_present_submits_zero_gpu_work() {
    llamp_ffi::llamp_vis_surface_reset();
    let layer = 0x3000 as *mut std::ffi::c_void;
    assert_eq!(llamp_ffi::llamp_vis_surface_bind(layer, 7), llamp_ffi::LLAMP_OK);
    llamp_ffi::llamp_vis_set_occluded(0);
    assert_eq!(llamp_ffi::llamp_vis_present(7), llamp_ffi::LLAMP_OK);
    assert_eq!(llamp_ffi::llamp_vis_gpu_submits(), 1);
    llamp_ffi::llamp_vis_set_occluded(1);
    assert_eq!(llamp_ffi::llamp_vis_present(7), llamp_ffi::LLAMP_OK);
    assert_eq!(
        llamp_ffi::llamp_vis_gpu_submits(),
        1,
        "occluded present must not increment the submit counter"
    );
    llamp_ffi::llamp_vis_set_occluded(0);
    assert_eq!(llamp_ffi::llamp_vis_present(7), llamp_ffi::LLAMP_OK);
    assert_eq!(llamp_ffi::llamp_vis_gpu_submits(), 2);
}

#[test]
fn first_party_presets_are_wgsl_plus_json_header() {
    let presets = llamp_ffi::first_party_presets();
    assert_eq!(presets[0].name, "Flux");
    assert_eq!(presets[1].name, "Pulse");
    for preset in presets {
        assert!(preset.wgsl.contains("fn vs_main"));
        assert!(preset.wgsl.contains("fn fs_main"));
        assert_eq!(preset.author, "LLaMP");
    }
    assert_eq!(llamp_ffi::llamp_vis_preset_count(), 2);
}
