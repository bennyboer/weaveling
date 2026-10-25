CREATE TABLE appearances (
    subject_type  TEXT COLLATE "C" NOT NULL,
    subject_id    TEXT COLLATE "C" NOT NULL,
    place_type    TEXT COLLATE "C" NOT NULL,
    place_id      TEXT COLLATE "C" NOT NULL,
    version       BIGINT           NOT NULL,
    present       BOOLEAN          NOT NULL,

    PRIMARY KEY (subject_type, subject_id, place_type, place_id)
);

CREATE INDEX appearances_by_place ON appearances (place_type, place_id);

CREATE TABLE subject_tombstones (
    subject_type  TEXT COLLATE "C" NOT NULL,
    subject_id    TEXT COLLATE "C" NOT NULL,
    laid_at       TIMESTAMPTZ      NOT NULL,

    PRIMARY KEY (subject_type, subject_id)
);

CREATE INDEX gone_subjects_by_laid_at ON subject_tombstones (laid_at);

CREATE TABLE place_tombstones (
    place_type  TEXT COLLATE "C" NOT NULL,
    place_id    TEXT COLLATE "C" NOT NULL,
    laid_at     TIMESTAMPTZ      NOT NULL,

    PRIMARY KEY (place_type, place_id)
);

CREATE INDEX gone_places_by_laid_at ON place_tombstones (laid_at);

CREATE TABLE subject_projects (
    subject_type  TEXT COLLATE "C" NOT NULL,
    subject_id    TEXT COLLATE "C" NOT NULL,
    project       TEXT COLLATE "C" NOT NULL,

    PRIMARY KEY (subject_type, subject_id)
);
