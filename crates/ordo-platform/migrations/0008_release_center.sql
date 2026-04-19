CREATE TABLE release_policies (
    id               TEXT        PRIMARY KEY,
    org_id           TEXT        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id       TEXT        REFERENCES projects(id) ON DELETE CASCADE,
    name             TEXT        NOT NULL,
    scope            TEXT        NOT NULL CHECK (scope IN ('org', 'project')),
    target_type      TEXT        NOT NULL CHECK (target_type IN ('project', 'environment')),
    target_id        TEXT        NOT NULL,
    description      TEXT,
    min_approvals    INTEGER     NOT NULL DEFAULT 1 CHECK (min_approvals >= 1),
    allow_self_approval BOOLEAN  NOT NULL DEFAULT false,
    approver_ids     TEXT[]      NOT NULL DEFAULT '{}',
    rollout_strategy JSONB       NOT NULL DEFAULT '{}'::jsonb,
    rollback_policy  JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX ON release_policies(org_id);
CREATE INDEX ON release_policies(project_id);
CREATE UNIQUE INDEX release_policies_target_unique
ON release_policies(org_id, COALESCE(project_id, ''), target_type, target_id, name);

CREATE TABLE release_requests (
    id                     TEXT        PRIMARY KEY,
    org_id                 TEXT        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id             TEXT        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    ruleset_name           TEXT        NOT NULL,
    version                TEXT        NOT NULL,
    environment_id         TEXT        NOT NULL REFERENCES project_environments(id) ON DELETE CASCADE,
    policy_id              TEXT        REFERENCES release_policies(id) ON DELETE SET NULL,
    status                 TEXT        NOT NULL CHECK (
        status IN (
            'draft', 'pending_approval', 'approved', 'rejected', 'cancelled',
            'executing', 'completed', 'failed', 'rolled_back'
        )
    ),
    title                  TEXT        NOT NULL,
    change_summary         TEXT        NOT NULL,
    release_note           TEXT,
    affected_instance_count INTEGER    NOT NULL DEFAULT 0,
    rollback_version       TEXT,
    created_by             TEXT        NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX ON release_requests(org_id, project_id, created_at DESC);
CREATE INDEX ON release_requests(project_id, environment_id, status);

CREATE TABLE release_approvals (
    id                 TEXT        PRIMARY KEY,
    release_request_id TEXT        NOT NULL REFERENCES release_requests(id) ON DELETE CASCADE,
    stage              INTEGER     NOT NULL CHECK (stage >= 1),
    reviewer_id        TEXT        NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    decision           TEXT        NOT NULL CHECK (decision IN ('pending', 'approved', 'rejected')),
    comment            TEXT,
    decided_at         TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (release_request_id, stage, reviewer_id)
);

CREATE INDEX ON release_approvals(release_request_id, stage);

CREATE TABLE release_executions (
    id                 TEXT        PRIMARY KEY,
    release_request_id TEXT        NOT NULL REFERENCES release_requests(id) ON DELETE CASCADE,
    status             TEXT        NOT NULL CHECK (
        status IN ('preparing', 'waiting_start', 'rolling_out', 'paused', 'verifying',
                   'rollback_in_progress', 'completed', 'failed')
    ),
    current_batch      INTEGER     NOT NULL DEFAULT 0,
    total_batches      INTEGER     NOT NULL DEFAULT 0,
    strategy_snapshot  JSONB       NOT NULL DEFAULT '{}'::jsonb,
    started_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at        TIMESTAMPTZ,
    triggered_by       TEXT        REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX ON release_executions(release_request_id);

CREATE TABLE release_execution_instances (
    id                   TEXT        PRIMARY KEY,
    release_execution_id TEXT        NOT NULL REFERENCES release_executions(id) ON DELETE CASCADE,
    instance_id          TEXT        NOT NULL,
    instance_name        TEXT        NOT NULL,
    zone                 TEXT,
    current_version      TEXT        NOT NULL,
    target_version       TEXT        NOT NULL,
    status               TEXT        NOT NULL CHECK (
        status IN ('pending', 'dispatching', 'updating', 'verifying', 'success', 'failed', 'rolled_back', 'skipped')
    ),
    message              TEXT,
    metric_summary       JSONB       NOT NULL DEFAULT '{}'::jsonb,
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX ON release_execution_instances(release_execution_id, status);

CREATE TABLE release_execution_events (
    id                   TEXT        PRIMARY KEY,
    release_execution_id TEXT        NOT NULL REFERENCES release_executions(id) ON DELETE CASCADE,
    instance_id          TEXT,
    event_type           TEXT        NOT NULL,
    payload              JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX ON release_execution_events(release_execution_id, created_at DESC);
