CREATE TABLE outline_summaries (
    outline     TEXT COLLATE "C" PRIMARY KEY,
    project     TEXT COLLATE "C" NOT NULL
);

CREATE INDEX outline_summaries_by_project ON outline_summaries (project, outline);

CREATE TABLE outline_pieces (
    outline     TEXT COLLATE "C" NOT NULL,
    piece       TEXT COLLATE "C" NOT NULL,

    PRIMARY KEY (outline, piece)
);

CREATE INDEX outline_pieces_by_piece ON outline_pieces (piece, outline);
