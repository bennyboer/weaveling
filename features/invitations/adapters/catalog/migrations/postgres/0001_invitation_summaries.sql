CREATE TABLE invitation_summaries (
    invitation    TEXT COLLATE "C" PRIMARY KEY,
    version       BIGINT           NOT NULL,
    email         TEXT             NOT NULL,
    issued_at     TIMESTAMPTZ      NOT NULL,
    until         TIMESTAMPTZ      NOT NULL,
    outcome       TEXT             NOT NULL,
    declined_for  TEXT
);
