# wgpu surface from a shell-owned CAMetalLayer

Date: 2026-09-14. Phase 7a. wgpu **30.0.1**.

## What ran

Rust integration test `crates/llamp-ffi/tests/vis_layer.rs` binds two fake layer pointers with generations `1` then `2`, resizes each, then uses the old generation and the invalidated generation. Both fail with `LLAMP_ERR_INVALID`. The core does not dereference either pointer on those paths.

`llamp_vis_surface_invalidate` drops the optional wgpu surface (and therefore any Metal retain wgpu held) before the slot forgets the pointers. That is the destroy-before-release order.

## Generation and invalidate

Achievable. The shell passes a generation it owns. After replace or invalidate, a call with the old token returns `LLAMP_ERR_INVALID` and does not load the stored address. That is the loud failure for a forgotten teardown **that still talks to the core**.

Not achievable: if the shell releases the layer and never calls again, the core cannot notice. A raw `void*` has no validity probe that is not itself a use-after-free. We do not retain the layer (ADR 003: the core does not own it). The finding is that protocol violations are loud; a silent release with no further FFI call is undetectable.

## Resize and layer replacement

Resize with a live generation stores the new pixel size and, if a wgpu surface exists, reconfigures it. Layer replacement is a new `bind` with a new generation: the previous `Gpu` is dropped first, then the new pointer is stored.

## wgpu 30 does not take a CAMetalLayer

The plan and ADR 003 name an inward `CAMetalLayer` pointer. wgpu **30.0.1** `SurfaceTargetUnsafe` is `RawHandle` (plus `Drm`). There is no `CoreAnimationLayer` variant. Embedding is an `NSView` via `raw-window-handle` `AppKitWindowHandle`.

The core still must not treat the layer and the view as the same object. `llamp_vis_surface_bind` stores the layer. `llamp_vis_surface_bind_view` opens wgpu from the view. Both share the generation. Invalidate drops the surface, then forgets both pointers.

Pin written into [visualizer.md](../spec/visualizer.md): **30.0.1**.
