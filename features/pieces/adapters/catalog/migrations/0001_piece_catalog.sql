CREATE TABLE piece_summaries (
    piece       TEXT COLLATE "C" PRIMARY KEY,
    version     BIGINT           NOT NULL,
    project     TEXT COLLATE "C" NOT NULL,
    title       TEXT             NOT NULL,
    passage     TEXT COLLATE "C"
);

CREATE INDEX piece_summaries_by_project ON piece_summaries (project, piece DESC);
