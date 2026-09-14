# ADR 010 — Milkdrop is not in v1; projectM is the later path

- Status: accepted
- Date: 2026-09-13
- Amended: 2026-09-13. A MIT macOS player claims Milkdrop. That does not reverse path (c). The cost note is checked before phase 7 treats this ADR as settled.

## Context

Milkdrop presets are the highest-leverage classic visualizer feature and the largest scope risk in the product. Three paths were required:

- (a) Integrate projectM.
- (b) Implement a `.milk` interpreter, including the per-frame and per-vertex expression language.
- (c) Ship an original shader preset format and no Milkdrop compatibility.

projectM 4.1 is LGPL-2.1, speaks a C API, and renders with OpenGL. It does not ship presets. Preset packs have their own licenses. The original Milkdrop packs are not ours to redistribute. AVS is out of scope regardless of path.

Path (b) means an expression language, megabuf, loops, and HLSL-to-Metal translation. projectM spent a release cycle on that and still has presets that render black. Doing it ourselves in v1 is a multi-month project beside the player, not a feature of the visualizer window.

## Decision

v1 is path (c). Built-in visualizers use an original preset format: WGSL, plus a small JSON header (name, author, pixel-vs-vertex). We ship presets we wrote. We do not ship `.milk` files.

Path (a) is the post-1.0 compatibility route, and only as an optional native plugin with its own OpenGL context, not as a wgpu backend. Link it as a shared library so LGPL obligations stay on that library. Do not statically absorb it into the MIT/Apache binary. Do not bundle a preset pack we do not have rights to. A user may point the plugin at a preset directory they own.

Path (b) is rejected for v1 and is not the preferred post-1.0 path. If projectM’s license or OpenGL embedding is unacceptable when that phase starts, we stay on path (c). We do not then start a `.milk` interpreter to avoid the license. The cost of (b) does not shrink because (a) was inconvenient.

A MIT macOS player (`mbrukman/winamp-macos`, a fork of `mgreenwood1001/winamp`; the original repo 404s as of 2026-09-13) claims fullscreen Milkdrop. That does not mean path (c) is wrong. It means the cost estimate under this ADR is checked against that example before phase 7 treats it as settled. The reading is bounded to how it embeds Milkdrop. It is not a license to copy the player. `pzzzy/macwamp` is GPL-3.0. Do not open it. A separate project, `SawyerChristensen/Prism`, embeds projectM through an ANGLE/EGL bridge, which is the “second project” this ADR already names. Verify that repo’s license before reading it. None of this pulls `.milk` into v1.

AVS compatibility is out of scope. It does not return as a stretch goal without a new ADR.

## Consequences

- 1.0 visualizers will not load a user’s Milkdrop folder. The UI must not imply that they will.
- The original preset format can be small. Two built-in plugins in phase 7 are enough to prove the frame packet and the kill switch.
- Before phase 7 treats the “multi-month / second project” cost as settled, the investigation in [PLAN.md](../PLAN.md) records what the MIT example actually links. A cheaper reading amends this ADR’s cost. It does not change the v1 decision without a new owner call.
- A later projectM plugin is a separate process or a carefully isolated GL context. Falling back to “render projectM into a wgpu texture via IOSurface” is a phase of its own, not a weekend, and is not assumed. An ANGLE/EGL bridge, if one exists in the wild, is that phase, not a counterexample that makes it small.
- LGPL and preset licensing are in [LEGAL.md](../LEGAL.md) before any link line is added.

## Alternatives

- **Path (b) in v1.** Rejected. Expression language, megabuf, shader translation, and a preset corpus we cannot ship. That is the schedule risk this ADR exists to refuse.
- **Path (a) in v1.** Rejected. OpenGL beside wgpu on macOS, an LGPL shared library in the first notarized build, and a preset story that is mostly “bring your own.” It also does nothing for the 76×16 pane, which must stay a CPU blit.
- **Claim Milkdrop compatibility without projectM or an interpreter.** Rejected. That would be a false label.
