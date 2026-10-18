CREATE TABLE scene_catalog (
    scene          TEXT COLLATE "C" PRIMARY KEY,
    project        TEXT COLLATE "C" NOT NULL,
    title          TEXT             NOT NULL DEFAULT '',
    title_version  BIGINT           NOT NULL DEFAULT 0,
    opening        TEXT             NOT NULL DEFAULT '',
    opened_at      BIGINT           NOT NULL DEFAULT 0
);

CREATE INDEX scene_catalog_by_project ON scene_catalog (project, scene);

CREATE TABLE scene_catalog_gone (
    scene  TEXT COLLATE "C" PRIMARY KEY
);

CREATE TABLE quotation_restyling_tasks (
    project        TEXT COLLATE "C" PRIMARY KEY,
    latest_change  BIGINT           NOT NULL
);
