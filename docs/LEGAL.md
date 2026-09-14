# Legal

This is an engineering constraint list, not a legal opinion. Where it says “do not,” a later session does not get to be clever. Where it says a gate is open, the owner closes it before we ship that thing.

Product name rules are in [ADR 012](adr/012-working-name.md). The mark is in [brand](spec/brand.md).

## Winamp source and skins

Do not use the Winamp source released in 2024. The Winamp Collaborative License forbids distributing modified versions, the repository was later pulled, and it contained third-party code the publisher was not in a position to grant. It is not a safe reference. If a file from that tree appears in a dependency, a paste, or a “reference” folder, remove it.

Clean-room only: published format documentation, and permissively licensed reimplementations read for behavior. Webamp (MIT) may be read for skin-format behavior. Copying its source into this repo requires an attribution review that has not been done. Do not copy it. Do not copy Llammp or any other player to save a weekend.

Prior art we may read and must not copy, confirmed or noted 2026-09-13. The list lives in [PLAN.md](PLAN.md):

- `yoanbernabeu/llammp` (MIT). Apple Music client. Do not copy.
- `mbrukman/winamp-macos` (MIT), a fork of `mgreenwood1001/winamp`. The original repo 404s. A bounded reading of the fork’s Milkdrop path is allowed before phase 7. Do not copy it. Do not vendor its visualizer.
- `wishval/wamp` (MIT). Do not copy.

Do not open, same rule as the Winamp drop:

- `pzzzy/macwamp` (GPL-3.0). Note that it exists. Screenshots only. Do not read the files.
- `SawyerChristensen/Prism` embeds projectM through ANGLE. Verify its license before anyone reads the tree. Do not vendor it into 1.0.

Do not redistribute Nullsoft’s base skin or any third-party skin. Users import their own `.wsz`. We ship three skins we authored. See [skin format](spec/skin-format.md).

“Winamp” is a trademark of its owner. The only acceptable use in our UI, docs, and store-style copy is descriptive: “compatible with classic Winamp skins.” Do not use it in the product name, the bundle id, the menu-bar label, or a domain. Do not use the whipping-llama splash or any Nullsoft graphic. Our llama is the file in `assets/brand/`, which we were given by the owner to use as our mark.

## Our license

Code we write is MIT OR Apache-2.0, dual licensed, except where a file says otherwise. Contributions are accepted under that dual license.

Dependencies we do not own stay on their licenses:

| Piece | License | Rule |
| --- | --- | --- |
| `symphonia` | MPL-2.0 | File-level copyleft. Do not strip notices. A modified MPL file stays MPL. Prefer not to fork. |
| `libopus` | BSD-3 | OK to link. Keep its notice. |
| `libopenmpt` | BSD-3-style | Only if the post-1.0 plugin lands. Keep its notice. |
| SQLite (bundled) | Public domain | OK. |
| projectM | LGPL-2.1 | Not in 1.0. If it lands, dynamic shared library only, presets we have rights to only. See [ADR 010](adr/010-milkdrop.md). |
| GTK, on Linux, system library | LGPL | Dynamic link to the system GTK. Do not vendor. |
| Sparkle | MIT | OK. Private update key never in the repo. |

No FFmpeg. No `fdk-aac`. AAC-LC comes from `symphonia`. HE-AAC on macOS comes from AudioToolbox, a system framework, not from a codec we ship. MP3 patents are expired; we do not add a patent-license codec “just in case.”

## Distribution

Direct download, notarized, hardened runtime, not sandboxed, not the Mac App Store. See [ADR 011](adr/011-distribution.md). We do not set `disable-library-validation`. User plugins are WASM.

Notarization needs the owner’s Team ID. Do not create an Apple ID or accept a developer agreement on the owner’s behalf.

## Lyrics and metadata services

Do not bundle lyrics. Cache is a local performance copy of a lookup the user asked for, keyed to the track, shown with the provider’s name.

LRCLIB is the default remote provider. It is opt-in. We send artist, title, album, and duration only, to `lrclib.net`. That sentence is the privacy note in the UI. Do not add the filename, path, or IP-identifying payload of our own. Their server will see an IP; the note should say a network request reveals that we asked, in the ordinary way. Do not over-claim anonymity.

Musixmatch, Genius, and similar commercial lyric catalogs need agreements. Scraping them is forbidden. Spotify’s lyrics are not a public API. Do not scrape them either.

Do not publish lyrics back to LRCLIB in 1.0. Their publish endpoint exists. We do not call it until a later ADR says the user consented to upload and we know what we are uploading.

## Spotify

Bring-your-own Client ID, PKCE, no client secret in the app. A Client ID the user pastes goes to the keychain. Extended-quota public distribution is an investigation with a likely no-go for an individual maintainer. Do not embed a shared Client ID and call it a public feature. Do not wrap the Web Playback SDK in a hidden web view to get PCM. That SDK is not licensed for this, and it would be a ToS bypass even if the trick worked.

## Apple Music

MusicKit only, and only if the phase 10 spike says a notarized non-App-Store binary is allowed to call it. No scraping. No PCM tap of `ApplicationMusicPlayer`.

## Radio directories

A station directory is shipped only after its terms are pasted under this heading and they allow an OSS desktop client. If they do not, the feature is “add a station URL.”

## Process taps

Not a feature. Capturing other applications’ audio is a consent problem even when Apple provides an API. It does not become a feature without a new ADR that includes the consent string, a persistent indicator, and a default of off.

## Trademarks that are not ours

Winamp, Spotify, Apple Music, and Meta Llama are not ours. UI copy names them only to say what a button does (“Play on Spotify device”) or to disclaim affiliation. The About window does not show their logos.
