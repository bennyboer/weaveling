CREATE TABLE scene_catalog (
    scene          TEXT    PRIMARY KEY,
    project        TEXT    NOT NULL,
    title          TEXT    NOT NULL DEFAULT '',
    title_version  INTEGER NOT NULL DEFAULT 0,
    opening        TEXT    NOT NULL DEFAULT '',
    opened_at      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX scene_catalog_by_project ON scene_catalog (project, scene);

CREATE TABLE scene_catalog_gone (
    scene    TEXT    PRIMARY KEY,
    gone_at  INTEGER NOT NULL
);

CREATE INDEX scene_catalog_gone_by_gone_at ON scene_catalog_gone (gone_at);

CREATE TABLE quotation_restyling_tasks (
    project        TEXT    PRIMARY KEY,
    latest_change  INTEGER NOT NULL,
    total          INTEGER NOT NULL DEFAULT 0,
    done           INTEGER NOT NULL DEFAULT 0,
    finished       INTEGER NOT NULL DEFAULT 0
);
