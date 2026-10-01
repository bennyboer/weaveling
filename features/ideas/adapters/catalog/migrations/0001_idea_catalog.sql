CREATE TABLE idea_summaries (
    idea       TEXT COLLATE "C" PRIMARY KEY,
    version     BIGINT           NOT NULL,
    project     TEXT COLLATE "C" NOT NULL,
    title       TEXT             NOT NULL
);

CREATE INDEX idea_summaries_by_project ON idea_summaries (project, idea DESC);
