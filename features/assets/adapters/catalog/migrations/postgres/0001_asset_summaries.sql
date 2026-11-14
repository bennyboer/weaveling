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

CREATE INDEX asset_summaries_by_uploader_largest ON asset_summaries (uploader, total_size DESC, id);
CREATE INDEX asset_summaries_by_uploader_newest ON asset_summaries (uploader, uploaded_at DESC, id);
CREATE INDEX asset_summaries_by_owner ON asset_summaries (owner_kind, owner);

CREATE TABLE asset_usage (
    bound  TEXT COLLATE "C" NOT NULL,
    id     TEXT COLLATE "C" NOT NULL,
    total  BIGINT           NOT NULL,
    PRIMARY KEY (bound, id)
);

CREATE INDEX asset_usage_by_total ON asset_usage (bound, total DESC, id);

CREATE TABLE asset_account_limits (
    account  TEXT COLLATE "C" PRIMARY KEY,
    most     BIGINT           NOT NULL
);

CREATE TABLE asset_people (
    account   TEXT COLLATE "C" PRIMARY KEY,
    version   BIGINT           NOT NULL,
    name      TEXT,
    email     TEXT,
    searched  TEXT,
    gone_at   TIMESTAMPTZ     
);

CREATE INDEX asset_people_by_gone_at ON asset_people (gone_at);

CREATE TABLE asset_project_names (
    project   TEXT COLLATE "C" PRIMARY KEY,
    version   BIGINT           NOT NULL,
    name      TEXT,
    searched  TEXT,
    gone_at   TIMESTAMPTZ     
);

CREATE INDEX asset_project_names_by_gone_at ON asset_project_names (gone_at);

CREATE TABLE asset_uses (
    purpose  TEXT COLLATE "C" NOT NULL,
    holder_kind TEXT COLLATE "C" NOT NULL,
    holder   TEXT COLLATE "C" NOT NULL,
    version  BIGINT           NOT NULL,
    asset    TEXT COLLATE "C",
    gone_at  TIMESTAMPTZ,
    PRIMARY KEY (purpose, holder_kind, holder)
);

CREATE INDEX asset_uses_by_asset ON asset_uses (asset);
CREATE INDEX asset_uses_by_gone_at ON asset_uses (gone_at);
