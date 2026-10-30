CREATE TABLE notices (
    notice           TEXT COLLATE "C" PRIMARY KEY,
    version          BIGINT           NOT NULL,
    account          TEXT COLLATE "C" NOT NULL,
    project          TEXT COLLATE "C",
    about            JSONB            NOT NULL,
    noted_at         TIMESTAMPTZ      NOT NULL,
    dismissed        BOOLEAN          NOT NULL
);

CREATE INDEX notices_open_by_account ON notices (account, noted_at DESC, notice DESC)
    WHERE NOT dismissed;
