# Plugin ABI

Two tiers. The split is [ADR 004](../adr/004-plugin-runtime.md). This file is the contract. It does not invent C signatures. Phase 6 pins names when cbindgen runs, and checks the header in. A session that pastes a fictional `llamp_plugin_init` into a crate before that pin is guessing.

## Kinds

| Kind | v1 first-party native | v1 user WASM | Thread |
| --- | --- | --- | --- |
| `MediaSource` | Yes (local files) | Yes | Library |
| `Decoder` | Yes (`symphonia`, AudioToolbox plugin) | Not in 1.0; allowed later on the decode thread | Decode |
| `DSP` | Yes | No | Audio callback |
| `Output` | Yes (`cpal` backend is not a user plugin) | No | Output callback |
| `Visualizer` | Yes, may use wgpu | CPU only: frame packet in, RGBA out | Render, never the audio callback |
| `LyricsProvider` | Yes (tags, sidecar, LRCLIB) | Yes, declared hosts only | Library |
| `Metadata` | Yes | Yes, declared hosts only | Library |
| `UIPanel` | Yes, a panel description the shell draws | Yes, description only, no native view | UI reads the description |

`UIPanel` in 1.0 is a title, a list of rows, and actions the host performs. It is not an embedded web view and not a plugin-drawn `NSView`. If that is too weak for a plugin idea, the idea waits. Do not add a web view to make a plugin interesting.

Legacy Winamp plugin DLLs are not a kind.

## Versioning

Each plugin has a manifest `abi` field: a semver range against the host ABI (`1.0` in 1.0). The host refuses to load a plugin whose range does not include the host version. The refusal is a record the UI can show. It is not a crash and not a silent skip.

Breaking the C ABI or the WIT interface bumps the major version. Adding an optional host function does not, if old plugins still load.

The host ABI and the app version are different numbers. A skin-only release does not bump the plugin ABI.

## Manifest

JSON, file `llamp-plugin.json` next to the plugin, or embedded in the WASM component as the established custom section once phase 6 pins the section name. JSON is the reviewable form either way.

```json
{
  "id": "example.lyrics",
  "name": "Example lyrics",
  "version": "0.1.0",
  "abi": "^1.0.0",
  "kind": "LyricsProvider",
  "capabilities": {
    "network_hosts": ["lrclib.net"],
    "filesystem": []
  }
}
```

Rules:

- `id` is reverse-DNS, stable across versions.
- `version` is the plugin’s semver.
- `kind` is one of the table above.
- `network_hosts` is a list of hostnames, no paths, no wildcards in 1.0. HTTPS only. A request to any other host fails before it is sent.
- `filesystem` is a list of path grants. In 1.0 the only grant a user plugin may receive is a directory the user picked in a panel after install. The manifest may request `"user-selected"`. It may not name `/` or a home directory and have that honored.
- Unknown fields are ignored. A missing `kind` is a refused plugin.

Native first-party plugins have a manifest too, so the host treats them uniformly. Their capabilities may include `audio_callback` and `gpu_surface`. A user manifest that sets those is refused.

## Capabilities, designed from the hard kinds

`LyricsProvider` is the network case. The default remote provider declares `lrclib.net` and nothing else. The host HTTP client is the only socket. The plugin does not open one. User opt-in is still required before the first request; a capability grant is not consent. See [lyrics](lyrics.md).

`Visualizer` is the GPU and realtime case.

- Every visualizer receives a frame packet (see [visualizer](visualizer.md)). The packet is a copy. The plugin may not retain the pointer past the call.
- WASM visualizers return an RGBA buffer the host uploads. They have a deadline: one display frame, measured by the host. Exceeding it kills the instance. The next frame uses the last good image or black, and the UI marks the visualizer stopped.
- Native visualizers may draw into the wgpu surface the shell created. They have the same deadline. They still must not run on the audio callback. The frame packet is how they see audio.
- No visualizer receives a device pointer through the WASM boundary in 1.0.

`DSP` is native-only because the callback cannot wait, allocate, or recover from a trap. A native DSP that allocates on the callback fails the phase 1 allocator hook in CI. That test covers first-party DSP, not only the EQ.

`MediaSource` may use declared hosts (a radio plugin, later a media-server plugin). It may not read the user’s granted folders except through host functions. It returns capability flags for each track it resolves. A source that claims `produces_pcm` and then fails to deliver PCM is unloaded, and the session shows an error. It is not silently treated as Tier B.

## Sandbox

wasmtime, component model. Phase 6 pins the version and writes that pin into this spec, plus the WIT package. Until that pin, this spec is the requirements list:

- No ambient filesystem, no ambient network, no clocks except a host function that returns the playback position, no random except a host function.
- Memory limit and a fuel or epoch budget so a tight loop dies. The numbers are chosen in phase 6 so a normal lyrics fetch succeeds and a deliberate infinite loop dies within one second.
- A plugin trap unloads that plugin. It does not take down the host process. If wasmtime cannot promise that, we do not ship user plugins in 1.0. That is a phase 6 exit, not a hope.

Extism is not a second runtime. See [ADR 004](../adr/004-plugin-runtime.md).

## Hot reload

Dev builds may watch a first-party plugin path and reload between tracks, or immediately if the kind is not `DSP`. A `DSP` reload happens only at a buffer boundary, and only if the new instance constructs without allocating on the callback. A failed reload keeps the old instance. That reload is a dev-build demo. A notarized binary with library validation on does not `dlopen` a rebuilt dylib. Do not treat the demo as a 1.0 feature.

Release builds load plugins at launch and when the user enables one. They do not poll a directory on the audio thread. Enable and disable must not drop the audio callback. That is the 1.0 behavior.

## What a shell does

The shell lists plugins, shows the permission prompt for hosts and folders, and draws `UIPanel` descriptions. It does not `dlopen` a user dylib. The host does not `dlopen` one either, in 1.0.
