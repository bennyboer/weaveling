CREATE TABLE claims (
    kind        TEXT NOT NULL,
    key         TEXT NOT NULL,
    id          TEXT NOT NULL,
    claimed_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (kind, key)
);
