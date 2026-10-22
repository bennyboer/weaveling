CREATE TABLE account_email_addresses (
    email    TEXT PRIMARY KEY,
    account  TEXT NOT NULL
);

CREATE TABLE account_passwords (
    account        TEXT PRIMARY KEY,
    password_hash  TEXT NOT NULL
);

CREATE TABLE email_verifications (
    account  TEXT    PRIMARY KEY,
    code     TEXT    NOT NULL,
    until    TEXT    NOT NULL,
    tried    INTEGER NOT NULL
);

CREATE TABLE account_sessions (
    digest   TEXT PRIMARY KEY,
    account  TEXT NOT NULL,
    stage    TEXT NOT NULL,
    until    TEXT NOT NULL
);
