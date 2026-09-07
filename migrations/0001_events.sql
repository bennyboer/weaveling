create table events (
    aggregate       text        not null,
    kind            text        not null,
    version         bigint      not null,
    name            text        not null,
    body            jsonb       not null,
    body_version    bigint      not null,
    agent           text        not null,
    occurred_at     timestamptz not null,
    is_snapshot     boolean     not null,
    written_at      timestamptz not null default now(),

    primary key (aggregate, kind, version)
);

create index events_snapshots on events (aggregate, kind, version desc) where is_snapshot;

create table outbox (
    entry           bigserial   primary key,
    aggregate       text        not null,
    kind            text        not null,
    version         bigint      not null,
    routing_key     text        not null,
    payload         jsonb       not null,
    occurred_at     timestamptz not null,
    written_at      timestamptz not null default now(),
    claimed_until   timestamptz,
    published_at    timestamptz,

    foreign key (aggregate, kind, version) references events (aggregate, kind, version) on delete cascade
);

create index outbox_waiting on outbox (entry) where published_at is null;
