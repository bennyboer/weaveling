CREATE TABLE board_summaries (
    board       TEXT COLLATE "C" PRIMARY KEY,
    project     TEXT COLLATE "C" NOT NULL
);

CREATE INDEX board_summaries_by_project ON board_summaries (project, board);

CREATE TABLE board_pieces (
    board       TEXT COLLATE "C" NOT NULL,
    piece       TEXT COLLATE "C" NOT NULL,

    PRIMARY KEY (board, piece)
);

CREATE INDEX board_pieces_by_piece ON board_pieces (piece, board);
