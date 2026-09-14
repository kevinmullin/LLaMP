# LLaMP plan

Working name: **LLaMP** (“Large Llama Music Player”). The public name is gated; see [ADR 012](adr/012-working-name.md). The pixel mark at `assets/brand/llamp-mark.png` stays even if the letters change. See [brand spec](spec/brand.md).

This file is the phased roadmap. Architecture is in [ARCHITECTURE.md](ARCHITECTURE.md). Do not start a phase whose exit criteria are not yet written down as tests.

Ship order: macOS first, then Windows, then Linux. The macOS build is a complete product before Windows work begins. macOS 1.0 is the end of phase 12.

## Product

A small, fast, pixel-art desktop audio player that loads classic Winamp 2 `.wsz` skins. Six windows share snapping, docking, shade, double-size, and always-on-top. Shared Rust core, thin native shells. No Electron. No cross-platform toolkit for the skinned chrome.

Tier A sources give us PCM. Tier B sources (Spotify Connect, Apple Music) do not. The UI must say so. See [providers](spec/providers.md).

## What we will not relitigate

- YouTube, SoundCloud, or anything that needs DRM circumvention or a ToS breach.
- Binary compatibility with Winamp `in_*.dll`, `dsp_*.dll`, or `vis_*.dll`. Impossible on macOS. Not a design input. A Windows-only shim is post-1.0 and unplanned.
- The 2024 Winamp source drop. See [LEGAL.md](LEGAL.md).
- AVS (`.avs`) visualizer presets.
- Accounts, cloud library sync, or a schema bent toward a future sync service.
- Mac App Store, or App Sandbox, for 1.0. Direct notarized distribution. See [ADR 011](adr/011-distribution.md).
- Subsonic, Jellyfin, and Plex in 1.0. The `MediaSource` trait must be able to host them. Implementation is the first post-1.0 provider pack.
- System-audio process taps as a feature. A later spike may return go or no-go. Default recommendation is no.
- A `.milk` interpreter in v1. See [ADR 010](adr/010-milkdrop.md).
- User-installable plugins on the audio callback or holding a GPU device. See [plugin ABI](spec/plugin-abi.md).
- Painting the llama mark into a skin. See [brand spec](spec/brand.md).
- Copying the user’s music library into an app-managed folder. 1.0 is reference-in-place. See [ADR 007](adr/007-library-database.md).
- Lowering the macOS 26 Apple Silicon deployment target to widen the audience or to green CI. See [ADR 002](adr/002-macos-ui.md).

## Cut order

If a gate fails, cut in this order. Do not cut the local player to save a remote feature.

1. Apple Music (phase 10 spike may no-go).
2. Public Spotify login (already not a ship blocker; bring-your-own Client ID remains).
3. Podcasts.
4. Internet radio directory. URL-add-a-station can remain if a directory’s terms fail.

Do not cut: local files, EQ, playlist, library, in-window visualizer, lyrics, skin import, the three default skins, VoiceOver on the six windows, notarized DMG.

## Prior art

Note these. Do not copy them. License rules are in [LEGAL.md](LEGAL.md).

May read, do not copy:

- [Llammp](https://github.com/yoanbernabeu/llammp) (MIT). An Apple Music client, not a decoder. Confirmed 2026-09-13. See [ADR 012](adr/012-working-name.md).
- [mbrukman/winamp-macos](https://github.com/mbrukman/winamp-macos) (MIT), a fork of `mgreenwood1001/winamp`. The original repo 404s as of 2026-09-13. The fork’s README still claims fullscreen Milkdrop. A bounded reading of that visualizer path is a phase 7 investigation. It does not pull `.milk` into v1.
- [wishval/wamp](https://github.com/wishval/wamp) (MIT). AppKit, macOS 26.3+, no Milkdrop claimed.

Do not open the source, same rule as the Winamp drop:

- [pzzzy/macwamp](https://github.com/pzzzy/macwamp) (GPL-3.0). It exists. Screenshots only.

[SawyerChristensen/Prism](https://github.com/SawyerChristensen/Prism) embeds projectM through an ANGLE/EGL bridge. That is the “second project” [ADR 010](adr/010-milkdrop.md) already names. Verify its license before anyone reads it. Path (c) for v1 stands.

## Budgets

The thesis is a small, fast player. Missing a number is a failed exit. Do not raise a number in the same change that misses it. The anti-pattern is an Electron player whose own notes put the installer near 98 MB and the unpacked app near 368 MB.

- Cold start to main window painted: ≤ 400 ms on developer hardware (phase 3). That number is not a CI fail. CI prints `launch_ms` and `painted`. It does not fail on them. Cold start to first audible sample of an already-granted local file: ≤ 800 ms (phase 12).
- Idle CPU, main window visible, one track playing, 76×16 vis on, other windows closed: ≤ 5% of one P-core, 30 s average after warmup. Phase 3 and phase 12.
- Idle RSS, same scene, at least 1,000 referenced tracks: ≤ 80 MiB at phase 3, ≤ 150 MiB at phase 12. A closed visualizer keeping wgpu resident is an [ADR 009](adr/009-visualizer-gpu.md) failure, not a budget raise.
- Unzipped `.app`, no user skins: ≤ 30 MiB at phase 3, ≤ 80 MiB at phase 12.

Bundle size is a hard CI gate. Launch-to-window 400 ms is a hard gate on developer hardware, same script. CI records `launch_ms` and `painted`. It does not fail on them: the same `--paint-and-exit` path on `macos-26` printed 444.2 / 764.5 / 1567.2 ms on 2026-09-14, and the last two shared `bundle_bytes 6152192`. The runner is not the 400 ms clock. A hung process still fails (`timeout=10`, and stdout must contain `painted`). Idle CPU and RSS are a phase-exit script on the `macos-26` runner. Do not raise the 400 ms number.

## Investigation tasks

These are under 80% confidence. Do not invent an API to close them. Each has an owner phase.

| Task | Phase | Done when |
| --- | --- | --- |
| Our `macos-26` workflow actually gets a runner | 0 | The label has been generally available since 2026-02-26. Phase 0 confirms a run is scheduled. If the label stops scheduling, stop and report. Do not lower the deployment target. |
| `cpal` 0.18 default-device hot-swap and underrun recovery | 1 | A scripted unplug/replug either recovers or the ADR is amended to `coreaudio-rs`. |
| Classic EQ AUTO button meaning | 4 | Written from published behavior notes, not Winamp source. Until then the button is present and documented as “behavior pending”, not a guessed function. |
| `pledit.bmp` vertical step is 29 px, and the full sprite atlas coordinates | 2 | A fixture `.wsz` (not checked in) renders to the golden PNG. Wrong coordinates fail that test. |
| 76×16 bar geometry | 2–3 | Golden image locks bar width. Do not hard-code “19 bars” before that image exists. |
| wasmtime WIT host allow-list for declared HTTPS hosts | 6 | Pinned wasmtime version written into [plugin ABI](spec/plugin-abi.md), and a test that an undeclared host is denied. |
| Sparkle 2 entitlements on macOS 26 hardened runtime | 12 | A notarized build updates itself. Entitlement keys are copied from Sparkle’s then-current docs, not guessed here. |
| MusicKit catalog from a notarized non-App-Store binary | 10 | Spike writes go or no-go. Local Apple Music library control is the fallback. No scraping. |
| Radio Browser terms | 9 | Terms pasted into [LEGAL.md](LEGAL.md). If they fail, ship manual station URLs only. |
| `lofty` 0.25 ReplayGain 2 field names | 1 | A fixture file’s gain tags round-trip. If they do not, the tag trait stays and the reader is swapped. |
| MP3 gapless via LAME/Xing encoder delay and padding | 1 | A fixture joins within one sample, or the note says `symphonia` does not surface delay and MP3 drops off the gapless list. FLAC, Vorbis, ALAC, and Opus stay required. |
| wgpu version for the visualizer surface | 7 | Pinned version written into [visualizer](spec/visualizer.md). Embedding calls are written against that pin, not guessed earlier. |
| wgpu surface from a shell-owned CAMetalLayer — lifetime across resize and layer replacement | 7 | A surface survives both in a running window. |
| What the MIT Milkdrop example actually links | 7 | A bounded reading of `mbrukman/winamp-macos` (MIT) writes what it links for fullscreen Milkdrop. Do not open `pzzzy/macwamp` (GPL-3.0). The note may amend [ADR 010](adr/010-milkdrop.md)’s cost. It does not pull `.milk` into v1. |

## Phases

Each phase is independently demoable and independently revertible. Exit criteria are tests or a checkable artifact, not a vibe.

### 0 — Foundations

Scope: Cargo workspace as in [REPO_LAYOUT.md](../REPO_LAYOUT.md). `llamp-ffi` exports a version string through the C ABI, plus two probes from [ADR 003](adr/003-ffi.md): a 64 KB buffer with its free function, and a polled counter. A Swift package calls them. GitHub Actions on macOS 26 arm64. `MACOSX_DEPLOYMENT_TARGET` is 26.0. License files (MIT OR Apache-2.0). Naming gate recorded (ADR 012). Canonical mark already at `assets/brand/llamp-mark.png`. MSRV is at least 1.87, because `rubato` 5 requires it.

Exit:

- CI is green on `macos-26`. If that label stops scheduling a job, the phase fails open and we report it. We do not retarget.
- A unit test asserts the version string matches `CARGO_PKG_VERSION`.
- A 64 KB dummy buffer is returned and freed with the documented free function. A test fails on leak or double-free.
- Swift polls a counter at 60 Hz for one second and sees a monotonic value. The publisher takes no lock.
- No BMP, audio decode, or UI chrome yet, except the smoke window.

Demo: a tiny AppKit window whose title came from Rust and whose icon is the mark at 1× nearest-neighbor.

### 1 — Headless audio

Scope: decode, seek, gapless, EQ, FFT tap, CoreAudio output, CLI. Spec: [audio pipeline](spec/audio-pipeline.md).

Exit:

- `llamp play` decodes MP3, AAC-LC, FLAC, ALAC, WAV, AIFF, Vorbis, and Opus (Opus via `libopus`).
- Gapless transition on FLAC, Vorbis, ALAC, and Opus fixtures. MP3 is in that list only if the encoder-delay investigation passes. AAC-LC is documented as not gapless.
- Seek lands within one device period of the requested frame.
- EQ band change does not click; a unit test shows coefficient crossfade, not a step.
- Allocator hook fails the test if the audio callback allocates, locks, or does I/O.
- Unplug of the default device either recovers or triggers the `cpal` investigation.

Demo: `llamp play fixture.flac` in Terminal, with sound.

### 2 — Headless skin engine

Scope: `.wsz` parse, atlas, bitmap font, region masks, CPU render of the main window to PNG. Spec: [skin format](spec/skin-format.md).

Exit:

- Golden PNG of the main window against a fixture skin the developer supplies locally (the `.wsz` is not committed).
- A skin missing an optional sheet still loads, with a defect record.
- An 8-bit `BI_RLE8` BMP does not panic. A decompression bomb of that encoding does not panic and does not allocate past the cap.
- A zip-slip path, a compression bomb, and declared BMP dimensions that overflow `width * height * 4` are rejected. None panic. No path from the archive touches the filesystem.
- `cargo-fuzz` targets exist for the ZIP reader and the BMP decoder.
- A `main.bmp` that is not 275×116 is rejected; the previous skin stays loaded.
- Region mask matches `region.txt` for the fixture.

Demo: a 275×116 PNG written by the CLI, no window.

### 3 — macOS main window

Scope: borderless chrome, drag-anywhere, hit regions, transport, seek, volume, balance, clutterbar, shuffle, repeat, time digits with elapsed/remaining, marquee, kbps/kHz, 76×16 visualizer (oscilloscope and spectrum, click to cycle), shade (14 px), double-size, always-on-top, snap against other LLaMP windows (even if the others are stubs), keyboard transport, VoiceOver labels on those hit regions.

Exit:

- An `NSBitmapImageRep` of the content view, in a pinned sRGB color space with no display profile, matches the phase 2 PNG at 1×. A window-server screenshot is a manual check only. It is not the exit test.
- The same capture at 2× backing scale is that PNG nearest-neighbor scaled. A test samples a scale-2 edge in the backing store and fails if it is bilinear. Softness from a scaled display mode is the window server resampling a 2× store. It is not an app bug and it is not a failed exit.
- Keyboard: space toggles play, arrows seek, and the actions are AX-exposed.
- Shade height is 14 skin pixels.
- Phase 3 budgets in the Budgets section pass: launch-to-window 400 ms on developer hardware (CI records `launch_ms` and `painted` and does not fail on them), idle CPU, idle RSS, bundle size.

Demo: the main window, movable, playing a file.

### 4 — EQ window

Scope: 275×116 equalizer. Preamp plus bands at 60, 170, 310, 600, 1k, 3k, 6k, 12k, 14k, 16k Hz, ±12 dB. On and auto toggles. Preset menu. Curve graph. Snap and dock to the main window. Shade. Spec notes on AUTO: [audio pipeline](spec/audio-pipeline.md).

Exit:

- A band sweep is audible in the CLI playback test and in the window.
- Coefficient crossfade test from phase 1 still passes when the slider is dragged.
- AUTO does not ship a guessed behavior. The button exists; its action is the investigation result or a documented no-op with a tooltip until that result exists.
- Docked drag moves main and EQ together.

Demo: EQ snapped to the main window, a band boosted, no zipper click.

### 5a — Playlist window

Scope: playlist window. Resize in 25 px horizontal and 29 px vertical steps from 275×116, with Add / Rem / Sel / Misc / List. Shade and dock. Playlist uses `text.bmp` when every glyph in the row exists; otherwise that row uses CoreText with `pledit.txt` colors. The mixed-typography rule lives here, not in 5c.

Exit:

- Resize snaps to the 25/29 increments; a free drag that lands mid-step is rejected.
- A fixture row that mixes a glyph present in `text.bmp` and a character absent from it renders the missing character with CoreText, not a blank box, so the seam is visible before the library exists.
- Rem removes rows from the playlist or the library. It does not delete the user’s file.

Demo: the playlist window snapped to the main window, with one mixed-font row.

### 5b — Library and import

Scope: SQLite library, FTS5, `user_version` migrations, reference-in-place grant and watch. No copy. No free-space check. Spec: [ADR 007](adr/007-library-database.md).

Exit:

- Grant a folder, search with FTS5, enqueue, write a tag, relaunch, read the tag back. The file on disk is the file the user granted, not a copy.
- A v1 fixture database upgrades to v2 without losing rows.
- A referenced file deleted in Finder stays in the library as missing. The row is not dropped.
- M3U, M3U8, PLS, and XSPF export then import round-trip paths as written, and order.

Demo: search the granted folder and enqueue a track without duplicating it on disk.

### 5c — Browser window

Scope: browser window, the fourth window, on `gen`/`genex` chrome. Always CoreText.

Exit:

- The browser lists the granted library and uses CoreText, not `text.bmp`.
- Main, EQ, playlist, and browser snap as one group.

Demo: the four windows snapped as one group.

### 6 — Plugin host

Scope: manifest, semver check, native first-party local `MediaSource`, wasmtime sandbox, hot reload in dev builds only. Spec: [plugin ABI](spec/plugin-abi.md).

Exit:

- A fixture WASM `LyricsProvider` that requests an undeclared host is denied, and one that reads an undeclared path is denied.
- A semver-incompatible plugin is refused with a recorded reason, not a crash.
- Reloading a first-party plugin in a dev build does not drop the audio callback (the allocation hook from phase 1 still passes).
- The pin written into the spec is the wasmtime version the test ran. A notarized build does not hot-reload a native dylib.

Demo: unload and reload a first-party plugin in a dev build while audio continues. That demo is not a 1.0 user feature.

### 7 — Visualizer window

Scope: fifth window, `gen` chrome, freely resizable, borderless fullscreen, wgpu surface, frame-packet contract, two built-in original visualizers. Spec: [visualizer](spec/visualizer.md).

Exit:

- A test double that sleeps past the frame budget is killed, and the audio callback does not miss a period during that kill.
- Hidden or occluded visualizer submits no GPU work (test double counters; Instruments is the manual check).
- The 76×16 pane is unchanged and still uses `viscolor.txt`.
- The wgpu version the window used is written into [visualizer](spec/visualizer.md).
- The Milkdrop cost note from the investigation table is written before this phase treats [ADR 010](adr/010-milkdrop.md) as settled. It does not add `.milk` loading.

Demo: fullscreen visualizer, then hide the window and watch CPU fall.

### 8 — Lyrics

Scope: tags, sidecar LRC, sync, lyrics window, opt-in LRCLIB, cache. Spec: [lyrics](spec/lyrics.md).

Exit:

- Seek, pause, and a sample-rate change do not leave the active line more than one display frame off after the next clock sample.
- Enhanced LRC highlights the current word.
- Unsynced lyrics show plain text. Missing lyrics show the empty state, including the mark at integer scale.
- Remote lookup does not run until the user opts in. The privacy note lists artist, title, album, and duration as the only fields sent.
- A user offset saved from the window is present in the sidecar when the audio file’s directory is writable. If it is not, the offset is in SQLite and the UI says the sidecar was not written. The audio file is not copied to make the directory writable.

Demo: karaoke-style highlight on an enhanced LRC.

### 9 — Radio and podcasts

Scope: Icecast/SHOUTcast with reconnect and ICY metadata. Station directory only if Radio Browser terms pass. Podcast RSS downloads into Application Support as `managed` tracks, with retention (keep the latest incomplete episode plus the last five completed, unless the user pins) and a free-space check before any write. HE-AAC via the AudioToolbox decoder plugin when symphonia cannot.

Exit:

- A dropped socket to a fixture server reconnects and playback resumes without a UI restart.
- A downloaded podcast episode is a normal library track: EQ applies, seek works, tags are readable.
- If Radio Browser terms fail, the directory UI is absent and “add station URL” still works.

Demo: play a station, then a downloaded episode with EQ on.

### 10 — Tier B

Scope: Spotify Connect controller (PKCE, bring-your-own Client ID) and the degraded capability UX. Apple Music spike, then `ApplicationMusicPlayer` in the Swift shell only if the spike passes. Spec: [providers](spec/providers.md).

Exit:

- With a development-mode Client ID, play, pause, seek, and next work on a Connect device.
- The EQ window’s caption names the device and says the equalizer is not applying. Sliders are disabled. The curve is a flat disabled line.
- The 76×16 pane is the non-reactive remote state, not a fake spectrum.
- Extended-quota public distribution is a written go/no-go in `docs/investigations/spotify-quota.md`. A no-go does not fail the build.
- MusicKit spike is a written go/no-go. A no-go leaves the provider out. No catalog scraping.

Demo: a Spotify Connect device playing while the in-window visualizer is in the honest remote state.

### 11 — Skins

Scope: three original skins (near-classic grey, dark, high-contrast), switcher, `.wsz` import. Layered PNG sources and an `xtask` that emits `.wsz`. No third-party skin in the repo or the bundle. The mark is not in the sprite sheets.

Exit:

- Each default skin loads, shades, and double-sizes.
- High-contrast CoreText surfaces meet the contrast ratio in [TESTING.md](TESTING.md).
- Switching skins frees the previous atlas (a test asserts the old skin id is gone).
- Import of a user-owned `.wsz` from outside the bundle works. The bundle contains only the three originals.

Demo: switch skins while a track plays.

### 12 — macOS 1.0

Scope: media keys, Now Playing / remote command center, hardened runtime, notarization, Sparkle, DMG, VoiceOver on all six windows, always-on-top, snap/dock/shade/double-size on all six. Bundle icon and DMG use the mark at integer sizes 16, 32, 128, 256, 512, 1024 on a flat black field.

Exit:

- A notarized DMG on a clean macOS 26 Apple Silicon account installs, plays a local file, and updates via Sparkle.
- Phase 12 budgets in the Budgets section pass, including launch-to-audible.
- A crash writes a local report. Nothing is sent unless the user opts in. There is no telemetry.
- `com.apple.security.cs.disable-library-validation` is absent.
- Finder and About show the llama. A loaded third-party skin does not contain it.
- VoiceOver rotor lists transport, seek, volume, and each window’s primary list.

Demo: the ship candidate. Windows work does not start before this exit is met.

### 13 — Windows shell

Scope: Win32 custom chrome over the same C ABI. WASAPI shared and exclusive behind `Output`. Integer-scale policy for fractional DPI (letterbox; never bilinear skins). The shell blits the core composed surface at integer scale. It does not assemble sprites from an atlas.

Exit:

- A 1× screenshot of the main window matches the phase 2 PNG.
- Unplugging the default device recovers without a restart.
- Exclusive mode is opt-in and releases the device on pause longer than five seconds.

Demo: main, EQ, and playlist on Windows.

### 14 — Linux shell

Scope: GTK4 for windowing and input only. Integer-scale blit of the core composed surface on the same C ABI. PipeWire, ALSA fallback. Does not start until the Windows shell trait has survived phase 13 without a breaking change. See [ADR 008](adr/008-linux-toolkit.md).

Exit:

- A 1× screenshot of the main window matches the phase 2 PNG.
- A file plays through PipeWire. If PipeWire is absent, ALSA fallback plays the same file.

Demo: main window playing a file on Linux.

## After 1.0

Not scheduled as ship work:

- Subsonic, Jellyfin, Plex provider pack.
- projectM as an optional OpenGL visualizer plugin, with only presets we have rights to ship. See [ADR 010](adr/010-milkdrop.md).
- Tap-to-sync LRC editor.
- Core Audio process-tap spike. Permissioned, persistent indicator, default off. Likely no.
- `libopenmpt` tracker plugin.
- Localization of non-skin strings. Skin chrome stays pixel art and is not translated by string tables.
- A Winamp plugin shim. It will not happen on macOS. Do not design the ABI to accommodate it.

## Process-tap note

macOS 14.4 and later can capture another process’s output with `CATapDescription`, after the user grants `NSAudioCaptureUsageDescription`. The prompt fires at `AudioDeviceStart`, not at tap creation. A missing usage string or a binary launched outside LaunchServices fails silently. That could feed reactive visuals for Tier B. It is also a request to listen to other applications. It is not a planned feature. A later spike writes go or no-go. The default recommendation is no, unless a user explicitly enables “visualizer listens to system audio” and a visible indicator stays on while the tap is running.
