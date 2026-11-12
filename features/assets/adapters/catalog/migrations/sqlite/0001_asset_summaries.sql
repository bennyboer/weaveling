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

CREATE INDEX asset_summaries_by_uploader ON asset_summaries (uploader);
CREATE INDEX asset_summaries_by_owner ON asset_summaries (owner_kind, owner);

CREATE TABLE asset_usage (
    bound  TEXT NOT NULL,
    id     TEXT NOT NULL,
    total  INTEGER NOT NULL,
    PRIMARY KEY (bound, id)
);

CREATE TABLE asset_memberships (
    project  TEXT NOT NULL,
    account  TEXT NOT NULL,
    PRIMARY KEY (project, account)
);

CREATE INDEX asset_memberships_by_account ON asset_memberships (account, project);
