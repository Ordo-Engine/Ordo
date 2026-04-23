-- Draft ruleset storage (platform-side, not written to ordo-server until published)
CREATE TABLE project_rulesets (
    id                TEXT        PRIMARY KEY,
    project_id        TEXT        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name              TEXT        NOT NULL,
    draft             JSONB       NOT NULL,
    draft_seq         BIGINT      NOT NULL DEFAULT 1,  -- optimistic-lock sequence
    draft_updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    draft_updated_by  TEXT        REFERENCES users(id) ON DELETE SET NULL,
    published_version TEXT,                             -- config.version of last publish
    published_at      TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, name)
);

CREATE INDEX ON project_rulesets(project_id);

-- Deployment records (one row per publish action)
CREATE TABLE ruleset_deployments (
    id              TEXT        PRIMARY KEY,
    project_id      TEXT        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    environment_id  TEXT        NOT NULL REFERENCES project_environments(id) ON DELETE CASCADE,
    ruleset_name    TEXT        NOT NULL,
    version         TEXT        NOT NULL,
    release_note    TEXT,
    snapshot        JSONB       NOT NULL,  -- full RuleSet JSON at time of deploy
    deployed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deployed_by     TEXT        REFERENCES users(id) ON DELETE SET NULL,
    status          TEXT        NOT NULL DEFAULT 'queued'  -- queued | success | failed
);

CREATE INDEX ON ruleset_deployments(project_id, ruleset_name, deployed_at DESC);
CREATE INDEX ON ruleset_deployments(environment_id, deployed_at DESC);
