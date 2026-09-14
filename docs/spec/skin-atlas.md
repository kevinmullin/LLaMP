# Skin atlas

Fixture-locked coordinates for the synthetic skin in `assets/skins/src/fixture/`. This file is the table. `skin-format.md` stays the rules. Do not paste a coordinate table from another player here.

The idle blit is unshaded. It draws the normal or off sprite, the time string `0:00`, and the marquee string `FIXTURE`. The 76×16 pane is not overpainted. A missing glyph is not drawn here; platform text is a shell concern.

Play’s cell is the second transport button, so its sheet origin is x=24, not 114.

## Golden PNG

This is the byte contract phases 3, 13, and 14 compare against. `crates/llamp-skin/tests/fixtures/golden/main-275x116.png`.

- 275×116.
- RGBA8, straight alpha. Color-keyed pixels are alpha 0. Everything else in the blit is alpha 255, except pixels outside the `[Normal]` region, which are alpha 0.
- Row-major, top to bottom, no row padding. Channel order is R, G, B, A.
- PNG color type 6, bit depth 8.
- An `sRGB` chunk (perceptual intent). No `iCCP`, no `gAMA`, no `cHRM`. No display profile.
- The 2× image is these pixels doubled, nearest-neighbor. It is not a second file.

## Windows

Control rects are skin pixels. Shade height is 14. The vis pane origin is not hard-coded in a shell; the core publishes it.

| Control | x | y | w | h |
| --- | --- | --- | --- | --- |
| Titlebar | 0 | 0 | 275 | 14 |
| Minimize | 244 | 3 | 9 | 9 |
| Shade | 254 | 3 | 9 | 9 |
| Close | 264 | 3 | 9 | 9 |
| Clutter O, A, I, D, V | 10, 20, 30, 40, 50 | 3 | 8 | 8 |
| Mono | 8 | 22 | 28 | 12 |
| Stereo | 40 | 22 | 28 | 12 |
| Time `0:00` | 72 | 22 | 36 | 13 |
| Stop indicator (idle) | 112 | 24 | 9 | 9 |
| Marquee `FIXTURE` | 8 | 42 | 35 | 7 |
| Vis pane | 24 | 52 | 76 | 16 |
| Volume track | 90 | 70 | 68 | 14 |
| Volume thumb | 90 | 72 | 14 | 11 |
| Balance track | 166 | 70 | 68 | 14 |
| Balance thumb | 193 | 72 | 14 | 11 |
| Seek bar | 8 | 86 | 248 | 10 |
| Seek thumb | 8 | 86 | 29 | 10 |
| Previous, play, pause, stop, next, eject | 8, 31, 54, 77, 100, 123 | 98 | 23 | 18 |
| Shuffle, repeat, EQ, playlist | 156, 179, 202, 225 | 98 | 23 | 12 |

## Sheets

Color key is the top-left pixel of every sheet except `main.bmp`. It is `#FF00FF` in this fixture. `main.bmp` is not keyed. A sheet smaller than the minimum size uses the fallback fill `#808080` and records a defect. The fallback is not a second authored skin.

| Sheet | Minimum | Keyed | Notes |
| --- | --- | --- | --- |
| `main.bmp` | 275×116 | no | Solid `#1A1A1A`. Not 275×116 rejects the skin. |
| `titlebar.bmp` | 275×28 | yes | Normal strip y=0, pressed strip y=14. Pixel (0,0) stays the key. Bar `#4A4A4A`. |
| `cbuttons.bmp` | 139×37 | yes | Six 23×18 cells at x=1+i·23. Normal y=1, pressed y=19. Play is the second cell. |
| `shufrep.bmp` | 93×25 | yes | Four 23×12 cells. Off y=1, on y=13. Order: shuffle, repeat, EQ, playlist. |
| `posbar.bmp` | 249×23 | yes | Bar (1,1) 248×10. Thumb (1,12) 29×10. |
| `volume.bmp` | 69×28 | yes | Track (1,1) 68×14. Thumb (1,16) 14×11. |
| `balance.bmp` | 69×28 | yes | Same slots, different colors. |
| `monoster.bmp` | 58×13 | yes | Mono (1,1) 28×12. Stereo (30,1) 28×12. |
| `playpaus.bmp` | 30×10 | yes | Play (1,1), pause (11,1), stop (21,1), each 9×9. Idle draws stop. |
| `numbers.bmp` | 118×14 | yes | 13 cells of 9×13 at x=1+i·9, y=1. Order: `0-9`, `:`, `-`, blank. Cell background `#101010`. |
| `text.bmp` | 80×42 | yes | 5×7 cells, 16 per row, index 0 = U+0020. Ink `#F0F0F0`. Missing scalar means platform text. |

Titlebar button slots, both rows (pressed y is 17): clutter 8×8 at x=10, 20, 30, 40, 50; minimize, shade, close 9×9 at x=244, 254, 264. The idle blit draws the titlebar strip, not the button sprites on top of it. The sprites exist so a later press state can swap them.

EQ, playlist, and gen sheet origins are not in this table. They are not guessed.
