# Testing

Tests exist to lock the exit criteria in [PLAN.md](PLAN.md). A phase is not done because the window “looks right” on one machine.

## Layers

| Layer | Where | Runs in CI |
| --- | --- | --- |
| Unit | crates | Yes, `cargo test` |
| Decoder and DSP fixtures | `llamp-audio`, fixtures not necessarily large binaries in git | Yes, small generated tones; compressed fixtures via a script the developer runs if the binary is not redistributable |
| Skin golden PNG | `llamp-skin` CPU blit | Yes, once a fixture we are allowed to commit exists. Until then, the developer’s local `.wsz` is a manual gate and the committed fixture is one of our generated skins from phase 11. Phase 2 may commit a tiny synthetic `.wsz` we authored, not a community skin. |
| Plugin capability | `llamp-plugin-host` | Yes |
| Realtime allocation | audio callback | Yes |
| Shell screenshot | macOS UI test | Phase 3 onward, on the macOS runner, a small set. Full VoiceOver and notarization are not this job. |

Do not commit a third-party `.wsz` or a commercial song as a fixture.

## Golden images

The reference renderer is the CPU blit in `llamp-skin`. It writes RGBA. Tests compare that buffer to a committed PNG.

Required goldens, added in the phase that first can produce them:

- Main window, 275×116, our synthetic fixture skin, at 1×.
- The same buffer nearest-neighbor scaled to 2×. A test that samples an edge pixel fails if a linear filter was used. Comparing against a separately blurred PNG is the wrong test; the 2× image must be exactly the 1× pixels doubled.
- EQ window at 1×, once `eqmain` slices exist.
- Playlist at 275×116 and at 275+25 by 116+29, to prove the step.
- 76×16 spectrum and scope, colors from a known `viscolor.txt`. Bar geometry is whatever this golden locks. Change the golden only with a deliberate fixture update, not a drive-by.

Shell screenshot in phase 3: render the content view into an `NSBitmapImageRep` with a pinned sRGB color space and no display profile, and compare that to the CPU golden. At 2× backing scale, compare to the doubled golden. A window-server screenshot is a manual check only. Do not compare window-server bytes to a raw RGBA golden. Pixel difference allowed: 0 on the sprite interior. One platform (macOS) in CI. Windows and Linux repeat the same comparison in their phases, against the layer they draw, not against a compositor resample. Softness from a scaled macOS display mode is not a failed exit. Bilinear filtering in the backing store is.

## Decoder conformance

Generated fixtures in tests:

- A one-second sine at 440 Hz, encoded as WAV. Decode must match within a tight epsilon after the format’s own dither.
- FLAC of that WAV. Bit-exact to the WAV decode where the encoder is lossless.
- MP3 and Vorbis: not bit-exact. A correlation or SNR floor against the source sine, set in phase 1 from the first good encode, then frozen. The floor exists to catch a decoder that returns silence or noise, not to certify a codec lab.
- Opus via `libopus`: the adapter returns samples, a test plays the round-trip through the ring without an allocation in the callback.
- Seek: request a frame, assert the next delivered frame index is within one device period.
- Gapless: two generated files with known encoder delay (where the format supports it). The joined PCM length matches the trimmed sum within one sample. FLAC, Vorbis, ALAC, and Opus are in this test. MP3 is in it only if the phase 1 investigation shows LAME/Xing delay is surfaced. AAC-LC is not in this test.

We do not claim ITU conformance. We claim we notice a broken pipeline.

## Hostile skins

This is a standing requirement, not a phase 2 checkbox. `.wsz` files stay untrusted after 1.0. Phase 11 import uses this parser. Do not add a second importer that skips these checks. A change that removes a cap, a fuzz target, or the “no archive path is a filesystem path” rule fails review even if the feature tests pass.

- Zip-slip (`..`, absolute paths) is rejected. No archive path is opened on the filesystem.
- Total uncompressed bytes for one skin stay capped at 32 MiB ([skin format](spec/skin-format.md)). A declared size over the cap, or an inflate that would pass it, rejects the skin before that buffer is allocated.
- Declared uncompressed size divided by compressed size must be at most 10000. Over that, reject before inflate. A stored sheet has ratio 1. A flat BMP may deflate well under this. A bomb does not.
- A sheet’s declared width and height stay capped at 4096 on each axis. `width * height * 4` is checked for overflow before any pixel allocation. An overflowing or over-cap sheet rejects the skin.
- A compression bomb and a `BI_RLE8` decompression bomb are rejected without exceeding those caps, and without panicking.
- `cargo-fuzz` targets `zip_reader` and `bmp_decoder` in `fuzz/` must remain. Run them with nightly `cargo fuzz` when a crash is suspected. They are not a one-time green check.

## Library

- Grant a folder and play a file in place. The test asserts the path on disk is the granted path.
- A missing file leaves the row, marked missing.
- A v1 fixture database upgrades to v2 without losing rows (`user_version` and ordered scripts).

## Budgets

Numbers are in [PLAN.md](PLAN.md). Missing one fails the phase. Do not raise a number in the same change that misses it.

- Bundle size: CI, size of the unzipped `.app`, fail over the phase 3 or phase 12 cap.
- Launch-to-window: CI, timed subprocess, fail over 400 ms at phase 3. Launch-to-audible is the phase 12 cap (800 ms).
- Idle CPU and RSS: a phase-exit script on the `macos-26` runner, at phase 3 and phase 12. If that runner is too noisy to be fair, write how we measure. Do not raise the number.

## Realtime

An allocator hook (a global allocator in the test binary that flags if the audio callback’s thread calls `alloc`) fails the test on allocation. The callback under test runs on a named thread so the hook knows it is in the callback. Lock detection: the callback path must not call a function that takes a mutex; review plus a debug-only mutex wrapper that aborts on that thread. I/O: no `std::fs` or network types reachable from the callback module. The module boundary is the structural test; the hook is the runtime test.

DSP and EQ coefficient updates are called from that callback in the test. A slider slew that allocates fails CI.

## Plugins

- Manifest with an unknown kind is refused.
- Semver range that excludes the host is refused.
- WASM lyrics fixture that requests a host other than its declared host is denied, and the test asserts no socket was opened (the host function is the only HTTP path, and the test double records calls).
- WASM fixture that reads a path it was not granted is denied.
- Visualizer test double that sleeps past the budget is killed. The audio callback’s underrun counter does not increase because of that kill. Use a fake clock if sleeping in CI is flaky; the contract is “budget exceeded,” not “we slept on a shared runner.”

## Lyrics

- Parser table: multiple timestamps, `[offset:]`, bad line skipped, enhanced word times.
- Clock test: seek to just before and just after a line boundary, using the sample clock, not wall time. Pause does not advance the line.
- Opt-in: with lookup declined, a track change issues no HTTP call (test double).

## Accessibility and contrast

High-contrast skin: CoreText colors from that skin’s `pledit.txt` meet a 7:1 contrast ratio for `Normal` on `NormalBG` and `Current` on `NormalBG`. Computed in a unit test from the skin’s text file, not eyedropped from a screenshot.

VoiceOver: phase 3 and phase 12 manual scripts in this file’s checklist, not an automated XCTest we do not yet trust. The automated part is that every control-map entry has a non-empty accessibility label string in the FFI snapshot. A control with an empty label fails the test.

Manual script (phase 12, clean account):

- VoiceOver on, main window focused, rotor lists play, pause, seek, volume.
- Each of the six windows appears as a window with a title.
- Keyboard space toggles play without VoiceOver clicks.

## What is not automated

- Full VoiceOver behavior (labels are automated; speech and rotor layout are manual).
- Notarization and Gatekeeper on a clean Mac (phase 12 manual exit).
- Spotify against a live account (phase 10 manual demo; the degraded UI is unit-tested by feeding flags, with no network).
- Battery GPU clocks (phase 7, one Instruments capture, noted in the PR).
- Trademark clearance. That is the owner’s gate, not a test.

## CI shape

Phase 0: GitHub Actions, macOS 26 arm64, `cargo test` for the workspace. `MACOSX_DEPLOYMENT_TARGET` is 26.0. If the `macos-26` label stops scheduling a job, the workflow does not quietly use `macos-15` and it does not lower the deployment target. It fails, and we stop.

Linux `cargo test` may be added when a free runner exists. It does not build GTK until phase 14.

Do not cache a Developer ID certificate in a public repo. Phase 12 signing stays on a private workflow or the owner’s machine until a secrets story exists. The plan does not assume a public CI can notarize.
