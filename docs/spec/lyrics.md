# Lyrics

Lyrics are a window and a lookup chain, not a decoration on the marquee. The marquee stays the song title. Synced lines live in the lyrics window.

Text rendering uses CoreText. `text.bmp` cannot carry CJK, Cyrillic, Arabic, or most accented lyrics. The rule is in [skin format](skin-format.md) and is repeated here so phase 8 does not “just use the bitmap font.”

## Sources, in order

1. Embedded tags: ID3v2 `USLT` (unsynced) and `SYLT` (synced), Vorbis `LYRICS`, MP4 `©lyr`.
2. Sidecar files next to the referenced audio file: `same-name.lrc`, then `same-name.txt` only if it looks like LRC (a line with a timestamp). A `.txt` that is a poem is not treated as synced. For a `managed` file the app created, the sidecar sits next to that file under Application Support.
3. Remote lookup, opt-in. Default provider is LRCLIB. No API key. Match on artist, title, album, and duration.

Stop at the first source that yields a non-empty lyric. A later source does not override an embedded synced lyric with a remote unsynced one. If embedded is unsynced and a sidecar is synced, the sidecar wins, because the user put it there.

### Remote

`GET https://lrclib.net/api/get` with `track_name`, `artist_name`, `album_name`, and `duration` in seconds. LRCLIB’s documented match window is about ±2 seconds. Send a `User-Agent` that identifies LLaMP and a version. Do not send the file path, the content hash, or an account id. We do not have an account.

LRCLIB is opt-in. The first time a track has no local lyric, the empty state offers “Look up lyrics” and a line that says we will send artist, title, album, and duration to LRCLIB. Decline is remembered. Accept is remembered. There is no silent lookup.

Musixmatch and Genius are licensing-gated. They are not implemented. Scraping them is out of scope. Spotify’s lyrics are not in the public API. Tier B tracks use the same metadata lookup and will often miss. The empty state may say the lyrics service had no match. It must not say Spotify withheld lyrics in a way that implies we tried to take them.

Cache: SQLite `lyrics_cache`, keyed by track id for library files and by the signature (artist, title, album, duration seconds) for Tier B. Offline reads the cache and never the network. A cached miss (instrumental, or no match) is stored so we do not hammer LRCLIB. Retry after 30 days, or when the user asks again.

Do not ship a lyrics corpus in the app bundle. Attribute LRCLIB in the window when the line came from there.

## Formats

### LRC

Accept:

- `[mm:ss.xx]` and `[mm:ss.xxx]` and `[mm:ss]` line timestamps. More than one timestamp on a line duplicates the line at each time.
- `[offset:±ms]` applied to every timestamp, including word times.
- ID tags (`[ar:]`, `[ti:]`, `[al:]`, `[length:]`) stored and not shown as lyrics.
- Enhanced LRC: `<mm:ss.xx>` word times inside a line.

Malformed files: skip the bad line, keep the rest. A file with no valid timestamp is unsynced plain text if it has any non-tag text, otherwise a miss. Do not fail the track load because a sidecar is garbage.

### Tags

`SYLT` becomes the same timed-line model as LRC. `USLT` and `©lyr` and unsynced Vorbis `LYRICS` are plain text. If both `USLT` and `SYLT` exist, use `SYLT`.

## Sync

The clock is the playback clock: the sample-frame counter for Tier A, the provider position for Tier B. Not the wall clock. Pause freezes the line. Seek jumps the line. A resampler change does not accumulate error, because we never integrate `dt` from the wall.

Active line: the last line whose start is less than or equal to the clock plus the user offset. Word highlight: the last word time less than or equal to that same value, if the line has word times. No interpolation of a caret between words in 1.0. The highlight snaps.

Drift tolerance: after a seek or a provider snap, the line must match that rule within one display frame (at 60 Hz, 16 ms). A test seeks to a timestamp 10 ms before a line and asserts the previous line is active, then 10 ms after and asserts the new line is active.

User offset: a control in the lyrics window, ±10 seconds, step 50 ms. “Save” writes `[offset:]` into the sidecar, creating `same-name.lrc` next to the audio file if that directory is writable and the lyric was embedded or remote and the user has edited the offset. If the directory is not writable, the offset is stored in SQLite and the UI says the sidecar was not written. We do not copy the audio file to gain a writable sidecar. We do not write offset back into ID3 in 1.0 (tag write of `SYLT` is easy to get wrong and to destroy a frame). Remote lyrics the user has not edited stay in the cache only.

Tier B: the same rules on the provider clock. Expect misses. Do not delay the highlight to hide poll jitter; snap with the position snap.

## Display

The lyrics window is `gen` chrome. See [skin format](skin-format.md).

- Synced: scrolling list, active line in `Current`, other lines in `Normal`, background `NormalBG`. Active line stays in the vertical center when it can.
- Enhanced: the active word uses `Current`; the rest of the active line uses `Normal` on `SelectedBG` or the inverse. Pick one in phase 8 and lock it with a screenshot. Do not use a gradient.
- Unsynced: plain text, no fake highlight that advances on a timer.
- Empty: short message, the brand mark at integer scale, and the lookup button if the user has not opted in. See [brand](brand.md).
- Colors from `pledit.txt`, so a skin change recolors lyrics. The mark keeps its own colors.

Right-to-left scripts: CoreText’s direction, not a hand-rolled reverser. If we cannot shape Arabic in the phase 8 spike, the investigation note says so and we still display the string rather than hiding the window. Do not claim full bidirectional layout until a fixture proves it.

## Editor

A tap-to-sync editor that builds an `.lrc` from unsynced text is post-1.0. The sidecar writer for offset is the only write path in 1.0, and it must not block that later editor. Saving offset must round-trip through the parser.

## Legal

Covered in [LEGAL.md](../LEGAL.md). Short version: opt-in, minimal fields, attribution, no bundled lyrics, no scraping of licensed catalogs.
