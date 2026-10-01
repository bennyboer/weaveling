CREATE TABLE outline_summaries (
    outline     TEXT COLLATE "C" PRIMARY KEY,
    project     TEXT COLLATE "C" NOT NULL
);

CREATE INDEX outline_summaries_by_project ON outline_summaries (project, outline);

CREATE TABLE outline_ideas (
    outline     TEXT COLLATE "C" NOT NULL,
    idea       TEXT COLLATE "C" NOT NULL,

    PRIMARY KEY (outline, idea)
);

CREATE INDEX outline_ideas_by_idea ON outline_ideas (idea, outline);
