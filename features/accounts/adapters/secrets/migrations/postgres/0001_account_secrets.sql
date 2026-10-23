CREATE TABLE account_email_addresses (
    email    TEXT COLLATE "C" PRIMARY KEY,
    account  TEXT COLLATE "C" NOT NULL
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
    digest      TEXT COLLATE "C" PRIMARY KEY,
    invitation  TEXT COLLATE "C" NOT NULL UNIQUE,
    email       TEXT COLLATE "C" NOT NULL,
    until       TIMESTAMPTZ      NOT NULL
);
