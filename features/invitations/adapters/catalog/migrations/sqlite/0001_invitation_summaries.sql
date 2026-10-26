CREATE TABLE invitation_summaries (
    invitation       TEXT    PRIMARY KEY,
    version          INTEGER NOT NULL,
    email            TEXT    NOT NULL,
    issued_at        TEXT    NOT NULL,
    until            TEXT    NOT NULL,
    outcome          TEXT    NOT NULL,
    declined_for     TEXT,
    project          TEXT,
    project_role     TEXT,
    retire_after     TEXT    NOT NULL
);

CREATE INDEX invitation_summaries_by_project ON invitation_summaries (project, invitation);
