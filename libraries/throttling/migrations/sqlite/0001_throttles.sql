CREATE TABLE throttles (
    guarded  TEXT NOT NULL,
    about    TEXT NOT NULL,
    counted  INTEGER NOT NULL,
    last     TEXT NOT NULL,
    PRIMARY KEY (guarded, about)
);
