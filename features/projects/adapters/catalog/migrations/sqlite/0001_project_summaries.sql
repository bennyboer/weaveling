CREATE TABLE project_summaries (
    project     TEXT    PRIMARY KEY,
    version     INTEGER NOT NULL,
    name        TEXT    NOT NULL,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);
