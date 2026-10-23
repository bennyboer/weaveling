CREATE TABLE invitation_links (
    digest      TEXT PRIMARY KEY,
    invitation  TEXT NOT NULL
);

CREATE INDEX invitation_links_by_invitation ON invitation_links (invitation);
