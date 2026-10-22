CREATE TABLE account_email_addresses (
    email    TEXT COLLATE "C" PRIMARY KEY,
    account  TEXT COLLATE "C" NOT NULL
);

CREATE TABLE account_passwords (
    account        TEXT COLLATE "C" PRIMARY KEY,
    password_hash  TEXT             NOT NULL
);

CREATE TABLE email_verifications (
    account  TEXT COLLATE "C" PRIMARY KEY,
    code     TEXT             NOT NULL,
    until    TIMESTAMPTZ      NOT NULL,
    tried    INTEGER          NOT NULL
);

CREATE TABLE account_sessions (
    digest   TEXT COLLATE "C" PRIMARY KEY,
    account  TEXT COLLATE "C" NOT NULL,
    stage    TEXT             NOT NULL,
    until    TIMESTAMPTZ      NOT NULL
);
