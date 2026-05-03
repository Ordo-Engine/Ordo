ALTER TABLE release_executions
ADD COLUMN IF NOT EXISTS next_batch_at TIMESTAMPTZ;

ALTER TABLE release_execution_instances
ADD COLUMN IF NOT EXISTS batch_index INTEGER NOT NULL DEFAULT 1,
ADD COLUMN IF NOT EXISTS scheduled_at TIMESTAMPTZ;

ALTER TABLE release_execution_instances
DROP CONSTRAINT IF EXISTS release_execution_instances_status_check;

ALTER TABLE release_execution_instances
ADD CONSTRAINT release_execution_instances_status_check CHECK (
    status IN (
        'pending',
        'waiting_batch',
        'scheduled',
        'dispatching',
        'updating',
        'verifying',
        'success',
        'failed',
        'rolled_back',
        'skipped'
    )
);

CREATE INDEX IF NOT EXISTS release_execution_instances_execution_batch_idx
ON release_execution_instances(release_execution_id, batch_index);
