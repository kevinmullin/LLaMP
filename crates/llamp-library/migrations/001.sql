-- v1. Enough to store a referenced track. Later columns are a later user_version.
CREATE TABLE tracks (
    id INTEGER PRIMARY KEY,
    storage TEXT NOT NULL CHECK (storage IN ('referenced', 'managed')),
    path TEXT NOT NULL UNIQUE,
    missing INTEGER NOT NULL DEFAULT 0
);
