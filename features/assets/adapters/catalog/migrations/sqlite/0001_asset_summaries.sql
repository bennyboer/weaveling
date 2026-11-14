CREATE TABLE asset_summaries (
    id             TEXT PRIMARY KEY,
    owner_kind     TEXT    NOT NULL,
    owner          TEXT    NOT NULL,
    uploader       TEXT,
    format         TEXT    NOT NULL,
    width          INTEGER NOT NULL,
    height         INTEGER NOT NULL,
    original_size  INTEGER NOT NULL,
    total_size     INTEGER NOT NULL,
    uploaded_at    TEXT    NOT NULL
);

CREATE INDEX asset_summaries_by_uploader_largest ON asset_summaries (uploader, total_size DESC, id);
CREATE INDEX asset_summaries_by_uploader_newest ON asset_summaries (uploader, uploaded_at DESC, id);
CREATE INDEX asset_summaries_by_owner ON asset_summaries (owner_kind, owner);

CREATE TABLE asset_usage (
    bound  TEXT NOT NULL,
    id     TEXT NOT NULL,
    total  INTEGER NOT NULL,
    PRIMARY KEY (bound, id)
);

CREATE INDEX asset_usage_by_total ON asset_usage (bound, total DESC, id);

CREATE TABLE asset_account_limits (
    account  TEXT    PRIMARY KEY,
    most     INTEGER NOT NULL
);

CREATE TABLE asset_people (
    account   TEXT    PRIMARY KEY,
    version   INTEGER NOT NULL,
    name      TEXT,
    email     TEXT,
    searched  TEXT,
    gone_at   TEXT   
);

CREATE INDEX asset_people_by_gone_at ON asset_people (gone_at);

CREATE TABLE asset_project_names (
    project   TEXT    PRIMARY KEY,
    version   INTEGER NOT NULL,
    name      TEXT,
    searched  TEXT,
    gone_at   TEXT   
);

CREATE INDEX asset_project_names_by_gone_at ON asset_project_names (gone_at);

CREATE TABLE asset_uses (
    purpose  TEXT NOT NULL,
    holder_kind TEXT NOT NULL,
    holder   TEXT NOT NULL,
    version  INTEGER           NOT NULL,
    asset    TEXT,
    gone_at  TEXT,
    PRIMARY KEY (purpose, holder_kind, holder)
);

CREATE INDEX asset_uses_by_asset ON asset_uses (asset);
CREATE INDEX asset_uses_by_gone_at ON asset_uses (gone_at);
