CREATE TABLE belongings (
    kind        TEXT COLLATE "C" NOT NULL,
    id          TEXT COLLATE "C" NOT NULL,
    owner_kind  TEXT COLLATE "C" NOT NULL,
    owner       TEXT COLLATE "C" NOT NULL,
    PRIMARY KEY (kind, id, owner_kind)
);
