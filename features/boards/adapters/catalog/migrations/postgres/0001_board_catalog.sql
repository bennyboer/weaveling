CREATE TABLE board_summaries (
    board       TEXT COLLATE "C" PRIMARY KEY,
    project     TEXT COLLATE "C" NOT NULL
);

CREATE INDEX board_summaries_by_project ON board_summaries (project, board);

CREATE TABLE board_ideas (
    board       TEXT COLLATE "C" NOT NULL,
    idea       TEXT COLLATE "C" NOT NULL,

    PRIMARY KEY (board, idea)
);

CREATE INDEX board_ideas_by_idea ON board_ideas (idea, board);

CREATE TABLE idea_tombstones (
    idea        TEXT COLLATE "C" PRIMARY KEY,
    laid_at     TIMESTAMPTZ      NOT NULL
);

CREATE INDEX gone_ideas_by_laid_at ON idea_tombstones (laid_at);

CREATE TABLE board_tombstones (
    board            TEXT COLLATE "C" PRIMARY KEY,
    laid_at          TIMESTAMPTZ      NOT NULL
);

CREATE INDEX board_tombstones_by_laid_at ON board_tombstones (laid_at);

CREATE TABLE project_tombstones (
    project          TEXT COLLATE "C" PRIMARY KEY,
    laid_at          TIMESTAMPTZ      NOT NULL
);

CREATE INDEX project_tombstones_by_laid_at ON project_tombstones (laid_at);
