CREATE TABLE outline_summaries (
    outline     TEXT COLLATE "C" PRIMARY KEY,
    project     TEXT COLLATE "C" NOT NULL
);

CREATE INDEX outline_summaries_by_project ON outline_summaries (project, outline);

CREATE TABLE outline_attachments (
    outline         TEXT COLLATE "C" NOT NULL,
    attachment_type TEXT COLLATE "C" NOT NULL,
    attachment_id   TEXT COLLATE "C" NOT NULL,

    PRIMARY KEY (outline, attachment_type, attachment_id)
);

CREATE INDEX outline_attachments_by_attachment ON outline_attachments (attachment_type, attachment_id, outline);
