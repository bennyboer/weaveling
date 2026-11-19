CREATE TABLE scene_texts (
    scene    TEXT    PRIMARY KEY,
    version  INTEGER NOT NULL,
    blocks   TEXT    NOT NULL
);

CREATE TABLE scene_text_tombstones (
    scene    TEXT    PRIMARY KEY,
    laid_at  INTEGER NOT NULL
);

CREATE INDEX gone_scene_texts_by_laid_at ON scene_text_tombstones (laid_at);

CREATE TABLE books (
    project          TEXT    PRIMARY KEY,
    title            TEXT,
    title_version    INTEGER,
    sections         TEXT,
    outline_version  INTEGER
);

CREATE TABLE book_tombstones (
    project  TEXT    PRIMARY KEY,
    laid_at  INTEGER NOT NULL
);

CREATE INDEX gone_books_by_laid_at ON book_tombstones (laid_at);

CREATE TABLE reading_copies (
    copy          TEXT    PRIMARY KEY,
    project       TEXT    NOT NULL,
    name          TEXT    NOT NULL,
    made_by_kind  TEXT    NOT NULL,
    made_by       TEXT,
    made_at       TEXT    NOT NULL,
    extent        TEXT    NOT NULL,
    extent_count  INTEGER,
    open_until    TEXT,
    deleted_on    TEXT
);

CREATE INDEX reading_copies_by_project_newest ON reading_copies (project, made_at DESC, copy DESC);

CREATE TABLE account_names (
    account  TEXT    PRIMARY KEY,
    name     TEXT    NOT NULL
);

CREATE TABLE account_name_tombstones (
    account  TEXT    PRIMARY KEY,
    laid_at  INTEGER NOT NULL
);

CREATE INDEX gone_account_names_by_laid_at ON account_name_tombstones (laid_at);
