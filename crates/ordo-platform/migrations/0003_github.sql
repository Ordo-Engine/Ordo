-- GitHub OAuth connections (one per user)
-- access_token should be encrypted at rest in production via a KMS/Vault.
-- For the MVP, stored as plaintext; rotate via re-connect at any time.
CREATE TABLE github_connections (
    user_id         TEXT        PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    github_user_id  BIGINT      NOT NULL,
    login           TEXT        NOT NULL,
    name            TEXT,
    avatar_url      TEXT,
    access_token    TEXT        NOT NULL,
    scope           TEXT        NOT NULL DEFAULT '',
    connected_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Unique index so one GitHub account can only be linked to one Ordo user
CREATE UNIQUE INDEX idx_github_connections_github_user_id
    ON github_connections(github_user_id);
