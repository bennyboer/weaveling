CREATE TABLE scene_texts (
    scene    TEXT COLLATE "C" PRIMARY KEY,
    version  BIGINT           NOT NULL,
    blocks   TEXT             NOT NULL
);

CREATE TABLE scene_text_tombstones (
    scene    TEXT COLLATE "C" PRIMARY KEY,
    laid_at  BIGINT           NOT NULL
);

CREATE INDEX gone_scene_texts_by_laid_at ON scene_text_tombstones (laid_at);
