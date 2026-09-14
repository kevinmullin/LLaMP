# What the MIT Milkdrop example actually links

Date: 2026-09-14. Phase 7 investigation. Tree read: `mbrukman/winamp-macos` at `main` (MIT, Copyright 2024 Matt Greenwood). The upstream `mgreenwood1001/winamp` still 404s.

Bounded to filenames, the package manifest, the Xcode Frameworks phase, and import / Milkdrop-string hits. `pzzzy/macwamp` was not opened. `SawyerChristensen/Prism` was not opened (license still unverified for a tree read). Nothing from the example was copied.

## What it claims

`README.md` lists “Milkdrop (click on the icon in the main app) - supports fullscreen mode” and “Lyrics overlay in Milkdrop.” `fullscreen.png` is a screenshot of that window.

## What it links

Nothing Milkdrop-shaped.

- `Package.swift` has `dependencies: []`. The executable target has no package products.
- `Winamp.xcodeproj/project.pbxproj` `PBXFrameworksBuildPhase` has no file entries. There is no `libprojectM`, ANGLE, OpenGL, Metal, SceneKit, or wgpu link line.
- The tree has no `.milk`, no shader sources, no `projectm` submodule, and no visualizer crate.

Imports on the visualizer path are `SwiftUI`, `AppKit`, `AVFoundation`, `Combine`, and `MediaPlayer`. That is the player, not a Milkdrop engine.

## What the fullscreen window is

`Sources/ContentView.swift` opens `MilkdropVisualizerView` when a toggle is on. Fullscreen is `NSWindow.collectionBehavior` plus a frame change. The canvas is `MilkdropCanvas` in `Sources/MainPlayerView.swift`: SwiftUI `Canvas` / `GraphicsContext` draws, switched on an enum of eleven in-repo names (Spiral Galaxy, Oscillator Grid, Plasma Field, Particle Storm, Frequency Rings, Waveform Tunnel, Kaleidoscope, LFO Morph, Nebula Galaxy, Starfield Flight, Star Wars Crawl). Those are original draw routines. They are not `.milk` presets and they do not parse the Milkdrop expression language.

The 76×16-style pane in `Sources/SpectrumView.swift` is a separate SwiftUI bars / oscilloscope view. `USAGE.md` says that spectrum is simulated; `Sources/AudioPlayer.swift` comments that it fills visualization from simulated data.

## Cost

This is not path (a) and not path (b). It is the alternative [ADR 010](../adr/010-milkdrop.md) already rejected: a Milkdrop label without projectM and without an interpreter.

The “multi-month / second project” cost does not shrink. Path (c) for v1 stands. A later projectM plugin is still a shared LGPL library and its own GL context (or an ANGLE/EGL bridge as a phase of its own). This note does not add `.milk` loading.

ADR 010 is not amended. The reading was not cheaper.
