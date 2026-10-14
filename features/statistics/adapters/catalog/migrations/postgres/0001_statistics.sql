CREATE TABLE word_counts (
    scene       TEXT COLLATE "C" PRIMARY KEY,
    project     TEXT COLLATE "C" NOT NULL,
    words       BIGINT           NOT NULL,
    counted_at  BIGINT           NOT NULL
);

CREATE INDEX word_counts_by_project ON word_counts (project, scene);

CREATE TABLE gone_scenes (
    scene  TEXT COLLATE "C" PRIMARY KEY
);
