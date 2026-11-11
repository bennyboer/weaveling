CREATE TABLE account_email_addresses (
    email       TEXT PRIMARY KEY,
    account     TEXT NOT NULL,
    claimed_at  TEXT NOT NULL
);

CREATE TABLE account_passwords (
    account        TEXT PRIMARY KEY,
    password_hash  TEXT NOT NULL
);

CREATE TABLE account_codes (
    account  TEXT    NOT NULL,
    purpose  TEXT    NOT NULL,
    digest   TEXT    NOT NULL,
    until    TEXT    NOT NULL,
    tried    INTEGER NOT NULL,
    PRIMARY KEY (account, purpose)
);

CREATE INDEX account_codes_by_until ON account_codes (until);

CREATE TABLE account_sessions (
    digest   TEXT PRIMARY KEY,
    account  TEXT NOT NULL,
    stage    TEXT NOT NULL,
    begun    TEXT NOT NULL,
    until    TEXT NOT NULL
);

CREATE INDEX account_sessions_by_until ON account_sessions (until);

CREATE TABLE account_operator_code (
    single   BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (single),
    digest   TEXT NOT NULL
);

CREATE TABLE account_operators (
    account  TEXT PRIMARY KEY
);

CREATE TABLE account_invitation_tickets (
    invitation  TEXT    PRIMARY KEY,
    digest      TEXT    UNIQUE,
    email       TEXT,
    until       TEXT,
    project     TEXT,
    by_an_operator BOOLEAN NOT NULL DEFAULT FALSE,
    ended       BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE account_totps (
    account              TEXT PRIMARY KEY,
    secret_in_use        BLOB,
    last_step            INTEGER,
    secret_being_set_up  BLOB
);

CREATE TABLE account_recovery_codes (
    account    TEXT NOT NULL,
    code_hash  TEXT NOT NULL,
    PRIMARY KEY (account, code_hash)
);

CREATE TABLE account_key_credentials (
    account        TEXT NOT NULL,
    key            TEXT NOT NULL,
    credential_id  BLOB NOT NULL UNIQUE,
    written        TEXT NOT NULL,
    PRIMARY KEY (account, key)
);

CREATE TABLE account_second_steps (
    account           TEXT PRIMARY KEY,
    answered_last_by  TEXT NOT NULL
);
