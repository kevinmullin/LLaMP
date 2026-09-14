# ADR 009 — wgpu for the visualizer surface only

- Status: accepted
- Date: 2026-09-13

## Context

Skin chrome is pixel art and is drawn natively per platform so it can stay on the integer-scale path without a GPU library. Visualizers are not pixel art. Reimplementing them in Metal, D3D12, and Vulkan would triplicate the only part of the UI that is not a sprite sheet.

The risk is coupling. If the core opens a GPU device to draw a window, every shell and every headless test inherits that device. A misbehaving visualizer must not be able to stall the audio callback.

## Decision

wgpu is used for the standalone visualizer’s client area, and nowhere else. On macOS it targets Metal. The native shell embeds the wgpu surface in a view it owns (an `NSView` subclass). The shell still draws `gen` chrome around that view with the atlas blit.

The core does not open a wgpu device to parse a skin, draw the main window, or run the 76×16 pane. That pane is a CPU blit of a 76×16 bitmap filled from `viscolor.txt`.

A hidden, occluded, or background visualizer submits no frames. The default cost of a closed visualizer window is zero GPU time.

User-installed plugins do not receive the device. They may return RGBA for the host to upload. First-party visualizers may use wgpu. See [ADR 004](004-plugin-runtime.md).

## Consequences

- One portable shading language (WGSL) for the two built-in full-window visualizers.
- The core’s visualizer crate is the one place Rust talks to a GPU. Headless tests of decode and skin do not link a requirement to display that surface.
- projectM does not fit this path. It is OpenGL. It is not pulled in to “just render to a texture” beside wgpu on macOS, where GL and Metal interop is a second project. See [ADR 010](010-milkdrop.md).
- Embedding wgpu in an `NSView` is platform glue in the shell, not a second skin renderer. The exact embedding calls are written in phase 7 against the wgpu version pinned then. They are not invented here.

## Alternatives

- **Fully native Metal, and D3D12 and Vulkan later.** Rejected for the visualizer body. Three shader pipelines for a feature that is explicitly not pixel-art chrome. Metal-only would also make the Windows visualizer a rewrite.
- **wgpu for all chrome.** Rejected. It collapses the core/shell drawing split and makes integer-scale skin tests depend on a GPU.
- **CPU only for all visualizers.** Rejected for the standalone window. The 76×16 pane stays CPU. A fullscreen preset that is a software rasterizer will miss the frame budget we are about to enforce.
