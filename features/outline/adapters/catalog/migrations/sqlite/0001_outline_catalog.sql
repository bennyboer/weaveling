CREATE TABLE outline_summaries (
    outline     TEXT PRIMARY KEY,
    project     TEXT NOT NULL
);

CREATE INDEX outline_summaries_by_project ON outline_summaries (project, outline);

CREATE TABLE outline_attachments (
    outline         TEXT NOT NULL,
    attachment_type TEXT NOT NULL,
    attachment_id   TEXT NOT NULL,

    PRIMARY KEY (outline, attachment_type, attachment_id)
);

CREATE INDEX outline_attachments_by_attachment ON outline_attachments (attachment_type, attachment_id, outline);
