CREATE TABLE access_roles (
    project  TEXT COLLATE "C" NOT NULL,
    account  TEXT COLLATE "C" NOT NULL,
    version  BIGINT           NOT NULL,
    role     TEXT COLLATE "C",
    gone_at  TIMESTAMPTZ     ,
    PRIMARY KEY (project, account)
);

CREATE INDEX access_roles_by_account ON access_roles (account, project);
CREATE INDEX access_roles_by_gone_at ON access_roles (gone_at);

CREATE TABLE access_disbanded (
    project  TEXT COLLATE "C" PRIMARY KEY,
    version  BIGINT           NOT NULL,
    laid_at  TIMESTAMPTZ      NOT NULL
);

CREATE INDEX access_disbanded_by_laid_at ON access_disbanded (laid_at);

CREATE TABLE access_trash (
    project       TEXT COLLATE "C" PRIMARY KEY,
    version       BIGINT           NOT NULL,
    in_the_trash  BOOLEAN          NOT NULL
);

CREATE TABLE access_operators (
    account   TEXT COLLATE "C" PRIMARY KEY,
    version   BIGINT           NOT NULL,
    operator  BOOLEAN          NOT NULL,
    gone_at   TIMESTAMPTZ     
);

CREATE INDEX access_operators_by_gone_at ON access_operators (gone_at);
