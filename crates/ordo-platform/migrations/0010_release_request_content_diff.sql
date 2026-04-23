ALTER TABLE release_requests
ADD COLUMN IF NOT EXISTS content_diff JSONB NOT NULL DEFAULT '{}'::jsonb;
