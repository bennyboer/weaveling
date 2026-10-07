CREATE TABLE passages (
    passage     TEXT    PRIMARY KEY,
    project     TEXT    NOT NULL,
    title       TEXT    NOT NULL,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX passages_by_project ON passages (project, passage);

CREATE TABLE passage_updates (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    passage     TEXT    NOT NULL REFERENCES passages (passage) ON DELETE CASCADE,
    is_snapshot INTEGER NOT NULL,
    bytes       BLOB    NOT NULL,
    written_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX passage_updates_reading ON passage_updates (passage, seq);
CREATE INDEX passage_updates_snapshots ON passage_updates (passage, seq DESC) WHERE is_snapshot;

CREATE TABLE passage_ideas (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    passage     TEXT    NOT NULL REFERENCES passages (passage) ON DELETE CASCADE,
    idea        TEXT    NOT NULL,

    UNIQUE (passage, idea)
);

CREATE INDEX passage_ideas_by_idea ON passage_ideas (idea);
