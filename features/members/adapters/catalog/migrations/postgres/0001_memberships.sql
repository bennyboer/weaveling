CREATE TABLE project_member_lists (
    project          TEXT COLLATE "C" PRIMARY KEY,
    version          BIGINT           NOT NULL
);

CREATE TABLE memberships (
    project          TEXT COLLATE "C" NOT NULL,
    account          TEXT COLLATE "C" NOT NULL,
    role             TEXT             NOT NULL,
    PRIMARY KEY (project, account)
);

CREATE INDEX memberships_by_account ON memberships (account, project);


CREATE TABLE people (
    account          TEXT COLLATE "C" PRIMARY KEY,
    name             TEXT             NOT NULL,
    email            TEXT             NOT NULL
);

CREATE TABLE project_trash (
    project          TEXT COLLATE "C" PRIMARY KEY,
    version          BIGINT           NOT NULL,
    in_the_trash     BOOLEAN          NOT NULL
);

CREATE TABLE leaving_accounts (
    account          TEXT COLLATE "C" PRIMARY KEY,
    version          BIGINT           NOT NULL,
    until            TIMESTAMPTZ
);
