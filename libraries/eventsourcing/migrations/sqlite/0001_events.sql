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

CREATE TABLE outbox (
    entry           INTEGER PRIMARY KEY AUTOINCREMENT,
    aggregate       TEXT    NOT NULL,
    kind            TEXT    NOT NULL,
    version         INTEGER NOT NULL,
    message_id      TEXT    NOT NULL,
    conversation    TEXT    NOT NULL,
    caused_by       TEXT,
    routing_key     TEXT    NOT NULL,
    payload         TEXT    NOT NULL,
    occurred_at     TEXT    NOT NULL,
    written_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    claimed_until   TEXT,
    published_at    TEXT
);

CREATE INDEX outbox_waiting ON outbox (entry) WHERE published_at IS NULL;
CREATE INDEX outbox_published ON outbox (published_at) WHERE published_at IS NOT NULL;
