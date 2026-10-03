CREATE TABLE passages (
    passage     TEXT COLLATE "C" PRIMARY KEY,
    project     TEXT COLLATE "C" NOT NULL,
    title       TEXT             NOT NULL,
    created_at  TIMESTAMPTZ      NOT NULL DEFAULT NOW()
);

CREATE INDEX passages_by_project ON passages (project, passage);

CREATE TABLE passage_updates (
    seq         BIGSERIAL   PRIMARY KEY,
    passage     TEXT        NOT NULL REFERENCES passages (passage) ON DELETE CASCADE,
    is_snapshot BOOLEAN     NOT NULL,
    bytes       BYTEA       NOT NULL,
    written_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX passage_updates_reading ON passage_updates (passage, seq);
CREATE INDEX passage_updates_snapshots ON passage_updates (passage, seq DESC) WHERE is_snapshot;
