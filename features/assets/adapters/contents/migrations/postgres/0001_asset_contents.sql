CREATE TABLE asset_contents (
    asset  TEXT COLLATE "C" NOT NULL,
    piece  TEXT COLLATE "C" NOT NULL,
    bytes  BYTEA            NOT NULL,
    PRIMARY KEY (asset, piece)
);

ALTER TABLE asset_contents ALTER COLUMN bytes SET STORAGE EXTERNAL;
