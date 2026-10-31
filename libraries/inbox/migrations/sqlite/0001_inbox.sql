CREATE TABLE inbox (
    listener    TEXT NOT NULL,
    message_id  TEXT NOT NULL,
    heard_at    TEXT NOT NULL,
    PRIMARY KEY (listener, message_id)
);

CREATE INDEX inbox_heard ON inbox (heard_at);
