CREATE TABLE account_summaries (
    account       TEXT COLLATE "C" PRIMARY KEY,
    version       BIGINT           NOT NULL,
    name          TEXT             NOT NULL,
    email         TEXT             NOT NULL,
    verified      BOOLEAN          NOT NULL,
    operator      BOOLEAN          NOT NULL,
    signed_up_at  TIMESTAMPTZ      NOT NULL,
    locks         JSONB            NOT NULL
);
