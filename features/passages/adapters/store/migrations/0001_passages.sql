CREATE TABLE passages (
    passage     TEXT        PRIMARY KEY,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE passage_updates (
    seq         BIGSERIAL   PRIMARY KEY,
    passage     TEXT        NOT NULL REFERENCES passages (passage) ON DELETE CASCADE,
    is_snapshot BOOLEAN     NOT NULL,
    bytes       BYTEA       NOT NULL,
    written_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX passage_updates_reading ON passage_updates (passage, seq);
CREATE INDEX passage_updates_snapshots ON passage_updates (passage, seq DESC) WHERE is_snapshot;
