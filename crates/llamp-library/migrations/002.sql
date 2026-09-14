-- v2. Nullable tag columns and new tables. Existing v1 rows stay.
ALTER TABLE tracks ADD COLUMN title TEXT;
ALTER TABLE tracks ADD COLUMN artist TEXT;
ALTER TABLE tracks ADD COLUMN album TEXT;
ALTER TABLE tracks ADD COLUMN genre TEXT;

CREATE TABLE grants (
    path TEXT PRIMARY KEY
);

CREATE TABLE artwork (
    id INTEGER PRIMARY KEY,
    track_id INTEGER NOT NULL,
    path TEXT NOT NULL,
    bytes INTEGER NOT NULL,
    last_used INTEGER NOT NULL
);

CREATE TABLE playlist_items (
    position INTEGER PRIMARY KEY,
    path TEXT NOT NULL
);

CREATE VIRTUAL TABLE tracks_fts USING fts5(
    title,
    artist,
    album,
    genre,
    tokenize = 'unicode61'
);
