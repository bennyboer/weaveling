CREATE TABLE inbox (
    listener    TEXT COLLATE "C" NOT NULL,
    message_id  UUID             NOT NULL,
    heard_at    TIMESTAMPTZ      NOT NULL,
    PRIMARY KEY (listener, message_id)
);

CREATE INDEX inbox_heard ON inbox (heard_at);
