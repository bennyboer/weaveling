CREATE TABLE scene_texts (
    scene    TEXT COLLATE "C" PRIMARY KEY,
    version  BIGINT           NOT NULL,
    blocks   TEXT             NOT NULL
);

CREATE TABLE scene_text_tombstones (
    scene    TEXT COLLATE "C" PRIMARY KEY,
    laid_at  BIGINT           NOT NULL
);

CREATE INDEX gone_scene_texts_by_laid_at ON scene_text_tombstones (laid_at);

CREATE TABLE books (
    project          TEXT COLLATE "C" PRIMARY KEY,
    title            TEXT,
    title_version    BIGINT,
    sections         TEXT,
    outline_version  BIGINT
);

CREATE TABLE book_tombstones (
    project  TEXT COLLATE "C" PRIMARY KEY,
    laid_at  BIGINT           NOT NULL
);

CREATE INDEX gone_books_by_laid_at ON book_tombstones (laid_at);

CREATE TABLE reading_copies (
    copy          TEXT COLLATE "C" PRIMARY KEY,
    project       TEXT COLLATE "C" NOT NULL,
    name          TEXT             NOT NULL,
    made_by_kind  TEXT             NOT NULL,
    made_by       TEXT COLLATE "C",
    made_at       TIMESTAMPTZ      NOT NULL,
    extent        TEXT             NOT NULL,
    extent_count  BIGINT,
    open_until    TIMESTAMPTZ,
    deleted_on    TIMESTAMPTZ
);

CREATE INDEX reading_copies_by_project_newest ON reading_copies (project, made_at DESC, copy DESC);
CREATE INDEX reading_copies_by_deletion ON reading_copies (deleted_on, copy);

CREATE TABLE account_names (
    account  TEXT COLLATE "C" PRIMARY KEY,
    name     TEXT             NOT NULL
);

CREATE TABLE account_name_tombstones (
    account  TEXT COLLATE "C" PRIMARY KEY,
    laid_at  BIGINT           NOT NULL
);

CREATE INDEX gone_account_names_by_laid_at ON account_name_tombstones (laid_at);

CREATE TABLE reading_copy_contents (
    copy      TEXT COLLATE "C" PRIMARY KEY,
    project   TEXT COLLATE "C" NOT NULL,
    contents  TEXT             NOT NULL
);

CREATE INDEX reading_copy_contents_by_project ON reading_copy_contents (project);
