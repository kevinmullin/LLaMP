# Source providers

A provider is a `MediaSource` plugin plus capability flags on each playback session. The shell reads flags. It does not branch on `"spotify"` or `"apple-music"`.

## Tiers

**Tier A — we receive PCM.** EQ, DSP, visualizer, sample-accurate seek (if the format can seek), gapless, ReplayGain.

| Source | 1.0 | Notes |
| --- | --- | --- |
| Local referenced files | Yes | The first-party `MediaSource`. Paths the user granted. Not copies. |
| Icecast / SHOUTcast | Yes | PCM, usually not seekable, duration unknown |
| Station directory | Only if terms pass | See [PLAN.md](../PLAN.md) phase 9 |
| Podcast RSS | Yes | Downloaded into Application Support (`managed`), then it is a local file |
| Subsonic / OpenSubsonic, Jellyfin, Plex | No | Trait must allow them. Implementation is post-1.0. |

**Tier B — remote control, no PCM.** EQ, DSP, ReplayGain, gapless, and reactive visuals cannot apply. Seeking and volume apply only if the provider says so.

| Source | 1.0 | Notes |
| --- | --- | --- |
| Spotify Connect | Yes, bring-your-own Client ID | Not a public “Log in with Spotify” for an individual OSS app. See below. |
| Apple Music | Spike, then maybe | Swift shell, `ApplicationMusicPlayer`. No PCM. |

**Out of scope:** YouTube, SoundCloud, and anything that needs DRM circumvention or a ToS breach. Do not reopen this because a library already parses the URL.

## Flags

Published on the session snapshot:

| Flag | Local file | Radio | Podcast file | Spotify Connect | Apple Music |
| --- | --- | --- | --- | --- | --- |
| `produces_pcm` | yes | yes | yes | no | no |
| `supports_seek` | yes | no | yes | yes, remote | yes, if the player reports it |
| `supports_volume` | yes | yes | yes | yes, remote device volume | yes, if the player reports it |
| `supports_balance` | yes | yes | yes | no | no |
| `supports_eq` | yes | yes | yes | no | no |
| `supports_gapless` | yes, format permitting | no | yes, format permitting | no | no |
| `supports_replaygain` | yes, if tagged | no | yes, if tagged | no | no |
| `duration_known` | yes | no | yes | yes, if the API returns it | yes, if the player returns it |
| `position_authoritative` | sample clock | no | sample clock | polled | polled |
| `requires_network` | no | yes | no, once downloaded | yes | yes |

A flag that is “yes, remote” still means the command is sent to the provider. It does not mean we move a local playhead and hope.

## Tier B transport

Position is stored as provider position plus a local interpolation since the last successful poll. On each poll, snap to the provider’s number. Do not slew across a snap larger than two seconds; snap immediately, or the seek bar lies.

Poll interval: 1 second while playing, 5 seconds while paused. If a poll fails, keep the last position, freeze interpolation, and show “reconnecting” in the status line. After three failures, transport buttons stay enabled and the status stays on reconnecting. Do not clear the queue.

If the user seeks, send the seek and ignore interpolated motion until the next poll confirms. If the provider rejects the seek, snap back and show a status line, not a modal.

Volume for Tier B is the remote device’s volume, 0–100, not our pipeline gain. Balance is disabled.

Quitting the app does not pause a Connect device. The UI says playback continues on that device. Apple Music follows `ApplicationMusicPlayer` background rules if that provider shipped; do not invent a background mode to match Spotify.

## Degraded UI

When `supports_eq` is false:

- The EQ window still opens.
- Sliders draw in the disabled sprite state and do not move audio.
- The curve is a flat line in the disabled color from `viscolor.txt` index 1, not a fake curve of the last Tier A track.
- Caption, in the EQ title or the gen status line of that window: `Equalizer does not apply — playing on {device}`. If the device name is unknown: `Equalizer does not apply — playback is remote`.
- Preset menu is disabled.
- AUTO and ON do not enable the graph.

When `produces_pcm` is false:

- The 76×16 pane does not draw a spectrum or an oscilloscope. It draws a slow animation using `viscolor.txt` (scrolling dots, index 1 on index 0) and a remote glyph from the skin’s indicator sprites if one exists, otherwise a lit square in index 23. It must be obvious the pane is alive and not analyzing audio.
- The standalone visualizer window stays metadata-driven: artwork if we have it, title, artist, device name, skin colors. No fake FFT. Label: `No audio to visualize — playing on {device}`.
- A hidden visualizer still costs nothing.

Seek bar:

- If `supports_seek` and `duration_known`, the bar is draggable and the thumb shows the interpolated remote position. The accessibility label includes “remote”.
- If duration is unknown, the bar is a full indeterminate strip and does not seek.
- Sample-accurate wording does not appear.

Time display: provider position, or `--:--` if unknown. Elapsed/remaining still toggles when duration is known.

Main window status, always visible while Tier B is playing: `Playing on {device}`. That string is the honesty requirement. A user who thinks the EQ is broken has not been told the truth.

Lyrics: still tags (usually none), sidecars (usually none), then opt-in LRCLIB from the metadata the provider did give us. Spotify’s own lyrics are licensed and are not in the public API. Do not scrape them. Misses are expected. See [lyrics](lyrics.md).

## Spotify Connect

There is no official SDK that streams Spotify audio into a third-party native desktop app. `libspotify` is dead. The Web Playback SDK is browser-only, needs EME, and is not licensed for this. The mobile SDKs are remote controls for the official app. We do not try to get PCM.

1.0 is a Connect controller:

- OAuth PKCE.
- The user pastes their own Client ID. We do not embed a shared Client ID. Development Mode is one Client ID, five allowlisted users, and the owner needs Premium ([Spotify’s February 2026 note](https://developer.spotify.com/blog/2026-02-06-update-on-developer-access-and-platform-security)). Extended Quota is organization-only as of May 2025. A public “just log in” cannot ship from an individual’s OSS repo.
- Browse and search via the Web API endpoints Development Mode still allows at implementation time. Phase 10 reads the then-current migration guide and deletes calls to removed endpoints. This spec does not freeze a path that Spotify has already narrowed.
- Transfer playback to a Connect device, send transport commands, poll player state.
- Setup UI explains the five-user cap and that this is the user’s own Spotify application, not ours.

Public extended-quota review is a written go/no-go in `docs/investigations/spotify-quota.md`. A no-go leaves bring-your-own Client ID in place. It does not block the notarized build.

Secrets: PKCE has no client secret in the app. A Client ID the user pasted is stored in the keychain, not in the SQLite file, not in the repo.

## Apple Music

Lives in the Swift shell. `ApplicationMusicPlayer` plays. It does not expose PCM. Do not tap it.

Phase 10 starts with a spike: can a notarized, non-App-Store binary authorize MusicKit and search the catalog? Confidence is under 80%. Outcomes:

- Go: library and catalog, still Tier B, same degraded UI, device name “Apple Music”.
- Partial: user’s library only, catalog hidden.
- No-go: provider not shipped. Do not scrape the Apple Music website.

Catalog requests that need a registered bundle id wait for [ADR 012](../adr/012-working-name.md). Do not register a placeholder id to make the spike pass.

## Radio and podcasts

Radio is Tier A when the codec decodes. HE-AAC uses the AudioToolbox decoder plugin on macOS. A station we cannot decode shows an error and does not pretend to play.

ICY metadata updates the marquee. It does not overwrite library tags of a downloaded file.

Podcasts download into Application Support. Those rows are `managed`. Music import does not use this path. Retention: the in-progress episode, plus the five most recently completed, unless pinned. Default is not “download the entire feed.” A feed import shows how many episodes will download and how much space, and refuses before any write if the disk cannot hold them.

## Adding a provider

A new provider is a `MediaSource` that sets flags honestly. If it does not have PCM, `produces_pcm` is false and the degraded UI falls out. Do not add a special case in the EQ window for the new name.
