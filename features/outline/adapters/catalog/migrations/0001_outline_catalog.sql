CREATE TABLE outline_summaries (
    outline     TEXT COLLATE "C" PRIMARY KEY,
    project     TEXT COLLATE "C" NOT NULL
);

CREATE INDEX outline_summaries_by_project ON outline_summaries (project, outline);

CREATE TABLE outline_attachments (
    outline     TEXT COLLATE "C" NOT NULL,
    kind        TEXT COLLATE "C" NOT NULL,
    attached    TEXT COLLATE "C" NOT NULL,

    PRIMARY KEY (outline, kind, attached)
);

CREATE INDEX outline_attachments_by_attached ON outline_attachments (kind, attached, outline);
