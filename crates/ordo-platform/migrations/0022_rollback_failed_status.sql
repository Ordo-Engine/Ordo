-- The status enums in code (`ReleaseExecutionStatus`, `ReleaseRequestStatus`) both
-- include 'rollback_failed', but the CHECK constraints from 0008 never did. A failed
-- rollback therefore could not persist its terminal status: the UPDATE violated the
-- constraint, the error was swallowed, the execution stayed 'rollback_in_progress',
-- and the worker re-claimed and re-ran the failing rollback every poll, forever.
-- Rebuild both CHECKs with the full status set the code writes.

ALTER TABLE release_executions
DROP CONSTRAINT IF EXISTS release_executions_status_check;

ALTER TABLE release_executions
ADD CONSTRAINT release_executions_status_check CHECK (
    status IN (
        'preparing',
        'waiting_start',
        'rolling_out',
        'paused',
        'verifying',
        'rollback_in_progress',
        'rollback_failed',
        'completed',
        'failed'
    )
);

ALTER TABLE release_requests
DROP CONSTRAINT IF EXISTS release_requests_status_check;

ALTER TABLE release_requests
ADD CONSTRAINT release_requests_status_check CHECK (
    status IN (
        'draft',
        'pending_approval',
        'approved',
        'rejected',
        'cancelled',
        'executing',
        'completed',
        'failed',
        'rollback_failed',
        'rolled_back'
    )
);
