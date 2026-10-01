CREATE TABLE outline_summaries (
    outline     TEXT COLLATE "C" PRIMARY KEY,
    project     TEXT COLLATE "C" NOT NULL
);

CREATE INDEX outline_summaries_by_project ON outline_summaries (project, outline);

CREATE TABLE outline_passages (
    outline     TEXT COLLATE "C" NOT NULL,
    passage       TEXT COLLATE "C" NOT NULL,

    PRIMARY KEY (outline, passage)
);

CREATE INDEX outline_passages_by_passage ON outline_passages (passage, outline);
