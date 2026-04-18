-- Project environments (replaces the single projects.server_id binding)
CREATE TABLE project_environments (
    id                   TEXT        PRIMARY KEY,
    project_id           TEXT        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name                 TEXT        NOT NULL,
    server_id            TEXT        REFERENCES servers(id) ON DELETE SET NULL,
    nats_subject_prefix  TEXT,                           -- NULL = platform global prefix
    is_default           BOOLEAN     NOT NULL DEFAULT false,
    -- Canary routing (non-default envs only)
    canary_target_env_id TEXT        REFERENCES project_environments(id) ON DELETE SET NULL,
    canary_percentage    INT         NOT NULL DEFAULT 0 CHECK (canary_percentage BETWEEN 0 AND 100),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, name)
);

CREATE INDEX ON project_environments(project_id);

-- Migrate existing projects.server_id → a default "production" environment.
-- Runs at application startup (see main.rs) to handle rows that exist before this migration.
