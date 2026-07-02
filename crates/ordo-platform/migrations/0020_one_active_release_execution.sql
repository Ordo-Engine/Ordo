-- Prevent a double rollout of the same release request. `execute_release_request`
-- checks-then-inserts without a lock, so two concurrent calls could both create an
-- active execution and roll the same ruleset out twice. This makes "at most one
-- active execution per request" a database invariant.

-- First, resolve any pre-existing duplicates (from the race this index prevents)
-- so CREATE UNIQUE INDEX can't fail on existing data and block startup: keep the
-- most recent active execution per request, mark the older ones failed.
WITH ranked AS (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY release_request_id
               ORDER BY started_at DESC, id DESC
           ) AS rn
    FROM release_executions
    WHERE status IN (
        'preparing', 'waiting_start', 'rolling_out', 'paused', 'verifying',
        'rollback_in_progress'
    )
)
UPDATE release_executions
SET status = 'failed', finished_at = NOW()
WHERE id IN (SELECT id FROM ranked WHERE rn > 1);

-- Terminal executions (completed/failed/rolled_back/...) are not covered, so
-- sequential retries after a terminal execution still work.
CREATE UNIQUE INDEX IF NOT EXISTS release_executions_one_active_per_request
    ON release_executions (release_request_id)
    WHERE status IN (
        'preparing', 'waiting_start', 'rolling_out', 'paused', 'verifying',
        'rollback_in_progress'
    );
