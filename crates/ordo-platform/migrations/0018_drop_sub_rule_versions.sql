-- Sub-rules no longer have a separate publish/version system.
-- They are snapshotted inline when the parent ruleset is published.
DROP TABLE IF EXISTS sub_rule_versions;

ALTER TABLE sub_rule_assets
    DROP COLUMN IF EXISTS published_version,
    DROP COLUMN IF EXISTS published_at;
