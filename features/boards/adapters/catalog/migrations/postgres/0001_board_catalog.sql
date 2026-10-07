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
