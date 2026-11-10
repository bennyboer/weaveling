CREATE TABLE account_summaries (
    account       TEXT COLLATE "C" PRIMARY KEY,
    version       BIGINT           NOT NULL,
    name          TEXT             NOT NULL,
    email         TEXT             NOT NULL,
    verified      BOOLEAN          NOT NULL,
    operator      BOOLEAN          NOT NULL,
    signed_up_at  TIMESTAMPTZ      NOT NULL,
    locks         JSONB            NOT NULL,
    second_factor BOOLEAN          NOT NULL DEFAULT FALSE,
    deletion_until    TIMESTAMPTZ,
    deletion_asked_by TEXT
);

CREATE TABLE project_holdings (
    project       TEXT COLLATE "C" NOT NULL,
    account       TEXT COLLATE "C" NOT NULL,
    version       BIGINT           NOT NULL,
    held          TEXT,
    gone_at       TIMESTAMPTZ,
    PRIMARY KEY (project, account)
);

CREATE INDEX project_holdings_by_account ON project_holdings (account, project);

CREATE INDEX project_holdings_by_gone_at ON project_holdings (gone_at);

CREATE TABLE disbanded_projects (
    project       TEXT COLLATE "C" PRIMARY KEY,
    version       BIGINT           NOT NULL,
    laid_at       TIMESTAMPTZ      NOT NULL
);

CREATE TABLE trashed_projects (
    project       TEXT COLLATE "C" PRIMARY KEY,
    version       BIGINT           NOT NULL,
    in_the_trash  BOOLEAN          NOT NULL
);
