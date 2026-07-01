-- Per-organization engine connect tokens. An engine registers with one of
-- these (header `x-connect-token`); the platform derives the owning org from it,
-- so servers are scoped by organization instead of landing in a global pool.
CREATE TABLE org_connect_tokens (
    id           TEXT        PRIMARY KEY,
    org_id       TEXT        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    token        TEXT        NOT NULL UNIQUE,
    label        TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by   TEXT,
    last_used_at TIMESTAMPTZ
);

CREATE INDEX idx_org_connect_tokens_org ON org_connect_tokens(org_id);
