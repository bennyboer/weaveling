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

CREATE TABLE account_throttles (
    guarded  TEXT             NOT NULL,
    email    TEXT COLLATE "C" NOT NULL,
    counted  INTEGER          NOT NULL,
    last     TIMESTAMPTZ      NOT NULL,
    PRIMARY KEY (guarded, email)
);
