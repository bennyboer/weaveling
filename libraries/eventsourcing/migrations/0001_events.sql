CREATE TABLE events (
    aggregate       TEXT        NOT NULL,
    kind            TEXT        NOT NULL,
    version         BIGINT      NOT NULL,
    name            TEXT        NOT NULL,
    body            JSONB       NOT NULL,
    body_version    BIGINT      NOT NULL,
    agent           TEXT        NOT NULL,
    occurred_at     TIMESTAMPTZ NOT NULL,
    is_snapshot     BOOLEAN     NOT NULL,
    written_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (aggregate, kind, version)
);

CREATE INDEX events_snapshots ON events (aggregate, kind, version DESC) WHERE is_snapshot;

CREATE TABLE outbox (
    entry           BIGSERIAL   PRIMARY KEY,
    aggregate       TEXT        NOT NULL,
    kind            TEXT        NOT NULL,
    version         BIGINT      NOT NULL,
    routing_key     TEXT        NOT NULL,
    payload         JSONB       NOT NULL,
    occurred_at     TIMESTAMPTZ NOT NULL,
    written_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    claimed_until   TIMESTAMPTZ,
    published_at    TIMESTAMPTZ
);

CREATE INDEX outbox_waiting ON outbox (entry) WHERE published_at IS NULL;
