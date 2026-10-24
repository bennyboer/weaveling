CREATE TABLE invitation_summaries (
    invitation       TEXT    PRIMARY KEY,
    version          INTEGER NOT NULL,
    email            TEXT    NOT NULL,
    issued_at        TEXT    NOT NULL,
    until            TEXT    NOT NULL,
    outcome          TEXT    NOT NULL,
    declined_for     TEXT,
    retire_after     TEXT    NOT NULL
);
