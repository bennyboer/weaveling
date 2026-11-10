CREATE TABLE account_summaries (
    account       TEXT COLLATE "C" PRIMARY KEY,
    version       BIGINT           NOT NULL,
    name          TEXT             NOT NULL,
    email         TEXT             NOT NULL,
    verified      BOOLEAN          NOT NULL,
    operator      BOOLEAN          NOT NULL,
    signed_up_at  TIMESTAMPTZ      NOT NULL,
    locks         JSONB            NOT NULL,
    second_factor BOOLEAN          NOT NULL DEFAULT FALSE
);

CREATE TABLE project_holdings (
    project       TEXT COLLATE "C" NOT NULL,
    account       TEXT COLLATE "C" NOT NULL,
    version       BIGINT           NOT NULL,
    held          TEXT,
    PRIMARY KEY (project, account)
);

CREATE INDEX project_holdings_by_account ON project_holdings (account, project);

CREATE TABLE disbanded_projects (
    project       TEXT COLLATE "C" PRIMARY KEY,
    version       BIGINT           NOT NULL
);

CREATE TABLE trashed_projects (
    project       TEXT COLLATE "C" PRIMARY KEY,
    version       BIGINT           NOT NULL,
    in_the_trash  BOOLEAN          NOT NULL
);
