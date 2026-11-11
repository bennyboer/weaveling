CREATE TABLE belongings (
    kind        TEXT NOT NULL,
    id          TEXT NOT NULL,
    owner_kind  TEXT NOT NULL,
    owner       TEXT NOT NULL,
    PRIMARY KEY (kind, id, owner_kind)
);

CREATE TABLE belonging_tombstones (
    kind     TEXT NOT NULL,
    id       TEXT NOT NULL,
    laid_at  TEXT NOT NULL,
    PRIMARY KEY (kind, id)
);

CREATE INDEX belonging_tombstones_by_laid_at ON belonging_tombstones (laid_at);
