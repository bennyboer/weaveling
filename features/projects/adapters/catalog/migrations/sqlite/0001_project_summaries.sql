CREATE TABLE project_summaries (
    project     TEXT    PRIMARY KEY,
    version     INTEGER NOT NULL,
    name        TEXT    NOT NULL,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

CREATE TABLE project_tombstones (
    project          TEXT PRIMARY KEY,
    laid_at          TEXT NOT NULL
);

CREATE INDEX project_tombstones_by_laid_at ON project_tombstones (laid_at);
