# ADR 007 — SQLite, referenced library, local only

- Status: accepted
- Date: 2026-09-13
- Amended: 2026-09-13. The owner chose reference-in-place. An earlier draft asserted import-by-copy as a product decision that had never been made.

## Context

The library needs search, play counts, artwork, and a stable id for lyrics cache and playlists. There are no accounts and no multi-device sync. The schema must not be bent toward a hypothetical sync service.

The owner chose play-in-place on 2026-09-13. The target library is a folder tree the user already curated. Copying it into an app-managed folder doubles disk and treats a Finder delete as a library delete. Those are not 1.0 features.

## Decision

One SQLite database via `rusqlite` with the `bundled` feature and FTS5. The database file, the artwork cache, and files the app itself creates live in Application Support. Music files stay where the user put them.

`rusqlite` is pre-1.0. SQL is written against SQLite, not against a query builder that hides it. Replacing `rusqlite` later means a thin connection wrapper, not a migration of query logic into another ORM.

Tags go through one trait. `lofty` 0.25 is the first reader and writer. It is pre-1.0. If ReplayGain or `SYLT` comes back wrong, we change the implementation behind the trait.

1.0 music import grants folders and watches those folders only. It does not scan `~/Music`. It does not copy audio. Track identity is the granted path. A content hash may be stored lazily, for lyrics identity and for noticing the same file granted twice. The hash is not an import gate and it is not a dedup-by-copy.

`tracks.storage` has two legal values. `referenced` is the 1.0 default and the only value music import writes. `managed` is reserved for files the app created (podcast downloads in phase 9). It is not a second music importer. The column exists so that later mode is a value add, not a new schema. There is no user id, no replica id, and no change-log table.

A referenced file missing on disk stays in the library with a missing status. Playback skips or errors. The user removes missing rows. 1.0 does not auto-relink and does not silently drop the row because Finder deleted the file.

Schema changes go through SQLite `user_version` and ordered migration scripts in `llamp-library`. A later version upgrades a previous database without losing rows. Phase 5b exits on that test, even if the v2 change is only a nullable column.

Core preferences (volume, ReplayGain preamp, crossfade, last skin, EQ preset) live in a SQLite key/value table so Windows and Linux inherit them. Window geometry lives in a shell-owned file. Neither is `NSUserDefaults`. Neither waits on the naming gate.

Native playlist files are ours. Import and export of M3U, M3U8, PLS, and XSPF is required. M3U8 round-trip must not lose order, and it must keep paths as written (absolute, or relative to the playlist file). It does not rewrite them under an app folder.

Artwork cache default cap is 512 MiB, least-recently-used eviction. Eviction deletes cache files and rows. It does not delete audio.

## Consequences

- A 2 TB library does not need 2 TB free to import.
- Deleting a referenced file in Finder marks the row missing. The import UI says so. It does not treat that delete as “remove from library.”
- Playlist remove-from-library drops the row. It does not move the user’s only copy to Trash. Trash is not a default menu item.
- App-created downloads (phase 9) live under Application Support, not a surprise folder in `~/Music`. Those rows use `managed`. A free-space check applies before those writes. It does not apply to music import.
- Sidecar lyrics sit next to the referenced file when that directory is writable. If it is not, the offset is stored in SQLite and the UI says so. We do not copy the audio file to gain a writable sidecar.
- `rusqlite` and `lofty` can be replaced without a product change.
- We do not build sync “just in case.”

## Alternatives

- **Import-by-copy into an app-managed folder.** Rejected for 1.0 by the owner on 2026-09-13. It is the losing fit for a large curated tree. It is not a hidden flag.
- **Both copy and reference as music importers in 1.0.** Rejected. Two importers before the first one works. `managed` remains for files the app created.
- **A document database or a custom binary index.** Rejected. SQLite FTS5 is the boring tool that already does the search we need.
- **iCloud or an account table “for later.”** Rejected. Local-only was explicit. A sync-shaped schema is how a later rewrite pretends it was planned.
