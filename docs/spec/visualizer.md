# Visualizer

Two surfaces. They do not share a renderer.

1. The 76×16 pane in the main window. Oscilloscope and spectrum analyzer, cycled by clicking. CPU blit. `viscolor.txt`. Pixel-exact, cheap. This pane exists in classic skins and stays that way.
2. A separate window, `gen` chrome, wgpu client, fullscreen without chrome. Plugin visualizers live here. See [ADR 009](../adr/009-visualizer-gpu.md) and [ADR 010](../adr/010-milkdrop.md). wgpu **30.0.1**. Embedding is `SurfaceTargetUnsafe::RawHandle` from the shell-owned `NSView`. The `CAMetalLayer` still crosses inward as `void*` for the generation table; wgpu 30 has no `CoreAnimationLayer` variant. See [wgpu-layer-lifetime](../investigations/wgpu-layer-lifetime.md).

A Tier B session has no PCM. Both surfaces use the non-reactive state in [providers](providers.md). They do not invent a spectrum from metadata.

## 76×16 pane

The core publishes the pane’s rectangle in skin pixels (fixture-locked). The shell blits a 76×16 RGBA buffer the analysis path fills.

- Background: `viscolor.txt` index 0.
- Dots: index 1. A sparse grid is enough. Do not animate the dots on the CPU if a static grid matches the golden image.
- Spectrum: indices 2 (top) through 17 (bottom). Peak-hold dots use index 23.
- Oscilloscope: indices 18–22.

Bar geometry is not specified as a bar count here. Phase 2–3 locks it with a golden image. Decay of the peak-hold is locked by that same image. Do not import constants from Winamp source or from another player’s renderer.

Click cycles scope and analyzer. The mode is persisted per skin? No. It is a session preference, not a skin setting. The skin only supplies colors.

This pane does not use wgpu. It does not run a plugin.

## Standalone window

`gen` / `genex` chrome. Client area is the wgpu surface on an `NSView` the shell embeds. Minimum window 275×116. The client may be any integer size. Fullscreen hides chrome and sets the surface to the display’s pixel size. Exit fullscreen on Escape and on the gen close control if it is visible; in fullscreen, Escape is enough, and a triple-click or a reserved key is not required.

Preset menu lists first-party presets. It does not list `.milk` or `.avs` files. If the user drops a `.milk` file, the status line says Milkdrop presets are not supported. It does not crash and does not half-parse the file.

## Frame packet

The analysis thread writes a snapshot. The visualizer **pulls** the latest snapshot when it draws. The host does not call into the plugin from the audio callback, and it does not call into the plugin from the analysis thread.

Cadence: the visualizer’s display loop, which is vsync when the window is visible and focused, capped at 60 frames per second even if the display is faster. On battery and unfocused, cap 30. Hidden, occluded, or the app in the background: zero frames, the surface is not presented, plugin `draw` is not called.

A mis-timed plugin cannot enlarge that cadence. The host calls `draw` and enforces the budget.

Packet fields (POD, copied for the call):

| Field | Definition |
| --- | --- |
| PCM | Interleaved stereo `f32`, the last 1024 samples at the device rate, post-DSP. If fewer exist, the prefix is zero. |
| FFT linear | 513 magnitude bins from the 1024-point Hann FFT. Nyquist is included (`WINDOW/2 + 1`). That is the existing `FftTap` choice, fixed in `frame_packet.rs`. Linear in frequency. |
| FFT bands | Log-spaced bands, count matching whatever the 76×16 golden image settled, plus the same peak-hold values. A plugin may ignore these and rebin the linear FFT. |
| RMS, peak | Per channel, the hop that produced this FFT. |
| Onset | 0–1 confidence. The detector is a spectral-flux threshold tuned in phase 7 against a fixture click track. Until that fixture exists, the field is present and may be 0. Do not ship a random “beat” to make presets bounce. |
| Metadata | Title, artist, album, duration frames, position frames. Strings are UTF-8 copies. A plugin must not retain them. |
| Flags | The session capability flags, so a preset can show the remote state itself if it wants. The host still forces the non-reactive path when `produces_pcm` is false, and does not call a reactive preset. |

Push versus pull: the analysis thread pushes into the seqlock (that is the push). The visualizer pulls. Nobody blocks the callback for a pull.

## Sandbox and budget

Native first-party visualizers draw with wgpu. User WASM visualizers return RGBA8 of the client size, capped at 1280×720. Above that, the host scales the returned buffer with nearest-neighbor. We do not ask a sandboxed plugin for a 5K buffer.

Budget: 8 ms of CPU on the render thread for `draw`, measured around the call. A GPU stall that makes present miss vsync twice in a row counts as a miss. Three misses in five seconds kill the instance, show “Visualizer stopped — it missed its frame budget”, and keep audio running. A test double that sleeps 50 ms must trip this without an audio underrun attributable to the kill.

Killing a visualizer does not unload the skin and does not stop playback.

## Presets

v1 format: a directory with `preset.json` and one or more `.wgsl` files.

`preset.json` fields: `name`, `author`, `llamp_preset` (must be `1`). Shader entry points are `vs_main` and `fs_main`.

We ship two presets we wrote. They must move when the onset field is non-zero and stay coherent when it is zero. They must not require network or a file outside the preset directory.

Milkdrop and AVS: see [ADR 010](../adr/010-milkdrop.md). Not in v1.

## Performance and battery

| State | Target |
| --- | --- |
| Visible, focused, on power | Vsync, cap 60 |
| Visible, unfocused, or on battery | Cap 30 |
| Occluded, minimized, or background | 0 GPU submissions |
| Window closed | The device for this surface is released |

The 76×16 pane still updates while audio plays, including when the big window is closed. That blit is a few kilobytes. It is not a reason to keep wgpu alive.

A laptop with the visualizer window open and unfocused should not sit at full GPU clocks. The 30 fps cap is the policy. Phase 7 verifies with a sample of GPU time, not with a feeling.

## Process taps

Not a feature. A future spike may ask whether a permissioned Core Audio process tap can feed this packet for Tier B. Default recommendation is no. If it ever ships, it is opt-in, has a visible indicator the entire time the tap runs, and is not implied by opening the visualizer on a Spotify track. See [PLAN.md](../PLAN.md).
