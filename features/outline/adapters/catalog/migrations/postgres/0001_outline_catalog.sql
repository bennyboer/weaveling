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

CREATE TABLE attachment_tombstones (
    attachment_type TEXT COLLATE "C" NOT NULL,
    attachment_id   TEXT COLLATE "C" NOT NULL,
    laid_at         TIMESTAMPTZ      NOT NULL,

    PRIMARY KEY (attachment_type, attachment_id)
);

CREATE INDEX gone_attachments_by_laid_at ON attachment_tombstones (laid_at);

CREATE TABLE outline_tombstones (
    outline          TEXT COLLATE "C" PRIMARY KEY,
    laid_at          TIMESTAMPTZ      NOT NULL
);

CREATE INDEX outline_tombstones_by_laid_at ON outline_tombstones (laid_at);

CREATE TABLE project_tombstones (
    project          TEXT COLLATE "C" PRIMARY KEY,
    laid_at          TIMESTAMPTZ      NOT NULL
);

CREATE INDEX project_tombstones_by_laid_at ON project_tombstones (laid_at);
