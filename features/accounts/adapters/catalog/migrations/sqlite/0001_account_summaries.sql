CREATE TABLE account_summaries (
    account       TEXT    PRIMARY KEY,
    version       INTEGER NOT NULL,
    name          TEXT    NOT NULL,
    email         TEXT    NOT NULL,
    verified      INTEGER NOT NULL,
    operator      INTEGER NOT NULL,
    signed_up_at  TEXT    NOT NULL,
    locks         TEXT    NOT NULL,
    second_factor INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE project_holdings (
    project       TEXT    NOT NULL,
    account       TEXT    NOT NULL,
    version       INTEGER NOT NULL,
    held          TEXT,
    PRIMARY KEY (project, account)
);

CREATE INDEX project_holdings_by_account ON project_holdings (account, project);

CREATE TABLE disbanded_projects (
    project       TEXT    PRIMARY KEY,
    version       INTEGER NOT NULL
);

CREATE TABLE trashed_projects (
    project       TEXT    PRIMARY KEY,
    version       INTEGER NOT NULL,
    in_the_trash  INTEGER NOT NULL
);
