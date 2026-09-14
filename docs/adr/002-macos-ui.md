# ADR 002 — macOS UI is AppKit

- Status: accepted
- Date: 2026-09-13
- Amended: 2026-09-13. Deployment target and CI image are named as separate knobs. The owner kept macOS 26, Apple Silicon only.

## Context

The main window is 275×116 skin pixels, borderless, with a `region.txt` mask, custom hit regions, and bitmap sprites. The same hit regions must be accessibility elements. Five other windows share snap, dock, shade, and double-size. Skins are 1× pixel art. A blurry scale is a bug.

The shell must be genuinely native. A cross-platform widget toolkit drawing the chrome would violate that, even if the toolkit can open a Metal view.

Deployment target is macOS 26, Apple Silicon only. The owner chose that. The feature set does not require it. The audience cut is accepted on purpose.

## Decision

The macOS shell is Swift and AppKit. Each player window is a borderless `NSWindow` with a custom `contentView`. Hit regions come from the core’s control map and are mirrored as `NSAccessibility` elements. Scaling is integer nearest-neighbor on a layer whose magnification filter is nearest, never linear.

Deployment target is macOS 26, Apple Silicon only. The owner chose that on 2026-09-13 and confirmed it when an external review asked to drop to macOS 14 and a universal binary. The reason is one architecture, no universal-plugin matrix, and current AppKit. It is not a missing API. The cost is accepted: machines on older macOS, and Intel Macs, cannot install 1.0. macOS 26 being the last release that still supports Intel does not change the choice.

`MACOSX_DEPLOYMENT_TARGET` and the CI image are different knobs. They both happen to be 26. The deployment target is 26.0 because the owner chose it. CI uses the `macos-26` label because that image exists (generally available since 2026-02-26) and is the host we ship from. A newer runner image must not silently raise the deployment target. If the label stops scheduling a job, phase 0 stops and reports. It does not retarget, and it does not point the workflow at an older label to go green.

SwiftUI is not used for skinned chrome. It may appear only in a system sheet we do not skin (a file importer, a permission explanation) if AppKit would be longer and the sheet is not pixel-art. Phase 3 does not need that exception. Do not introduce it casually.

## Consequences

- Window geometry, drag, and the non-rectangular mask are under our control.
- VoiceOver work is part of building each window, not a later pass. Phase 3 exits with AX labels on the main window. Phase 12 exits with all six.
- We do not get SwiftUI’s layout for free. We do not want it for this chrome.
- Catalyst and iOS are out. This ADR is macOS 26 AppKit.

## Alternatives

- **SwiftUI as the chrome.** Rejected: it will not hit-test a `region.txt` polygon or keep a 275×116 sprite layout pixel-stable across OS updates.
- **Catalyst.** Rejected: it is not a desktop player window model, and it does not help Windows or Linux.
- **A cross-platform toolkit (GTK, Qt, iced, egui) on macOS.** Rejected: the brief forbids a shared toolkit for the skinned chrome, and those toolkits do not speak `NSAccessibility` the way a native `NSView` does.
