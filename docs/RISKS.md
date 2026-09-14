# Risks

Likelihood and impact are high, medium, or low. A risk retires when the mitigation has happened, not when this table exists. Owner of every row is whoever is in the phase named in the last column. There is no separate risk team.

| Risk | L | I | Mitigation | Retires |
| --- | --- | --- | --- | --- |
| GitHub `macos-26` runner stops scheduling, so the deployment target cannot be built in CI | M | H | The label has been generally available since 2026-02-26. Phase 0 confirms a run is scheduled. If it is not, stop and report. Do not silently lower the target or point CI at an older label. | 0 |
| Working name collides with Llammp, Winamp Group’s former Llama name, or Meta’s Llama mark | H | H | [ADR 012](adr/012-working-name.md) blocks bundle id, notarization, and a public repo name until the owner passes or renames. The mark is independent of the letters. | 12, gate before notarization |
| A referenced file disappears, or a granted folder is revoked, and the library silently drops the row | H | M | The row stays, marked missing. Playback skips or errors. The user removes missing rows. We watch granted folders only. [ADR 007](adr/007-library-database.md). | 5b |
| `cpal` 0.18 cannot hot-swap the default device | M | H | Phase 1 spike. Fallback is `coreaudio-rs`, and [ADR 006](adr/006-audio-output.md) is amended. Do not write that backend before the spike fails. | 1 |
| `symphonia` Opus is incomplete, or AAC-LC gapless is assumed by accident | H | M | Opus goes through `libopus`. AAC-LC is documented as not gapless. MP3 gapless waits on LAME/Xing delay being surfaced. Tests cover the formats that claim gapless. | 1 |
| HE-AAC radio fails closed because `symphonia` has no HE-AAC | H | M | macOS AudioToolbox decoder plugin, not `fdk-aac`, not FFmpeg. Stations we cannot decode error out. | 9 |
| EQ AUTO ships a guessed behavior | M | L | Button stays inert until `docs/investigations/eq-auto.md` exists from published notes. | 4 |
| Sprite coordinates copied from another player and wrong | M | H | Phase 2 golden PNG against a local fixture. This repo does not contain a transcribed Webamp atlas. Zip-slip, compression bombs, and overflow dimensions are rejected. `cargo-fuzz` covers the ZIP reader and BMP decoder. | 2 |
| Fractional DPI or Retina scaling blurs skins | M | H | Integer scale only, nearest filter. The failing test is a bilinear sample in the backing store, not a window-server screenshot. Scaled macOS display modes resample a 2× store; that softness is not an app bug. | 3 |
| Custom chrome is invisible to VoiceOver | H | H | Accessibility map is the hit-test map. Phase 3 exits with main-window labels. Phase 12 exits with all six. Not deferred past 1.0. | 12 |
| User-installed native dylibs force `disable-library-validation` | M | H | Not set. User plugins are WASM. [ADR 011](adr/011-distribution.md). | 6, confirmed at 12 |
| WASM on the audio thread causes dropouts | M | H | DSP and output are not user WASM. Allocator hook covers the callback. A visualizer kill test must not underrun. | 6 and 7 |
| Visualizer GPU work leaks into skin chrome and headless tests | M | M | wgpu is the visualizer surface only. [ADR 009](adr/009-visualizer-gpu.md). | 7 |
| Milkdrop compatibility eats the schedule | H | H | Not in v1. [ADR 010](adr/010-milkdrop.md). UI must not imply `.milk` loads. | 7, held by the ADR after 1.0 |
| projectM later pulls LGPL and unlicensed presets into the app | M | H | Shared library only, no bundled preset pack we do not own. Not started in 1.0. | Post-1.0, before any link line |
| Spotify public login is impossible for an individual, and 1.0 is defined as blocked on it | H | H | Bring-your-own Client ID. Extended quota is a written go/no-go, not a build breaker. [providers](spec/providers.md). | 10 |
| Someone embeds the Web Playback SDK or scrapes Spotify to get PCM | L | H | Forbidden in [LEGAL.md](LEGAL.md). Tier B has no PCM by design. | 10 |
| MusicKit catalog does not work outside the App Store | H | M | Spike first. Partial or no-go drops catalog or the provider. Local 1.0 does not wait on it. | 10 |
| Lyrics lookup sends more than we said, or runs before opt-in | M | H | Opt-in gate, fields listed in the UI, cache offline-first, test that a declined preference issues no request. | 8 |
| Commercial lyric APIs get scraped “just for the demo” | L | H | Forbidden. LRCLIB only. | 8 |
| `text.bmp` used for lyrics and CJK renders as boxes | H | M | CoreText path is specified before phase 5a. Playlist falls back per row. Library, browser, and lyrics never use the bitmap font for body text. | 5a and 8 |
| `lofty` or `rusqlite` pre-1.0 break on a bump | M | M | Behind traits. Pin in `Cargo.toml`. A bump is a deliberate change. Planning pin is `lofty` 0.25.2. | Ongoing, first pin in 1 and 5b |
| Winamp 2024 source or a Nullsoft skin lands in the tree | L | H | [LEGAL.md](LEGAL.md). Review of new binary assets in phase 11. | 11 |
| Sparkle entitlements on macOS 26 hardened runtime are wrong | M | H | Phase 12 investigation against Sparkle’s docs. Notarized update is the exit test. | 12 |
| Process tap added as a “quick” visualizer for Spotify | M | H | Not a feature. Spike is post-1.0, default recommendation no, indicator required if it ever exists. | Held |
| Subsonic/Jellyfin/Plex sneaks into 1.0 and slips the ship | M | M | Designed for, not scheduled. Cut order in [PLAN.md](PLAN.md). | Held through 12 |
| Three skins slip because pixel art takes longer than a phase | H | M | Checklist-complete, not gallery-complete. One skin can be the phase 3 development skin; phase 11 still exits only when three ship. If phase 11 slips, cut order does not drop the high-contrast skin. Drop polish on `dark` before dropping `contrast`. | 11 |
| The player is not small or not fast, and nothing measures it | H | H | Budgets in [PLAN.md](PLAN.md) are exit criteria at phase 3 and phase 12. Missing a number fails the phase. Do not raise a number in the same change that misses it. | 3, again at 12 |
| Windows starts before macOS 1.0 | L | H | Phase 13 does not start until phase 12’s notarized DMG exit is met. | 12 |
| Linux toolkit argument consumes phase 14 | M | L | GTK4 is already decided. The trigger is ABI stability, not a bake-off. [ADR 008](adr/008-linux-toolkit.md). | 14 |

## Investigations that are risks until written

These do not have a likelihood until someone does the work. They are listed so they are not forgotten. Paths:

- `docs/investigations/eq-auto.md` (phase 4)
- `docs/investigations/spotify-quota.md` (phase 10)
- `docs/investigations/musickit-direct.md` (phase 10)
- Radio Browser terms appended to [LEGAL.md](LEGAL.md) (phase 9)
- wasmtime version pin written into [plugin ABI](spec/plugin-abi.md) (phase 6), not a new product risk if the deny-test passes
- wgpu version pin written into [visualizer](spec/visualizer.md) (phase 7)
- Milkdrop cost note from a bounded reading of `mbrukman/winamp-macos` (phase 7). Do not open `pzzzy/macwamp`.
