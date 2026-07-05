-- Time-series of cumulative rule-execution counters reported by engines.
-- Each row is a snapshot (totals since engine start) for one (server, ruleset)
-- at captured_at. The analytics API diffs consecutive snapshots to derive rates.
CREATE TABLE IF NOT EXISTS execution_metric_snapshots (
    id                   BIGSERIAL PRIMARY KEY,
    org_id               TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    server_id            TEXT NOT NULL,
    ruleset              TEXT NOT NULL,
    captured_at          TIMESTAMPTZ NOT NULL,
    exec_success         DOUBLE PRECISION NOT NULL DEFAULT 0,
    exec_error           DOUBLE PRECISION NOT NULL DEFAULT 0,
    -- decision code -> cumulative count
    terminal_counts      JSONB NOT NULL DEFAULT '{}',
    duration_count       DOUBLE PRECISION NOT NULL DEFAULT 0,
    duration_sum_seconds DOUBLE PRECISION NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_exec_snap_org_ruleset_time
    ON execution_metric_snapshots (org_id, ruleset, captured_at DESC);
CREATE INDEX IF NOT EXISTS idx_exec_snap_server_ruleset_time
    ON execution_metric_snapshots (server_id, ruleset, captured_at);
