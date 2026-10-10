CREATE TABLE deliveries (
    delivery        BIGSERIAL   PRIMARY KEY,
    listener        TEXT COLLATE "C" NOT NULL,
    message_id      UUID        NOT NULL,
    conversation    UUID        NOT NULL,
    caused_by       UUID,
    routing_key     TEXT        NOT NULL,
    payload         JSONB       NOT NULL,
    occurred_at     TIMESTAMPTZ NOT NULL,
    attempts        INTEGER     NOT NULL DEFAULT 0,
    due_at          TIMESTAMPTZ NOT NULL,
    claimed_until   TIMESTAMPTZ,
    last_refusal    TEXT
);

CREATE INDEX deliveries_due ON deliveries (due_at, delivery);

CREATE TABLE dead_letters (
    dead_letter     BIGSERIAL   PRIMARY KEY,
    listener        TEXT COLLATE "C" NOT NULL,
    message_id      UUID        NOT NULL,
    conversation    UUID        NOT NULL,
    caused_by       UUID,
    routing_key     TEXT        NOT NULL,
    payload         JSONB       NOT NULL,
    occurred_at     TIMESTAMPTZ NOT NULL,
    attempts        INTEGER     NOT NULL,
    why             TEXT        NOT NULL,
    given_up_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged_at TIMESTAMPTZ
);
