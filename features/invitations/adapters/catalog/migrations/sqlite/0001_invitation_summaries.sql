CREATE TABLE invitation_summaries (
    invitation       TEXT    PRIMARY KEY,
    version          INTEGER NOT NULL,
    email            TEXT    NOT NULL,
    issued_by        TEXT,
    issued_at        TEXT    NOT NULL,
    until            TEXT    NOT NULL,
    outcome          TEXT    NOT NULL,
    declined_for     TEXT,
    project          TEXT,
    project_role     TEXT,
    retire_after     TEXT    NOT NULL
);

CREATE INDEX invitation_summaries_by_project ON invitation_summaries (project, invitation);

CREATE TABLE people (
    account          TEXT    PRIMARY KEY,
    name             TEXT    NOT NULL,
    email            TEXT    NOT NULL
);

CREATE TABLE project_names (
    project          TEXT    PRIMARY KEY,
    version          INTEGER NOT NULL,
    name             TEXT    NOT NULL
);

CREATE TABLE project_tombstones (
    project          TEXT    PRIMARY KEY,
    laid_at          TEXT    NOT NULL
);

CREATE INDEX project_tombstones_by_laid_at ON project_tombstones (laid_at);
