# ADR 001 — Core language is Rust

- Status: accepted
- Date: 2026-09-13
- Amended: 2026-09-13. `rubato` 5 (5.0.0) and `lofty` 0.25.2. wasmtime and wgpu stay unpinned until their phases.

## Context

LLaMP needs one portable engine for decode, DSP, skin parsing, library, playlists, and a plugin host. The audio thread must not allocate, lock, or do I/O. Three native shells (AppKit, later Win32, later GTK) must call the engine through a C ABI. The implementers have no language preference and will learn the one this ADR picks.

The realtime constraint rules out a garbage-collected core on the audio path. A Swift-only core cannot be the Windows or Linux engine. A C core can be realtime-safe and is a painful way to own a database, a ZIP parser, and a plugin host for a team of one or two.

## Decision

The engine is Rust. Shells are not. `llamp-ffi` is the only crate that may be linked by a shell, and it exports C.

Pinned intent, not a lock to a yanked minor: `symphonia` 0.6 for decode, `rtrb` for the rings, `rubato` 5 for resampling, `rusqlite` with bundled SQLite for the library, `lofty` 0.25 for tags, `wasmtime` for third-party plugins (version pinned in phase 6 and written into the plugin spec), `wgpu` for the visualizer surface only (version pinned in phase 7 and written into the visualizer spec). Crates still pre-1.0 (`rtrb`, `rusqlite`, and `lofty`) sit behind traits so they can be replaced without a rewrite.

## Consequences

- The team learns Rust. That is an accepted cost.
- The audio callback can be written without a garbage collector and tested with an allocator hook.
- `symphonia` is MPL-2.0. Our code stays MIT OR Apache-2.0. We do not strip MPL notices. We do not fork `symphonia` unless a fixture requires a change we can upstream.
- Crate versions in this ADR are the planning baseline. Phase 1 pins them in `Cargo.toml`. A bump that changes realtime behavior is a new ADR, not a quiet edit.

## Alternatives

- **C++ core.** Realtime-safe and familiar. Rejected: more boilerplate for ZIP, SQLite, and a safe plugin boundary, and a worse FFI story into Swift than a dedicated C ABI crate.
- **Swift core.** Rejected: it does not ship as the Windows or Linux engine, and ARC on an audio callback is the wrong default even when a callback can be written carefully.
- **C core.** Rejected: the library, skin parser, and plugin host would be a long unsafe project for a team of one or two, with no gain on the shells, which already speak C.
