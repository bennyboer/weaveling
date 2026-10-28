CREATE TABLE invitation_summaries (
    invitation       TEXT COLLATE "C" PRIMARY KEY,
    version          BIGINT           NOT NULL,
    email            TEXT             NOT NULL,
    issued_at        TIMESTAMPTZ      NOT NULL,
    until            TIMESTAMPTZ      NOT NULL,
    outcome          TEXT             NOT NULL,
    declined_for     TEXT,
    project          TEXT COLLATE "C",
    project_role     TEXT,
    retire_after     TIMESTAMPTZ      NOT NULL
);

CREATE INDEX invitation_summaries_by_project ON invitation_summaries (project, invitation);

CREATE TABLE people (
    account          TEXT COLLATE "C" PRIMARY KEY,
    name             TEXT             NOT NULL,
    email            TEXT             NOT NULL
);
