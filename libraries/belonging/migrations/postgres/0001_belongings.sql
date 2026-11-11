CREATE TABLE belongings (
    kind        TEXT COLLATE "C" NOT NULL,
    id          TEXT COLLATE "C" NOT NULL,
    owner_kind  TEXT COLLATE "C" NOT NULL,
    owner       TEXT COLLATE "C" NOT NULL,
    PRIMARY KEY (kind, id, owner_kind)
);

CREATE TABLE belonging_tombstones (
    kind     TEXT COLLATE "C" NOT NULL,
    id       TEXT COLLATE "C" NOT NULL,
    laid_at  TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (kind, id)
);

CREATE INDEX belonging_tombstones_by_laid_at ON belonging_tombstones (laid_at);
