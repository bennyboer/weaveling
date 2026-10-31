CREATE TABLE notices (
    notice           TEXT    PRIMARY KEY,
    version          INTEGER NOT NULL,
    account          TEXT    NOT NULL,
    project          TEXT,
    about            TEXT    NOT NULL,
    noted_at         TEXT    NOT NULL,
    dismissed_at     TEXT
);

CREATE INDEX notices_open_by_account ON notices (account, noted_at DESC, notice DESC)
    WHERE dismissed_at IS NULL;


CREATE TABLE people (
    account          TEXT    PRIMARY KEY,
    name             TEXT    NOT NULL
);

CREATE TABLE project_names (
    project          TEXT    PRIMARY KEY,
    version          INTEGER NOT NULL,
    name             TEXT    NOT NULL
);

CREATE TABLE project_tombstones (
    project          TEXT    PRIMARY KEY,
    laid_at          TEXT    NOT NULL
);

CREATE INDEX project_tombstones_by_laid_at ON project_tombstones (laid_at);
