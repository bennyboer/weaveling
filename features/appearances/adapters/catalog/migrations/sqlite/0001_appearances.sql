CREATE TABLE appearances (
    subject_type  TEXT NOT NULL,
    subject_id    TEXT NOT NULL,
    place_type    TEXT NOT NULL,
    place_id      TEXT NOT NULL,
    version       INTEGER NOT NULL,
    present       INTEGER NOT NULL,

    PRIMARY KEY (subject_type, subject_id, place_type, place_id)
);

CREATE INDEX appearances_by_place ON appearances (place_type, place_id);

CREATE TABLE gone_subjects (
    subject_type  TEXT NOT NULL,
    subject_id    TEXT NOT NULL,
    gone_at       TEXT NOT NULL,

    PRIMARY KEY (subject_type, subject_id)
);

CREATE INDEX gone_subjects_by_gone_at ON gone_subjects (gone_at);

CREATE TABLE gone_places (
    place_type  TEXT NOT NULL,
    place_id    TEXT NOT NULL,
    gone_at     TEXT NOT NULL,

    PRIMARY KEY (place_type, place_id)
);

CREATE INDEX gone_places_by_gone_at ON gone_places (gone_at);

CREATE TABLE subject_projects (
    subject_type  TEXT NOT NULL,
    subject_id    TEXT NOT NULL,
    project       TEXT NOT NULL,

    PRIMARY KEY (subject_type, subject_id)
);
