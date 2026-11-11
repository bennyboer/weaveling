CREATE TABLE belongings (
    kind        TEXT NOT NULL,
    id          TEXT NOT NULL,
    owner_kind  TEXT NOT NULL,
    owner       TEXT NOT NULL,
    PRIMARY KEY (kind, id, owner_kind)
);
