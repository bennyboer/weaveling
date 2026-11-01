CREATE TABLE project_summaries (
    project     TEXT COLLATE "C" PRIMARY KEY,
    version     BIGINT           NOT NULL,
    name        TEXT             NOT NULL,
    created_at  TIMESTAMPTZ      NOT NULL,
    updated_at  TIMESTAMPTZ      NOT NULL
);

CREATE TABLE project_tombstones (
    project          TEXT COLLATE "C" PRIMARY KEY,
    laid_at          TIMESTAMPTZ      NOT NULL
);

CREATE INDEX project_tombstones_by_laid_at ON project_tombstones (laid_at);
