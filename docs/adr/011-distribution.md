# ADR 011 — Direct notarized distribution, no App Sandbox

- Status: accepted
- Date: 2026-09-13
- Amended: 2026-09-13. Diagnostics posture: local log, opt-in crash report, no telemetry.

## Context

Where the macOS app is sold, or not sold, and whether it is sandboxed, decides the plugin system and file access. The Mac App Store requires the App Sandbox. The sandbox makes user-installed native plugins impractical and turns every music folder into a security-scoped bookmark. We also want a plugin story that is not “WASM only because the store said so,” but we do not want to disable library validation to get that story.

The product is permissive open source, distributed to other people, not sold as a proprietary binary. It is still notarized.

## Decision

1.0 ships as a direct download: hardened runtime, notarized, Developer ID, not App Sandboxed, not in the Mac App Store.

Updates use Sparkle 2. Entitlement keys are taken from Sparkle’s documentation at implementation time (phase 12 investigation). This ADR does not invent them.

`com.apple.security.cs.disable-library-validation` is not set. User-installed plugins are WASM and do not need to be dylibs. First-party native code is inside the signed bundle.

Music folders are ordinary filesystem paths because we are not sandboxed. The import UI asks with a panel before watching a folder. It does not copy audio. We do not silently scan `~/Music`.

An Apple Developer Team ID is an input from the owner before phase 12. We do not invent a bundle id at notarization time. The working bundle id is unset until [ADR 012](012-working-name.md) passes. A placeholder in the repo, if one is required for the phase 0 smoke app, is clearly marked `UNREGISTERED` and is not submitted for notarization.

There is no telemetry and no analytics SDK. A direct-download app with Sparkle and no store analytics is debugged from what the user sends. The app writes a local log under Application Support. A crash writes a local report the user can attach. Nothing is sent unless they opt in. Phase 12 exits on that: a crash produces a local report, and a test or a review asserts no network call from the crash path when opt-in is off. Do not add a vendor SDK to close this.

## Consequences

- Hardened runtime without a sandbox is a conscious choice. WASM is what keeps untrusted code out of the process address space.
- App Store distribution later would be a new product shape (sandbox, no unsigned native code — already true — and security-scoped bookmarks). This ADR does not forbid that future. It forbids taking a dependency that exists only because we are unsandboxed, other than “user-granted folders are normal paths.” A later sandbox would bookmark those granted folders. Do not store security-sensitive data in a place only an unsandboxed app can read.
- Sparkle requires a network client entitlement and a key pair. The public key ships in the app. The private key does not enter the repo.
- An opt-in crash report is not telemetry. The default is silence. A later session that adds a beacon “so we know it launched” is off this ADR.

## Alternatives

- **Mac App Store only.** Rejected. Review, sandbox, and MusicKit-shaped entitlements would dominate 1.0, and a store build cannot be the place we learn the player.
- **Direct download, App Sandboxed.** Rejected for 1.0. Security-scoped bookmarks and the sandbox’s plugin limits are real work. We declined them so the player can ship. Granted folders are ordinary paths in 1.0. Bookmarks are the later-sandbox work, not a reason to copy the library.
- **Direct download, unsandboxed, library validation off, native user plugins.** Rejected. One entitlement weakens the whole process so that a DSP dylib can load. WASM is the plugin system instead.
