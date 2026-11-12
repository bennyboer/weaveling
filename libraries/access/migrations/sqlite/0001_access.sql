CREATE TABLE access_roles (
    project  TEXT    NOT NULL,
    account  TEXT    NOT NULL,
    version  INTEGER NOT NULL,
    role     TEXT   ,
    gone_at  TEXT   ,
    PRIMARY KEY (project, account)
);

CREATE INDEX access_roles_by_account ON access_roles (account, project);
CREATE INDEX access_roles_by_gone_at ON access_roles (gone_at);

CREATE TABLE access_disbanded (
    project  TEXT    PRIMARY KEY,
    version  INTEGER NOT NULL,
    laid_at  TEXT    NOT NULL
);

CREATE INDEX access_disbanded_by_laid_at ON access_disbanded (laid_at);

CREATE TABLE access_trash (
    project       TEXT    PRIMARY KEY,
    version       INTEGER NOT NULL,
    in_the_trash  INTEGER NOT NULL
);

CREATE TABLE access_operators (
    account   TEXT    PRIMARY KEY,
    version   INTEGER NOT NULL,
    operator  INTEGER NOT NULL,
    gone_at   TEXT   
);

CREATE INDEX access_operators_by_gone_at ON access_operators (gone_at);
