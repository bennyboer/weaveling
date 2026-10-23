CREATE TABLE account_email_addresses (
    email    TEXT PRIMARY KEY,
    account  TEXT NOT NULL
);

CREATE TABLE account_passwords (
    account        TEXT PRIMARY KEY,
    password_hash  TEXT NOT NULL
);

CREATE TABLE account_codes (
    account  TEXT    NOT NULL,
    purpose  TEXT    NOT NULL,
    code     TEXT    NOT NULL,
    until    TEXT    NOT NULL,
    tried    INTEGER NOT NULL,
    PRIMARY KEY (account, purpose)
);

CREATE TABLE account_sessions (
    digest   TEXT PRIMARY KEY,
    account  TEXT NOT NULL,
    stage    TEXT NOT NULL,
    begun    TEXT NOT NULL,
    until    TEXT NOT NULL
);

CREATE TABLE account_operator_code (
    single   BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (single),
    digest   TEXT NOT NULL
);

CREATE TABLE account_operators (
    account  TEXT PRIMARY KEY
);

CREATE TABLE account_invitation_tickets (
    digest      TEXT PRIMARY KEY,
    invitation  TEXT NOT NULL UNIQUE,
    email       TEXT NOT NULL,
    until       TEXT NOT NULL
);
