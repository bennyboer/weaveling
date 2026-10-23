CREATE TABLE throttles (
    guarded  TEXT COLLATE "C" NOT NULL,
    about    TEXT COLLATE "C" NOT NULL,
    counted  INTEGER NOT NULL,
    last     TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (guarded, about)
);
