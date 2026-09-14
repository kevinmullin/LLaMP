# Skin format

LLaMP loads classic Winamp 2 `.wsz` skins. This spec is the contract `llamp-skin` implements and the shells consume. Behavior is taken from published skinning notes, not from the Winamp source tree. Sprite rectangles that those notes do not pin down are **fixture-locked** in phase 2: a locally supplied `.wsz` (not committed) must render to the golden PNG. A guessed rectangle that fails that PNG is a bug.

References we may read:

- The republished Winamp skin tutorial’s configuration chapter (viscolor, pledit, region), including the copy at <https://winampskins.neocities.org/config>.
- Archive Team’s file list: <http://justsolve.archiveteam.org/wiki/Winamp_Skin>.
- Webamp (MIT, `captbaritone/webamp`) for format behavior, with attribution if a comment in our parser is explaining a quirk we learned there. Do not copy Webamp source into this tree.

Do not use the 2024 Winamp source drop. Do not check in Nullsoft’s base skin or any third-party `.wsz`.

## Container

A `.wsz` is a ZIP archive. Also accept a directory of the same layout for our skin build.

- Compression methods: stored and deflate. Other methods: the skin fails to load, the previous skin stays, a defect is recorded.
- Filenames are matched case-insensitively. A leading directory is stripped (`Skins/Foo/main.bmp` is `main.bmp`). Nested duplicates: the shallowest path wins, and a defect is recorded.
- Reject zip-slip (`..` components, absolute paths). A rejected entry is skipped. If the rejection was the only copy of `main.bmp`, the skin fails to load.
- No path from the archive is opened on the filesystem. Parse in memory. Extracting a skin to a directory to blit it is a bug.
- Total uncompressed bytes for one skin are capped at 32 MiB. A declared size over the cap, or a inflate that would pass it, rejects the skin. The previous skin stays loaded.
- A sheet’s declared width and height are capped at 4096 on each axis. `width * height * 4` is checked for overflow before any allocation. An overflowing or over-cap sheet rejects the skin.
- These caps and the zip-slip rule are what phase 11’s user `.wsz` import inherits. Do not grow a separate importer that skips them.
- The llama mark is not a skin file. See [brand](brand.md).

## Files

Names are case-insensitive. BMP is required for classic sheets. PNG is accepted as an extension for skins we generate, decoded to the same RGBA. A classic skin that uses only BMP must still load.

| File | Role | If missing |
| --- | --- | --- |
| `main.bmp` | Main window background, 275×116 | Skin fails to load |
| `titlebar.bmp` | Titlebar, min, shade, close, clutterbar states | Defect; that chrome uses the built-in default skin’s sheet |
| `cbuttons.bmp` | Previous, play, pause, stop, next, eject | Defect; fallback sheet |
| `shufrep.bmp` | Shuffle, repeat, EQ and playlist toggles | Defect; fallback sheet |
| `posbar.bmp` | Seek bar and thumb | Defect; fallback sheet |
| `volume.bmp` | Volume slider | Defect; fallback sheet |
| `balance.bmp` | Balance slider | Defect; fallback sheet |
| `monoster.bmp` | Mono / stereo indicators | Defect; indicators hidden, defect recorded |
| `playpaus.bmp` | Play / pause / stop indicator by the time display | Defect; indicator hidden |
| `numbers.bmp` | Time digits | Defect; fallback digits |
| `nums_ex.bmp` | Alternate digits | Optional; ignore if absent |
| `text.bmp` | Bitmap font for the marquee and eligible playlist rows | Defect; marquee uses CoreText in skin colors (see text path) |
| `eqmain.bmp` | EQ window | EQ window uses fallback sheet |
| `eq_ex.bmp` | EQ extras (preamp, on/auto, graph chrome) | Those controls use fallback |
| `pledit.bmp` | Playlist frame, resize tiles, menu buttons | Playlist uses fallback |
| `pledit.txt` | Playlist and text colors | Defaults listed below |
| `viscolor.txt` | 24 visualizer colors | Defaults listed below |
| `region.txt` | Non-rectangular masks | Window is the full rectangle |
| `gen.bmp` | Frame for library, visualizer, lyrics | Those windows use fallback |
| `genex.bmp` | Buttons and sliders for gen windows | Those controls use fallback |

Cursors (`.cur`) may be present. Phase 2 records them. Phase 3 applies them only if the platform cursor API accepts them without a custom parser we have not specified. A failed cursor is a defect, not a failed skin.

`video.bmp`, `mb.bmp`, and AVS sheets are ignored. We do not implement the minibrowser or video window.

## Windows

Skin pixels are the coordinate space. The shell multiplies by an integer scale at draw time. See [ADR 005](../adr/005-skin-rendering.md).

| Window | Unshaded size | Shade height | Resize |
| --- | --- | --- | --- |
| Main | 275×116 | 14 | No |
| Equalizer | 275×116 | 14 | No |
| Playlist | minimum 275×116 | title-strip height, fixture-locked | +25 px horizontal, +29 px vertical |
| Library | minimum 275×116 | gen title-strip height, fixture-locked | gen frame; integer skin pixels, no fractional size |
| Visualizer | minimum 275×116, client area may grow freely | gen title-strip height | Free, including the wgpu client. Chrome tiles stay integer. |
| Lyrics | minimum 275×116 | gen title-strip height | Same as library |

Double-size doubles skin pixels, then the display integer scale (1, 2, or 4) multiplies. Nearest-neighbor only.

Shared behavior is in [ARCHITECTURE.md](../ARCHITECTURE.md): drag-anywhere except on controls, snap within 10 skin pixels, dock on mouse-up, undock after a 12 skin-pixel drag, always-on-top per window, docked group moves together and double-sizes together, group is on top if any member is.

### Controls the main window must hit-test

In skin pixels, implemented as entries in the control map (the same rects VoiceOver uses):

- Titlebar drag, minimize, shade, close.
- Clutterbar: O, A, I, D, V. Each is a button. Actions: O options (our preferences, not a Winamp clone menu), A always-on-top, I file info for the current track, D double-size, V visualizer cycle on the 76×16 pane. If a published tutorial assigns different letters, the fixture wins and this list is amended in the phase 2 notes. Do not invent a sixth clutterbar button.
- Transport: previous, play, pause, stop, next. Eject opens the importer. Pause and play may be one combined control if the sheet has a single play/pause sprite; the control map still exposes both actions to accessibility.
- Seek bar. Dragging seeks only if `supports_seek`.
- Volume and balance sliders.
- Mono and stereo indicators (display only, not buttons).
- EQ and playlist window toggles.
- Shuffle and repeat.
- Time digits: click toggles elapsed and remaining.
- Marquee: click cycles display (title, title — artist, filename) in that order.
- 76×16 visualizer pane: click cycles oscilloscope and spectrum analyzer.

The 76×16 pane’s origin inside the 275×116 window is fixture-locked. Do not hard-code a coordinate in the shell. The core publishes the rect in the control map.

### Playlist menus

The bottom edge has five slide-up buttons labeled Add, Rem, Sel, Misc, List. Contents are ours, under those labels:

- Add: files, folder, URL, playlist.
- Rem: selected, crop (remove unselected), all, remove from library (drops the row; does not delete the file). Moving a referenced file to Trash is not a menu item.
- Sel: all, none, invert.
- Misc: sort by title, artist, path; reverse; file info.
- List: new, save, load. Load and save use M3U8 by default and offer the other playlist types.

Menu chrome is sliced from `pledit.bmp`. Menu text uses the text path below, not a modern popup that ignores the skin. A platform menu is allowed only as a fallback if the slice is missing, and it must use `pledit.txt` colors.

### Library, visualizer, lyrics

These are not Winamp windows. They use `gen.bmp` / `genex.bmp` so they inherit the skin.

They must read as 1999 chrome: a title strip, a tiled frame, a client area, text in skin colors. No sidebar of SF Symbols, no card grid, no blurred artwork backdrop.

- **Library:** a tree (Artists, Albums, Folders, then one node per provider) and a detail list. Artwork is a square thumbnail in the row, not a hero image. Search is a single line. Empty state may stamp the brand mark at integer scale. See [brand](brand.md).
- **Visualizer:** gen chrome around the wgpu client. No extra toolbar beyond shade, close, fullscreen, and a preset menu drawn with genex sprites. Fullscreen hides the chrome. See [visualizer](visualizer.md).
- **Lyrics:** a scrolling client. Active line emphasized. See [lyrics](lyrics.md).

## Config files

### `viscolor.txt`

24 lines. Each line is `r,g,b` with an optional comment. Values 0–255. Extra tokens on a line are ignored. Fewer than 24 valid lines: the missing lines use the defaults below, and a defect is recorded. A comment or junk before the first color, if it causes the first line not to parse, is a defect and we use defaults for the unparsed prefix (published notes say a leading comment can make Winamp ignore the file; we parse line-by-line instead, and record the defect, so a leading comment does not discard a valid file).

Published roles (skin tutorial configuration chapter):

| Index | Role |
| --- | --- |
| 0 | Background |
| 1 | Background dots |
| 2–17 | Spectrum, 2 at the peak (top), 17 at the bottom |
| 18–22 | Oscilloscope, 18 at the trough, 22 at the crest |
| 23 | Peak-hold mark |

Defaults if the file is missing: index 0 is `0,0,0`, index 1 is `255,255,255`, indices 2–17 are a green-to-red ramp, 18–22 are green, 23 is white. The exact default ramp is fixed in the phase 2 golden test, not restated here as a second source of truth.

### `pledit.txt`

`key=value` lines. A `[Text]` section header is accepted and ignored. Unknown keys are ignored. Hex colors are `#RRGGBB`.

| Key | Meaning | Default |
| --- | --- | --- |
| `Normal` | Playlist text | `#00FF00` |
| `Current` | Current track text | `#FFFFFF` |
| `NormalBG` | Playlist background | `#000000` |
| `SelectedBG` | Selected row background | `#0000C0` |
| `MbFG` | Status line text (we use this for the gen-window status line) | `#00FF00` |
| `MbBG` | Status line background | `#000000` |
| `Font` | Preferred face for CoreText rows | a built-in pixel-appropriate face we ship, not “Comic Sans MS” even if the file asks for a face we cannot embed |

We do not embed the named font from the skin. We record the requested face and use our bundled pixel-appropriate face, tinted with these colors. Licensing a skin’s named font is out of scope. See [LEGAL.md](../LEGAL.md).

### `region.txt`

Sections: `[Normal]`, `[WindowShade]`, `[Equalizer]`, `[EqualizerWS]`. Other section names are kept if we recognize them later (`[Playlist]` is not classic; ignore unknown sections).

Each section has `NumPoints` and `PointList`.

- `NumPoints` is a comma-separated list of integers. Each integer `n` consumes the next `n` points and forms one polygon.
- `PointList` is comma-separated `x,y` pairs in skin pixels.

A polygon with fewer than 3 points is dropped. A point outside the window rectangle is clamped, and a defect is recorded. The window mask is the union of the polygons. If a section is missing, that mode uses the full rectangle.

Playlist and gen windows do not gain a classic region. They are rectangular in 1.0. A `region.txt` section we do not recognize does not change that.

## Bitmap text

`text.bmp` is a grid of glyphs. Cell size and which byte maps to which cell are fixture-locked in phase 2. The parser must not assume a proportional font.

Which surfaces use it:

| Surface | Font |
| --- | --- |
| Time digits | `numbers.bmp` / `nums_ex.bmp` sprites, not `text.bmp` |
| kbps and kHz readouts | digit sprites if the sheet has them; otherwise `text.bmp` |
| Marquee | `text.bmp` if every glyph exists; otherwise the whole marquee uses CoreText in `Normal` color |
| Playlist list | `text.bmp` if every glyph in the visible set exists; otherwise the whole visible list uses CoreText. Do not mix bitmap and CoreText in one row or across visible rows. |
| Library, browser, lyrics | CoreText |
| Menus that contain a missing glyph | CoreText |
| EQ curve numeric readouts, if any | sprites first, CoreText if the sheet has no digits |

CoreText uses a system face drawn into an offscreen buffer at the destination integer scale, nearest-neighbor onto the skin grid. Colors come from `pledit.txt`. Do not anti-alias CoreText onto the skin and then scale. Do not rasterize into a 5×7 cell and magnify. Blurry text on a sharp skin is a bug. Intra-row Mixed is rejected: a 5×7 CoreText CJK cell is not that character.

Karaoke word highlight is a CoreText concern. It is not forced through `text.bmp`. See [lyrics](lyrics.md).

## BMP defects

Parse `BITMAPFILEHEADER` and `BITMAPINFOHEADER`. Row stride is 4-byte aligned: `((width * bits_per_pixel + 31) / 32) * 4`. Do not treat a 4-bit image as one byte per pixel.

Support:

- 1, 4, 8, 24, and 32 bits per pixel, `BI_RGB`.
- 8-bit `BI_RLE8`.
- Bottom-up (positive height) and top-down (negative height).
- Palettes for indexed images.

Do not support, and do not crash on: `BI_RLE4`, JPEG-in-BMP, PNG-in-BMP, bitfields we do not understand. Skip that file, record a defect, use the fallback sheet.

32-bit pixels: ignore alpha. Classic skins are not alpha. Color key for sprite sheets that need transparency (buttons, sliders): the top-left pixel of that sheet. Documented as the convention we implement; phase 2 confirms it against the fixture. Window shape comes from `region.txt`, not from a color key on `main.bmp`.

Wrong size:

- `main.bmp` not 275×116: skin fails to load. Do not scale it.
- Any other sheet at an unexpected size: do not scale. Use the fallback sheet for that file and record a defect. “Unexpected” means it does not contain the sprite rectangles the atlas table requires. Until phase 2 locks those rectangles, “unexpected” means width or height is zero.

## In-memory `Skin`

Shells consume this. They do not see a ZIP or a BMP.

- `id` — stable hash of the archive bytes.
- `atlas` — one RGBA8 buffer, straight alpha (255 everywhere except color-keyed pixels, which are alpha 0).
- `sprites` — name to rectangle in the atlas. Names are an enum in the core, not free strings the shell invents. The enum covers every control this spec lists. Adding a sprite is a core change.
- `regions` — polygons per window mode, in skin pixels.
- `vis_colors` — 24 RGB triples.
- `playlist_colors` — the `pledit.txt` colors and the requested font name.
- `glyphs` — map from Unicode scalar (the subset the sheet encodes) to an atlas rectangle. Missing scalar means “use CoreText for this string.”
- `defects` — list of human-readable warnings. The shell may show them in the gen status line. It must not modal-dialog one defect per missing cursor.

The atlas is built once at load. Switching skins drops the previous atlas. A test in phase 11 asserts the old id is gone.

## Sprite rectangles

Published, and still verified by the phase 2 fixture:

- Main window background: 275×116 (`main.bmp`).
- Transport button cell: 23×18, published in secondary writeups of `cbuttons.bmp`. A writeup also places the play cell at x=114, which contradicts a 23 px stride if play is the second button. **The fixture wins.** Do not encode 114 in the shell.

Not pinned in the sources this spec is willing to treat as final: every other slice origin (titlebar buttons, volume thumb frames, `pledit.bmp` tile origins, `eqmain.bmp` slider slots, `text.bmp` cell size, the 76×16 origin). Phase 2 writes `docs/spec/skin-atlas.md` from the fixture, with a golden PNG. That file becomes the coordinate table. This spec stays the rules. Do not paste a coordinate table from Webamp or from another player’s repo to fill the gap.

## Our three skins

Sources live in `assets/skins/src/<name>/` as layered PNG (one layer per sheet, plus a text file for `pledit.txt`, `viscolor.txt`, `region.txt`). `xtask` emits indexed or 24-bit BMP and a `.wsz` into `assets/skins/dist/`. The app bundles the `.wsz` files only.

| Id | Intent |
| --- | --- |
| `grey` | Near-classic grey. Reads as a 1999 player. Not a copy of the Nullsoft base skin. |
| `dark` | Same layout, dark metal and amber text. |
| `contrast` | High contrast. CoreText surfaces meet the ratio in [TESTING.md](../TESTING.md). Bitmap text uses the lightest and darkest indices in the sheet, not mid-grey on mid-grey. |

Authors: us, in phase 11. Quality bar is checklist-complete and period-correct, not a famous skinner’s finish. No third-party BMP in those directories.

A skin build that emits a `main.bmp` other than 275×116 fails the xtask.

## Import

The user imports a `.wsz` by a file panel. We copy it into Application Support `Skins/` and load it. We do not write into the imported file. We do not upload it. Removing it from the skin list deletes our copy, not the user’s original if they still have one outside the app.
