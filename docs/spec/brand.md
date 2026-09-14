# Brand

The product mark is the pixel llama in headphones supplied by the owner. Canonical file: `assets/brand/llamp-mark.png`. It was copied unchanged. Do not redraw it. Do not replace it with Meta’s llama. Do not use Winamp’s “it really whips the llama’s ass” splash, llama, or any Nullsoft artwork.

The letters “LLaMP” may change at the [ADR 012](../adr/012-working-name.md) gate. The sprite stays.

## What the file is

A PNG, RGBA, 512×384, black field, cream face, slate-blue earcups, one green eye and one white eye, small horns. If a later export has a soft fringe, quantize to hard pixels before the icon build so nearest-neighbor scaling does not halo. Quantize in a build step. Do not overwrite the canonical file with a “cleaned up” painting.

## Where it appears

- Bundle icon, integer sizes 16, 32, 128, 256, 512, and 1024, centered on a flat black field with padding so the headphones are not clipped by the macOS icon mask.
- DMG background and volume icon, same sprite, integer scale.
- Sparkle update prompt, if Sparkle allows a custom image; otherwise the bundle icon is enough.
- About window: a `gen`-skinned window, the mark at 2× or 4× nearest-neighbor, the display name, and the line “compatible with classic Winamp skins.” No other Winamp artwork.
- Empty states for the library and lyrics windows, a small integer-scaled stamp. Not a watermark on a playing session.

## Where it must not appear

- `main.bmp`, `eqmain.bmp`, `pledit.bmp`, `gen.bmp`, or any other sheet of a skin we ship.
- A user-imported `.wsz`. Loading a skin must not composite the llama into the chrome.
- The 76×16 visualizer pane.
- The marquee.
- A running visualizer preset, unless that preset is explicitly the “about” preset and phase 11 decides not to bother. Default is no.

The macOS squircle mask applies to the bundle icon only. In-app drawing is the raw sprite on transparency or on the gen client background. Do not pre-mask the in-app stamp into a squircle.

## Scale

Integer nearest-neighbor only: 1, 2, 4, and the icon sizes above which are integer multiples of a grid-snapped master. If 16×16 cannot contain the headphones without becoming noise, the 16 px icon is the face cropped to the eyes and earcups, still nearest-neighbor, still from this file, not a new drawing. Document the crop rectangle in the icon build once it exists. Do not invent a second mascot for the menu bar.

## Name beside the mark

Until the rename gate passes, the words next to the mark are “LLaMP” and, in About only, “Large Llama Music Player”. Do not set a Meta “Built with Llama” credit. We do not ship a Llama model. Do not imply affiliation with Meta, with Winamp Group, or with the Llammp project.
