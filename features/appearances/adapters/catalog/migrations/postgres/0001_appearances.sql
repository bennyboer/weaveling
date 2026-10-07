CREATE TABLE appearances (
    subject_type  TEXT COLLATE "C" NOT NULL,
    subject_id    TEXT COLLATE "C" NOT NULL,
    place_type    TEXT COLLATE "C" NOT NULL,
    place_id      TEXT COLLATE "C" NOT NULL,

    PRIMARY KEY (subject_type, subject_id, place_type, place_id)
);

CREATE INDEX appearances_by_place ON appearances (place_type, place_id);
