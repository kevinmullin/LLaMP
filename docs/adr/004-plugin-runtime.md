# ADR 004 — Plugin runtime: native first-party, wasmtime for users

- Status: accepted
- Date: 2026-09-13
- Amended: 2026-09-13. First-party native hot reload is a dev-build demo. A notarized build does not do it.

## Context

We need plugins for media sources, decode, DSP, output, visualization, lyrics, metadata, and UI panels. Two of those break a naive sandbox: a visualizer wants a GPU surface and a realtime-adjacent feed, and a lyrics provider wants the network.

Users will install plugins we do not sign. The app is notarized with the hardened runtime and without `disable-library-validation`, so an unsigned dylib must not load. See [ADR 011](011-distribution.md).

Winamp’s `in_*.dll` / `dsp_*.dll` / `vis_*.dll` ABI is not a goal. It cannot exist on macOS and would bend the design toward a 1999 Windows calling convention.

## Decision

Two tiers:

1. **Native.** A C ABI for first-party code compiled into the signed app, or an embedded framework signed with the same Team ID. This is the only tier allowed on the audio callback (`DSP`) and the only tier allowed to touch wgpu (`Visualizer` GPU).
2. **Sandboxed.** wasmtime with the component model (WIT) for anything a user installs. No ambient network. No ambient filesystem. Capabilities are an allow-list in the manifest: HTTPS hosts, and filesystem paths the user has granted.

v1 kinds a user may install: `MediaSource`, `LyricsProvider`, `Metadata`, and a CPU `Visualizer` that receives a frame packet and returns RGBA. Not in user WASM in v1: `DSP` on the audio thread, `Output`, and a GPU `Visualizer`. A WASM `Decoder` is allowed later because it runs on the decode thread; 1.0 decoders are first-party.

Capability grants are designed from `LyricsProvider` and `Visualizer` first, not from the easy kinds. A lyrics plugin declares hosts (`lrclib.net` is the default grant if the user opts in). A visualizer gets the frame packet and a deadline. It does not get a `wgpu::Device`.

A plugin that exceeds its frame budget is killed. Audio must already have continued, because the plugin was not on the callback.

Hot reload of a first-party native plugin exists in dev builds. It is not a 1.0 user feature. A signed, notarized build with library validation on, and without `disable-library-validation`, cannot `dlopen` a rebuilt dylib. Do not build a 1.0 feature around that reload. WASM may still be disabled and enabled without an audio drop. That is the release behavior. A `DSP` reload, in a dev build, happens only at a buffer boundary, and only if the new instance constructs without allocating on the callback.

## Consequences

- Third-party DSP and GPU visualizers are not a 1.0 feature. That is intentional.
- Milkdrop compatibility cannot arrive as a user WASM plugin that owns GL. If projectM lands later, it is a first-party or separately signed optional native plugin. See [ADR 010](010-milkdrop.md).
- wasmtime’s WIT networking shape at the pinned version is an investigation in phase 6. The contract (declared hosts, deny otherwise) is not optional. The exact host-function names are pinned in that phase, not invented here.
- We maintain one sandbox runtime, not two.

## Alternatives

- **Extism.** Rejected. It is a simpler plugin SDK and a second runtime. It does not make a GPU handle safe, and it does not make a WASM call realtime-safe on the audio thread.
- **User-installed native dylibs.** Rejected for 1.0. Loading them requires disabling library validation, which weakens the hardened runtime for the whole process.
- **WASM DSP on the audio thread.** Rejected. Fuel, epoch interruption, and instance memory are not a proven no-allocation callback. A buffered handoff adds latency and can underrun. Neither is acceptable as the default DSP path.
- **Legacy Winamp plugin ABI, even as a Windows shim in the core design.** Rejected. Note it as a possible far-future Windows-only experiment. Do not reserve calling conventions for it.
