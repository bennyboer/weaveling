CREATE TABLE events (
    aggregate       TEXT    NOT NULL,
    kind            TEXT    NOT NULL,
    version         INTEGER NOT NULL,
    name            TEXT    NOT NULL,
    body            TEXT    NOT NULL,
    body_version    INTEGER NOT NULL,
    agent           TEXT    NOT NULL,
    occurred_at     TEXT    NOT NULL,
    is_snapshot     INTEGER NOT NULL,
    written_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (aggregate, kind, version)
);

CREATE INDEX events_snapshots ON events (aggregate, kind, version DESC) WHERE is_snapshot;
