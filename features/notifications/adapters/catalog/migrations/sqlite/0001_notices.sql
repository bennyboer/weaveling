CREATE TABLE notices (
    notice           TEXT    PRIMARY KEY,
    version          INTEGER NOT NULL,
    account          TEXT    NOT NULL,
    project          TEXT,
    about            TEXT    NOT NULL,
    noted_at         TEXT    NOT NULL,
    dismissed        INTEGER NOT NULL
);

CREATE INDEX notices_open_by_account ON notices (account, noted_at DESC, notice DESC)
    WHERE NOT dismissed;
