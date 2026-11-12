CREATE TABLE asset_summaries (
    id             TEXT COLLATE "C" PRIMARY KEY,
    owner_kind     TEXT COLLATE "C" NOT NULL,
    owner          TEXT COLLATE "C" NOT NULL,
    uploader       TEXT COLLATE "C",
    format         TEXT COLLATE "C" NOT NULL,
    width          BIGINT           NOT NULL,
    height         BIGINT           NOT NULL,
    original_size  BIGINT           NOT NULL,
    total_size     BIGINT           NOT NULL,
    uploaded_at    TIMESTAMPTZ      NOT NULL
);

CREATE INDEX asset_summaries_by_uploader ON asset_summaries (uploader);
CREATE INDEX asset_summaries_by_owner ON asset_summaries (owner_kind, owner);

CREATE TABLE asset_usage (
    bound  TEXT COLLATE "C" NOT NULL,
    id     TEXT COLLATE "C" NOT NULL,
    total  BIGINT           NOT NULL,
    PRIMARY KEY (bound, id)
);

CREATE TABLE asset_memberships (
    project  TEXT COLLATE "C" NOT NULL,
    account  TEXT COLLATE "C" NOT NULL,
    version  BIGINT           NOT NULL,
    gone_at  TIMESTAMPTZ     ,
    PRIMARY KEY (project, account)
);

CREATE INDEX asset_memberships_by_account ON asset_memberships (account, project);
CREATE INDEX asset_memberships_by_gone_at ON asset_memberships (gone_at);

CREATE TABLE asset_disbanded_projects (
    project  TEXT COLLATE "C" PRIMARY KEY,
    version  BIGINT           NOT NULL,
    laid_at  TIMESTAMPTZ      NOT NULL
);

CREATE INDEX asset_disbanded_projects_by_laid_at ON asset_disbanded_projects (laid_at);

CREATE TABLE asset_account_limits (
    account  TEXT COLLATE "C" PRIMARY KEY,
    most     BIGINT           NOT NULL
);
