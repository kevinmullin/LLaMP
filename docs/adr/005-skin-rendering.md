# ADR 005 — Skin atlas in the core, integer scale in the shell

- Status: accepted
- Date: 2026-09-13
- Amended: 2026-09-13. The automated compare is a pinned-color-space backing-store capture, not a window-server screenshot. macOS backing scale is 1 or 2.

## Context

Classic skins are ZIP files of BMP sprite sheets and a few text files. Every shell must draw the same slices. If each shell parses BMP, they will disagree on RLE, padding, and color keys, and the golden test will not match the window.

Skins are 1× pixel art. A linear magnification filter on the layer we draw will blur them. On macOS the backing scale is 1 or 2. A scaled display mode resamples that backing store in the window server. The app cannot stop that softness, and it is not the blur bug this ADR exists to refuse. Fractional DPI letterbox is a Windows rule, written in phase 13.

## Decision

`llamp-skin` parses the `.wsz`, decodes BMP, and slices sheets into one RGBA atlas plus a sprite table, region polygons, `viscolor.txt`, and `pledit.txt` colors. That `Skin` value is what the FFI returns, as an atlas buffer and tables of rectangles. A shell never parses a BMP.

A CPU reference blit in `llamp-skin` renders a window to an RGBA buffer. Golden tests use that blit. Shells must match it at 1×.

Scale factors the shell may use: 1, 2, and 4, nearest-neighbor. The layer’s magnification filter is nearest. Double-size doubles the skin-pixel map and then the display integer scale multiplies. On macOS the backing factor is 1 or 2. The testable surface is that backing store. A scaled panel may still look soft after the window server resamples a correct 2× store. That is not a failed phase 3 exit. On Windows, a fractional OS scale picks the nearest integer factor and letterboxes. Bilinear, trilinear, and mipmaps are bugs for skin chrome we draw.

`main.bmp` that is not 275×116 is a failed skin, not a scaled one. Stretching it would be the blur bug in another costume.

The llama mark is not part of a skin. See [brand spec](../spec/brand.md).

## Consequences

- One parser, three shells.
- Golden PNGs do not need a display or a GPU.
- Shell bugs that blit differently from the reference still need a capture check. Phase 3’s exit renders the content view into an `NSBitmapImageRep` with a pinned sRGB color space and no display profile, and compares that to the phase 2 PNG. A window-server screenshot is a manual check only. Color management will not equal a raw RGBA golden, and weakening the tolerance to hide that is not allowed.
- We own BMP edge cases (8-bit, 24-bit, `BI_RLE8`, bottom-up rows, 4-byte stride). That work is in phase 2, not smeared across shells.

## Alternatives

- **Each shell parses BMP.** Rejected. Three parsers, three bugs, and the core cannot render a PNG in phase 2.
- **Draw skin chrome with wgpu.** Rejected. The GPU path is quarantined to the visualizer surface. See [ADR 009](009-visualizer-gpu.md). Putting the chrome there couples every window to a GPU library and makes “no GPU” headless tests harder than a CPU blit.
- **Let AppKit scale the window to the backing factor with the default filter.** Rejected. On a 2× display that can be made correct only if we set nearest filtering and an integer contents scale. The failing test is a bilinear sample in the backing store we drew. Softness on a scaled panel, after a correct 2× store, is the window server. It is not a reason to bilinear-filter the skin.
