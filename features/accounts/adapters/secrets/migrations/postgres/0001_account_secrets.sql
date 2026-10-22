CREATE TABLE account_credentials (
    email          TEXT COLLATE "C" PRIMARY KEY,
    account        TEXT COLLATE "C" NOT NULL UNIQUE,
    password_hash  TEXT             NOT NULL
);

CREATE TABLE email_verifications (
    account  TEXT COLLATE "C" PRIMARY KEY,
    code     TEXT             NOT NULL,
    until    TIMESTAMPTZ      NOT NULL,
    failed   INTEGER          NOT NULL
);
