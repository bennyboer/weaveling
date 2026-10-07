CREATE TABLE deliveries (
    delivery        INTEGER PRIMARY KEY AUTOINCREMENT,
    listener        TEXT    NOT NULL,
    message_id      TEXT    NOT NULL,
    conversation    TEXT    NOT NULL,
    caused_by       TEXT,
    routing_key     TEXT    NOT NULL,
    payload         TEXT    NOT NULL,
    occurred_at     TEXT    NOT NULL,
    attempts        INTEGER NOT NULL DEFAULT 0,
    due_at          TEXT    NOT NULL,
    claimed_until   TEXT,
    last_refusal    TEXT
);

CREATE INDEX deliveries_due ON deliveries (due_at, delivery);

CREATE TABLE dead_letters (
    dead_letter     INTEGER PRIMARY KEY AUTOINCREMENT,
    listener        TEXT    NOT NULL,
    message_id      TEXT    NOT NULL,
    conversation    TEXT    NOT NULL,
    caused_by       TEXT,
    routing_key     TEXT    NOT NULL,
    payload         TEXT    NOT NULL,
    occurred_at     TEXT    NOT NULL,
    attempts        INTEGER NOT NULL,
    why             TEXT    NOT NULL,
    given_up_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
