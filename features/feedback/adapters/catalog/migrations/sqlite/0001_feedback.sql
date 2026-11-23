CREATE TABLE feedback (
    feedback TEXT PRIMARY KEY,
    project TEXT NOT NULL,
    subject_kind TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    place_kind TEXT,
    place_id TEXT,
    about TEXT NOT NULL,
    comment TEXT NOT NULL,
    written_by TEXT NOT NULL,
    written_at INTEGER NOT NULL,
    submitted_at INTEGER,
    edited INTEGER NOT NULL,
    done INTEGER NOT NULL
);

-- Listed a page at a time in the order they were submitted, by project, by subject or by place.
CREATE INDEX feedback_by_project ON feedback (project, submitted_at, written_at, feedback);
CREATE INDEX feedback_by_subject ON feedback (subject_kind, subject_id, submitted_at, written_at, feedback);
CREATE INDEX feedback_by_place ON feedback (place_kind, place_id, submitted_at, written_at, feedback);
CREATE INDEX feedback_by_writer ON feedback (written_by, written_at, feedback);

-- Each giver's first piece submitted in a project, found without reading their others.
CREATE INDEX feedback_firsts_by_giver ON feedback (project, written_by, submitted_at, written_at, feedback)
    WHERE submitted_at IS NOT NULL;

-- What is pending, counted without reading what is done.
CREATE INDEX feedback_pending_by_place ON feedback (project, place_kind, place_id)
    WHERE submitted_at IS NOT NULL AND NOT done;
CREATE INDEX feedback_pending_by_subject ON feedback (subject_kind, subject_id)
    WHERE submitted_at IS NOT NULL AND NOT done;

-- Whoever gave feedback in a project, with their first piece submitted, so they are listed without reading all of it.
CREATE TABLE givers (
    project TEXT NOT NULL,
    giver TEXT NOT NULL,
    submitted_at INTEGER NOT NULL,
    written_at INTEGER NOT NULL,
    feedback TEXT NOT NULL,
    PRIMARY KEY (project, giver)
);

CREATE INDEX givers_in_order ON givers (project, submitted_at, written_at, feedback);

CREATE TABLE reading_copies_heard (
    reading_copy TEXT PRIMARY KEY,
    project TEXT NOT NULL,
    open_until INTEGER,
    version INTEGER NOT NULL,
    drafts_dropped INTEGER NOT NULL DEFAULT 0
);

-- The copies closing, whose drafts are still to be dropped once they are closed.
CREATE INDEX reading_copies_closing ON reading_copies_heard (open_until)
    WHERE open_until IS NOT NULL AND NOT drafts_dropped;

CREATE TABLE reading_copy_tombstones (
    reading_copy TEXT PRIMARY KEY,
    laid_at INTEGER NOT NULL
);

CREATE INDEX gone_reading_copies_by_laid_at ON reading_copy_tombstones (laid_at);

CREATE TABLE account_names (
    account TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE account_name_tombstones (
    account TEXT PRIMARY KEY,
    laid_at INTEGER NOT NULL
);

CREATE INDEX gone_account_names_by_laid_at ON account_name_tombstones (laid_at);
