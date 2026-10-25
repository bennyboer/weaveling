CREATE TABLE word_counts (
    scene       TEXT    PRIMARY KEY,
    project     TEXT    NOT NULL,
    words       INTEGER NOT NULL,
    counted_at  INTEGER NOT NULL
);

CREATE INDEX word_counts_by_project ON word_counts (project, scene);

CREATE TABLE gone_scenes (
    scene    TEXT    PRIMARY KEY,
    gone_at  INTEGER NOT NULL
);

CREATE INDEX gone_scenes_by_gone_at ON gone_scenes (gone_at);
