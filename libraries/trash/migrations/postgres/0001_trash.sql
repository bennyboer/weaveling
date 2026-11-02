CREATE TABLE trash (
    kind      TEXT COLLATE "C" NOT NULL,
    id        TEXT COLLATE "C" NOT NULL,
    kept_until  TIMESTAMPTZ      NOT NULL,
    PRIMARY KEY (kind, id)
);

CREATE INDEX trash_due ON trash (kept_until, kind, id);
