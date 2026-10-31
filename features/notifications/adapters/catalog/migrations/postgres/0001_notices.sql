CREATE TABLE notices (
    notice           TEXT COLLATE "C" PRIMARY KEY,
    version          BIGINT           NOT NULL,
    account          TEXT COLLATE "C" NOT NULL,
    project          TEXT COLLATE "C",
    about            JSONB            NOT NULL,
    noted_at         TIMESTAMPTZ      NOT NULL,
    dismissed_at     TIMESTAMPTZ
);

CREATE INDEX notices_open_by_account ON notices (account, noted_at DESC, notice DESC)
    WHERE dismissed_at IS NULL;


CREATE TABLE people (
    account          TEXT COLLATE "C" PRIMARY KEY,
    name             TEXT             NOT NULL
);

CREATE TABLE project_names (
    project          TEXT COLLATE "C" PRIMARY KEY,
    version          BIGINT           NOT NULL,
    name             TEXT             NOT NULL
);

CREATE TABLE project_tombstones (
    project          TEXT COLLATE "C" PRIMARY KEY,
    laid_at          TIMESTAMPTZ      NOT NULL
);

CREATE INDEX project_tombstones_by_laid_at ON project_tombstones (laid_at);
