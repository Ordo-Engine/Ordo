CREATE TABLE servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    token TEXT NOT NULL UNIQUE,
    org_id TEXT REFERENCES organizations(id) ON DELETE SET NULL,
    labels JSONB NOT NULL DEFAULT '{}',
    version TEXT,
    status TEXT NOT NULL DEFAULT 'offline',
    last_seen TIMESTAMPTZ,
    registered_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_servers_org ON servers(org_id);
CREATE INDEX idx_servers_token ON servers(token);
CREATE INDEX idx_servers_status ON servers(status);

ALTER TABLE projects ADD COLUMN server_id TEXT REFERENCES servers(id) ON DELETE SET NULL;
