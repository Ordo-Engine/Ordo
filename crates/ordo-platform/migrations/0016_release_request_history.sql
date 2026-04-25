CREATE TABLE release_request_history (
    id                   TEXT        PRIMARY KEY,
    release_request_id   TEXT        NOT NULL REFERENCES release_requests(id) ON DELETE CASCADE,
    release_execution_id TEXT        REFERENCES release_executions(id) ON DELETE CASCADE,
    instance_id          TEXT,
    scope                TEXT        NOT NULL CHECK (
        scope IN ('request', 'approval', 'execution', 'batch', 'instance', 'rollback')
    ),
    action               TEXT        NOT NULL,
    actor_type           TEXT        NOT NULL CHECK (
        actor_type IN ('user', 'system', 'server')
    ),
    actor_id             TEXT,
    actor_name           TEXT,
    actor_email          TEXT,
    from_status          TEXT,
    to_status            TEXT,
    detail               JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX release_request_history_request_created_idx
ON release_request_history(release_request_id, created_at DESC);

CREATE INDEX release_request_history_execution_created_idx
ON release_request_history(release_execution_id, created_at DESC);
