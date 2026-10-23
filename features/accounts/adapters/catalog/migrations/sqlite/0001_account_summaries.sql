CREATE TABLE account_summaries (
    account       TEXT    PRIMARY KEY,
    version       INTEGER NOT NULL,
    name          TEXT    NOT NULL,
    email         TEXT    NOT NULL,
    verified      INTEGER NOT NULL,
    operator      INTEGER NOT NULL,
    signed_up_at  TEXT    NOT NULL,
    locks         TEXT    NOT NULL
);
