CREATE TABLE scene_catalog (
    scene          TEXT COLLATE "C" PRIMARY KEY,
    project        TEXT COLLATE "C" NOT NULL,
    title          TEXT             NOT NULL DEFAULT '',
    title_version  BIGINT           NOT NULL DEFAULT 0,
    opening        TEXT             NOT NULL DEFAULT '',
    opened_at      BIGINT           NOT NULL DEFAULT 0
);

CREATE INDEX scene_catalog_by_project ON scene_catalog (project, scene);

CREATE TABLE scene_tombstones (
    scene    TEXT COLLATE "C" PRIMARY KEY,
    laid_at  BIGINT           NOT NULL
);

CREATE INDEX scene_catalog_gone_by_laid_at ON scene_tombstones (laid_at);

CREATE TABLE quotation_restyling_tasks (
    project        TEXT COLLATE "C" PRIMARY KEY,
    latest_change  BIGINT           NOT NULL,
    total          BIGINT           NOT NULL DEFAULT 0,
    done           BIGINT           NOT NULL DEFAULT 0,
    finished       BOOLEAN          NOT NULL DEFAULT FALSE
);
