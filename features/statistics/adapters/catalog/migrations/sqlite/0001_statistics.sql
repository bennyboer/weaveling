CREATE TABLE word_counts (
    scene       TEXT    PRIMARY KEY,
    project     TEXT    NOT NULL,
    words       INTEGER NOT NULL,
    counted_at  INTEGER NOT NULL
);

CREATE INDEX word_counts_by_project ON word_counts (project, scene);

CREATE TABLE scene_tombstones (
    scene    TEXT    PRIMARY KEY,
    laid_at  INTEGER NOT NULL
);

CREATE INDEX gone_scenes_by_laid_at ON scene_tombstones (laid_at);
