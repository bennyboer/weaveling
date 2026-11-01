CREATE TABLE idea_summaries (
    idea       TEXT COLLATE "C" PRIMARY KEY,
    version     BIGINT           NOT NULL,
    project     TEXT COLLATE "C" NOT NULL,
    title       TEXT             NOT NULL
);

CREATE INDEX idea_summaries_by_project ON idea_summaries (project, idea DESC);

CREATE TABLE idea_tombstones (
    idea             TEXT COLLATE "C" PRIMARY KEY,
    laid_at          TIMESTAMPTZ      NOT NULL
);

CREATE INDEX idea_tombstones_by_laid_at ON idea_tombstones (laid_at);
