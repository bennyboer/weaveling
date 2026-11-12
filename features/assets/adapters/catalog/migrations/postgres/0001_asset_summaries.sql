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
