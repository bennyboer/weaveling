CREATE TABLE scenes (
    scene     TEXT    PRIMARY KEY,
    project     TEXT    NOT NULL,
    title       TEXT    NOT NULL,
    version     INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX scenes_by_project ON scenes (project, scene);

CREATE TABLE scene_updates (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    scene     TEXT    NOT NULL REFERENCES scenes (scene) ON DELETE CASCADE,
    is_snapshot INTEGER NOT NULL,
    bytes       BLOB    NOT NULL,
    written_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX scene_updates_reading ON scene_updates (scene, seq);
CREATE INDEX scene_updates_snapshots ON scene_updates (scene, seq DESC) WHERE is_snapshot;

CREATE TABLE scene_ideas (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    scene     TEXT    NOT NULL REFERENCES scenes (scene) ON DELETE CASCADE,
    idea        TEXT    NOT NULL,

    UNIQUE (scene, idea)
);

CREATE INDEX scene_ideas_by_idea ON scene_ideas (idea);
