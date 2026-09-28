CREATE TABLE claims (
    kind        TEXT COLLATE "C" NOT NULL,
    key         TEXT COLLATE "C" NOT NULL,
    id          TEXT COLLATE "C" NOT NULL,
    claimed_at  TIMESTAMPTZ      NOT NULL DEFAULT NOW(),
    PRIMARY KEY (kind, key)
);
