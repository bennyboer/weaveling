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

CREATE TABLE attachment_tombstones (
    attachment_type TEXT NOT NULL,
    attachment_id   TEXT NOT NULL,
    laid_at         TEXT NOT NULL,

    PRIMARY KEY (attachment_type, attachment_id)
);

CREATE INDEX gone_attachments_by_laid_at ON attachment_tombstones (laid_at);

CREATE TABLE outline_tombstones (
    outline          TEXT PRIMARY KEY,
    laid_at          TEXT NOT NULL
);

CREATE INDEX outline_tombstones_by_laid_at ON outline_tombstones (laid_at);

CREATE TABLE project_tombstones (
    project          TEXT PRIMARY KEY,
    laid_at          TEXT NOT NULL
);

CREATE INDEX project_tombstones_by_laid_at ON project_tombstones (laid_at);
