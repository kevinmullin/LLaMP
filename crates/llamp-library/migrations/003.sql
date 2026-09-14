-- v3. Lyrics cache, opt-in, and offset fallback. Existing rows stay.
CREATE TABLE lyrics_cache (
    id INTEGER PRIMARY KEY,
    track_id INTEGER,
    artist TEXT NOT NULL,
    title TEXT NOT NULL,
    album TEXT NOT NULL,
    duration_seconds INTEGER NOT NULL,
    body TEXT,
    source TEXT NOT NULL,
    kind TEXT NOT NULL,
    fetched_at INTEGER NOT NULL,
    UNIQUE (artist, title, album, duration_seconds)
);

CREATE TABLE lyrics_prefs (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE lyrics_offsets (
    path TEXT PRIMARY KEY,
    track_id INTEGER,
    offset_ms INTEGER NOT NULL
);
