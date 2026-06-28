-- Custom roles per org (is_system=true = built-in, cannot be deleted)
CREATE TABLE org_roles (
    id          TEXT        PRIMARY KEY,
    org_id      TEXT        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    description TEXT,
    permissions TEXT[]      NOT NULL DEFAULT '{}',
    is_system   BOOLEAN     NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (org_id, name)
);

CREATE INDEX ON org_roles(org_id);

-- User ↔ role assignments (replaces members.role for permission checks)
CREATE TABLE user_org_roles (
    user_id     TEXT        NOT NULL,
    org_id      TEXT        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    role_id     TEXT        NOT NULL REFERENCES org_roles(id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    assigned_by TEXT        REFERENCES users(id) ON DELETE SET NULL,
    PRIMARY KEY (user_id, org_id, role_id)
);

CREATE INDEX ON user_org_roles(org_id, user_id);

-- members.role kept for backward-compat display; permission checks use user_org_roles.
-- Built-in roles are seeded at application startup (see main.rs).
