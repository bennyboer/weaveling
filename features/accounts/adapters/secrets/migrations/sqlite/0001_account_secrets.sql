CREATE TABLE account_credentials (
    email          TEXT PRIMARY KEY,
    account        TEXT NOT NULL UNIQUE,
    password_hash  TEXT NOT NULL
);

CREATE TABLE email_verifications (
    account  TEXT    PRIMARY KEY,
    code     TEXT    NOT NULL,
    until    TEXT    NOT NULL,
    tried    INTEGER NOT NULL
);
