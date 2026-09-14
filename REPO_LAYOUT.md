# Repository layout

Names are the contract. Do not rename a crate because a session preferred a shorter word. The display name of the app is a string in the shell and may change at the [naming gate](docs/adr/012-working-name.md). Crate prefixes stay `llamp-` through 1.0 even if the public name changes.

Working name in paths and folders: `LLaMP`. Database, artwork cache, and files the app creates: Application Support. Music stays at the path the user granted. Bundle id: unset until the naming gate. Do not invent one in a plist that gets notarized.

## Tree

```
Cargo.toml                 workspace
Cargo.lock
LICENSE-MIT
LICENSE-APACHE
docs/
  PLAN.md
  ARCHITECTURE.md
  LEGAL.md
  RISKS.md
  TESTING.md
  adr/                     001–012
  spec/                    skin, audio, plugins, providers, visualizer, lyrics, brand
  investigations/          written when a phase opens a gate, not before
crates/
  llamp-core/              session, transport policy, capability flags
  llamp-audio/             decode, resample, DSP, Output, rings
  llamp-skin/              wsz, atlas, regions, bitmap font, CPU blit
  llamp-library/           SQLite, import, tags, artwork, playlist files
  llamp-plugin-api/        kinds, manifest types, capability bits; no loader
  llamp-plugin-host/       native host and wasmtime
  llamp-ffi/               C ABI, cbindgen, checked-in header
  llamp-cli/               llamp binary: play, decode, render-skin
plugins/
  llamp-source-local/      first-party MediaSource for granted local files
  llamp-decoder-audiotoolbox/   macOS only, HE-AAC, not linked into the portable core
shells/
  macos/                   Swift package, AppKit app, XCFramework consumer
assets/
  brand/llamp-mark.png     canonical mark, do not redraw
  skins/src/               layered PNG and text sources for grey, dark, contrast
  skins/dist/              generated .wsz, bundled; do not hand-edit
xtask/                     skin build, later icon sizes
.cursor/rules/
```

Phase 0 creates the workspace and empty crates with a version function. It does not fill every directory with stubs that imply the feature exists. `plugins/llamp-decoder-audiotoolbox` appears in phase 9. `assets/skins` sources appear in phase 11. `docs/investigations/` appears when the first investigation is written.

## Targets

| Target | Package | Kind |
| --- | --- | --- |
| `llamp` | `llamp-cli` | bin |
| `llamp-core` | `llamp-core` | lib |
| `llamp-audio` | `llamp-audio` | lib |
| `llamp-skin` | `llamp-skin` | lib |
| `llamp-library` | `llamp-library` | lib |
| `llamp-plugin-api` | `llamp-plugin-api` | lib |
| `llamp-plugin-host` | `llamp-plugin-host` | lib |
| `llamp-ffi` | `llamp-ffi` | staticlib and cdylib, C ABI only |
| `LLaMP` | `shells/macos` | macOS app, phase 0 smoke app onward |

The app target name may change when the naming gate passes. The crate names do not, in 1.0.

`llamp-ffi` is the only Rust artifact the shell links. The shell does not depend on `llamp-core` as a Rust crate.

## Workspace rules

- One workspace `Cargo.toml`. Members are `crates/*`, `plugins/*`, `xtask`.
- Shells are not Cargo members. Swift does not go through `cargo`.
- MSRV is whatever phase 0’s toolchain is, recorded in the workspace `rust-version` field. Do not float to nightly.
- Apple Silicon, macOS 26, for the shell and for CI. Deployment target is `MACOSX_DEPLOYMENT_TARGET=26.0`. A newer CI image must not raise that. The Rust core may compile on the CI host that exists; it does not grow `cfg(target_arch = "x86_64")` paths for macOS. MSRV is at least 1.87 (`rubato` 5).

## What does not go in the tree

- Third-party `.wsz` files.
- Winamp source, Nullsoft skins, Milkdrop preset packs we do not own.
- Spotify Client IDs, Sparkle private keys, Apple certificates.
- A `node_modules` UI, Electron, or a second skin parser in Swift.
