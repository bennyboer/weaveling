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
    code     TEXT             NOT NULL,
    until    TIMESTAMPTZ      NOT NULL,
    tried    INTEGER          NOT NULL,
    PRIMARY KEY (account, purpose)
);

CREATE TABLE account_sessions (
    digest   TEXT COLLATE "C" PRIMARY KEY,
    account  TEXT COLLATE "C" NOT NULL,
    stage    TEXT             NOT NULL,
    begun    TIMESTAMPTZ      NOT NULL,
    until    TIMESTAMPTZ      NOT NULL
);

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

CREATE TABLE account_trashed_projects (
    project       TEXT COLLATE "C" PRIMARY KEY,
    version       BIGINT  NOT NULL,
    in_the_trash  BOOLEAN NOT NULL
);

CREATE TABLE account_second_factors (
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
