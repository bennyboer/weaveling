CREATE TABLE projects (
    project     TEXT COLLATE "C" PRIMARY KEY,
    name        TEXT             NOT NULL,
    created_at  TIMESTAMPTZ      NOT NULL,
    updated_at  TIMESTAMPTZ      NOT NULL
);
