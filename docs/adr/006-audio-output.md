# ADR 006 — Output trait, cpal on macOS, no hog mode

- Status: accepted
- Date: 2026-09-13

## Context

macOS 1.0 must play to CoreAudio, recover when the default device changes, and not glitch when the buffer underruns. Windows later needs WASAPI shared and exclusive. Linux later needs PipeWire with an ALSA fallback.

`cpal` 0.18 is the maintained cross-platform crate. It is still pre-1.0. Its CoreAudio backend reports device-changed and stream-invalidated errors. It does not expose WASAPI exclusive in a way we should depend on for the macOS design. macOS “hog” mode takes the device away from every other app. That is the wrong default for a player that sits beside a browser.

## Decision

`llamp-audio` defines an `Output` trait: enumerate devices, open a stream at a requested rate and channel count, deliver a callback of a given frame count, and surface underrun and device-loss as values the session can handle. The trait is the seam. `cpal` is the macOS 1.0 backend, not the architecture.

macOS opens a shared float stream. Hog / exclusive is not implemented on macOS. If the device cannot do float, we accept the integer format the device offers and dither at the edge. See [audio pipeline](../spec/audio-pipeline.md).

Phase 1 includes a hot-swap and underrun spike. If `cpal` 0.18 cannot recover from a default-device change without a dropout we consider a bug, we replace the backend with `coreaudio-rs` and amend this ADR. We do not write a CoreAudio backend speculatively before that spike fails.

WASAPI shared plus exclusive, and PipeWire plus ALSA, are later implementations of `Output`. They are not a reason to avoid `cpal` on macOS now. Exclusive mode, when it exists on Windows, is opt-in and must release the device on a pause longer than five seconds.

## Consequences

- A pre-1.0 crate is on the critical path. The trait is how we delete it if a release breaks the callback contract.
- Device hot-swap is a phase 1 exit, not a polish item.
- We do not promise bit-perfect exclusive output on macOS. Shared float is the product.

## Alternatives

- **Call `coreaudio-rs` immediately.** Rejected as the starting point. It is more code before we know `cpal` is insufficient. It remains the documented fallback.
- **WASAPI-style exclusive on macOS from day one.** Rejected. Hog mode is hostile, and classic use is a shared desktop player.
- **PortAudio.** Rejected. Another C dependency and another callback model, without a better story for the trait we need to own anyway.
