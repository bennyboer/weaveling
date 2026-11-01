CREATE TABLE idea_summaries (
    idea        TEXT    PRIMARY KEY,
    version     INTEGER NOT NULL,
    project     TEXT    NOT NULL,
    title       TEXT    NOT NULL
);

CREATE INDEX idea_summaries_by_project ON idea_summaries (project, idea DESC);

CREATE TABLE idea_tombstones (
    idea             TEXT PRIMARY KEY,
    laid_at          TEXT NOT NULL
);

CREATE INDEX idea_tombstones_by_laid_at ON idea_tombstones (laid_at);
