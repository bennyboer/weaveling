CREATE TABLE project_summaries (
    project     TEXT COLLATE "C" PRIMARY KEY,
    version     BIGINT           NOT NULL,
    name        TEXT             NOT NULL,
    created_at  TIMESTAMPTZ      NOT NULL,
    updated_at  TIMESTAMPTZ      NOT NULL
);
