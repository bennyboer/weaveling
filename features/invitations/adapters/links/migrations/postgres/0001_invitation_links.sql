CREATE TABLE invitation_links (
    digest      TEXT COLLATE "C" PRIMARY KEY,
    invitation  TEXT COLLATE "C" NOT NULL
);

CREATE INDEX invitation_links_by_invitation ON invitation_links (invitation);
