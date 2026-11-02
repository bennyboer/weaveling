CREATE TABLE trash (
    kind      TEXT NOT NULL,
    id        TEXT NOT NULL,
    kept_until  TEXT NOT NULL,
    PRIMARY KEY (kind, id)
);

CREATE INDEX trash_due ON trash (kept_until, kind, id);
