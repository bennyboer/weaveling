CREATE TABLE project_member_lists (
    project          TEXT    PRIMARY KEY,
    version          INTEGER NOT NULL
);

CREATE TABLE memberships (
    project          TEXT    NOT NULL,
    account          TEXT    NOT NULL,
    role             TEXT    NOT NULL,
    PRIMARY KEY (project, account)
);

CREATE INDEX memberships_by_account ON memberships (account, project);
