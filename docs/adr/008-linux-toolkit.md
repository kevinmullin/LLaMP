# ADR 008 — Linux toolkit is GTK4, and it waits

- Status: accepted
- Date: 2026-09-13

## Context

Linux is last. The shell must be native windowing and input, with our own blit of the skin atlas. A cross-platform UI toolkit must not draw the chrome on any platform. Picking the Linux toolkit now, in detail, would freeze an API the Windows shell has not yet pressured.

We still need a named choice and a trigger, so the decision is not re-litigated in the phase that should be building windows.

## Decision

The Linux shell uses GTK4 for windows, input, and accessibility integration. It draws the atlas itself, the same contract as AppKit. It does not use GTK widgets for transport buttons, sliders, or the playlist text.

Work starts only after phase 13 (Windows) has shipped a demo **and** the shell-facing C ABI has gone through that phase without a breaking change. If Windows forces an ABI break, Linux waits until the next Windows demo that does not break it.

PipeWire is the output backend, ALSA the fallback, both behind `Output`. See [ADR 006](006-audio-output.md).

## Consequences

- Linux is not sketched in parallel with the macOS shell. That is the point.
- GTK4 is LGPL. We link the system library dynamically. We do not vendor GTK. [LEGAL.md](../LEGAL.md) states that.
- A headless core test suite is the Linux story until phase 14. CI for Linux can run `cargo test` on a Linux runner from phase 0 if one is free. It does not have to build GTK.

## Alternatives

- **Qt.** Rejected. LGPL dynamic linking is legally possible and still the heavier binding, with a dual-license story we do not need for a permissive app.
- **iced or egui.** Rejected. They are not a native shell, and they would draw chrome we have reserved for the atlas blit.
- **A pure wgpu window.** Rejected. It puts skin chrome on the GPU path [ADR 009](009-visualizer-gpu.md) quarantines to the visualizer.
- **Decide the toolkit when Linux starts, with no default.** Rejected. That is how the phase burns its first weeks on a toolkit argument. The trigger is about timing. The toolkit is decided.
