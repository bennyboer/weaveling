CREATE TABLE scene_texts (
    scene    TEXT    PRIMARY KEY,
    version  INTEGER NOT NULL,
    blocks   TEXT    NOT NULL
);

CREATE TABLE scene_text_tombstones (
    scene    TEXT    PRIMARY KEY,
    laid_at  INTEGER NOT NULL
);

CREATE INDEX gone_scene_texts_by_laid_at ON scene_text_tombstones (laid_at);
