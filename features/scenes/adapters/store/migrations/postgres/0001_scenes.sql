CREATE TABLE scenes (
    scene     TEXT COLLATE "C" PRIMARY KEY,
    project     TEXT COLLATE "C" NOT NULL,
    title       TEXT             NOT NULL,
    version     BIGINT           NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ      NOT NULL DEFAULT NOW()
);

CREATE INDEX scenes_by_project ON scenes (project, scene);

CREATE TABLE scene_updates (
    seq         BIGSERIAL   PRIMARY KEY,
    scene     TEXT        NOT NULL REFERENCES scenes (scene) ON DELETE CASCADE,
    is_snapshot BOOLEAN     NOT NULL,
    bytes       BYTEA       NOT NULL,
    written_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX scene_updates_reading ON scene_updates (scene, seq);
CREATE INDEX scene_updates_snapshots ON scene_updates (scene, seq DESC) WHERE is_snapshot;

CREATE TABLE scene_ideas (
    seq         BIGSERIAL        PRIMARY KEY,
    scene     TEXT COLLATE "C" NOT NULL REFERENCES scenes (scene) ON DELETE CASCADE,
    idea        TEXT COLLATE "C" NOT NULL,

    UNIQUE (scene, idea)
);

CREATE INDEX scene_ideas_by_idea ON scene_ideas (idea);
