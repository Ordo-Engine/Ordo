-- Add two-level organization hierarchy support.
-- depth 0 = root org, depth 1 = sub-org (max allowed).
-- parent_org_id is NULL for root orgs.

ALTER TABLE organizations
  ADD COLUMN IF NOT EXISTS parent_org_id TEXT REFERENCES organizations(id) ON DELETE CASCADE,
  ADD COLUMN IF NOT EXISTS depth         INT NOT NULL DEFAULT 0;
