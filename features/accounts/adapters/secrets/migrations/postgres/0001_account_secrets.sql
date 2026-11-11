CREATE TABLE account_email_addresses (
    email       TEXT COLLATE "C" PRIMARY KEY,
    account     TEXT COLLATE "C" NOT NULL,
    claimed_at  TIMESTAMPTZ      NOT NULL
);

CREATE TABLE account_passwords (
    account        TEXT COLLATE "C" PRIMARY KEY,
    password_hash  TEXT             NOT NULL
);

CREATE TABLE account_codes (
    account  TEXT COLLATE "C" NOT NULL,
    purpose  TEXT             NOT NULL,
    digest   TEXT             NOT NULL,
    until    TIMESTAMPTZ      NOT NULL,
    tried    INTEGER          NOT NULL,
    PRIMARY KEY (account, purpose)
);

CREATE INDEX account_codes_by_until ON account_codes (until);

CREATE TABLE account_sessions (
    digest   TEXT COLLATE "C" PRIMARY KEY,
    account  TEXT COLLATE "C" NOT NULL,
    stage    TEXT             NOT NULL,
    begun    TIMESTAMPTZ      NOT NULL,
    until    TIMESTAMPTZ      NOT NULL
);

CREATE INDEX account_sessions_by_until ON account_sessions (until);

CREATE TABLE account_operator_code (
    single   BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (single),
    digest   TEXT COLLATE "C" NOT NULL
);

CREATE TABLE account_operators (
    account  TEXT COLLATE "C" PRIMARY KEY
);

CREATE TABLE account_invitation_tickets (
    invitation  TEXT COLLATE "C" PRIMARY KEY,
    digest      TEXT COLLATE "C" UNIQUE,
    email       TEXT COLLATE "C",
    until       TIMESTAMPTZ,
    project     TEXT COLLATE "C",
    by_an_operator BOOLEAN       NOT NULL DEFAULT FALSE,
    ended       BOOLEAN          NOT NULL DEFAULT FALSE
);

CREATE TABLE account_totps (
    account              TEXT COLLATE "C" PRIMARY KEY,
    secret_in_use        BYTEA,
    last_step            BIGINT,
    secret_being_set_up  BYTEA
);

CREATE TABLE account_recovery_codes (
    account    TEXT COLLATE "C" NOT NULL,
    code_hash  TEXT COLLATE "C" NOT NULL,
    PRIMARY KEY (account, code_hash)
);

CREATE TABLE account_key_credentials (
    account        TEXT COLLATE "C" NOT NULL,
    key            TEXT COLLATE "C" NOT NULL,
    credential_id  BYTEA NOT NULL UNIQUE,
    written        TEXT  NOT NULL,
    PRIMARY KEY (account, key)
);

CREATE TABLE account_second_steps (
    account           TEXT COLLATE "C" PRIMARY KEY,
    answered_last_by  TEXT COLLATE "C" NOT NULL
);
