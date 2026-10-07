CREATE TABLE board_summaries (
    board       TEXT PRIMARY KEY,
    project     TEXT NOT NULL
);

CREATE INDEX board_summaries_by_project ON board_summaries (project, board);

CREATE TABLE board_ideas (
    board       TEXT NOT NULL,
    idea        TEXT NOT NULL,

    PRIMARY KEY (board, idea)
);

CREATE INDEX board_ideas_by_idea ON board_ideas (idea, board);
