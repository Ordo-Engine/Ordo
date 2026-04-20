CREATE TABLE notifications (
    id TEXT PRIMARY KEY,
    org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    type TEXT NOT NULL,
    ref_id TEXT,
    ref_type TEXT,
    payload JSONB NOT NULL DEFAULT '{}',
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX notifications_user_unread ON notifications (user_id, read_at) WHERE read_at IS NULL;
CREATE INDEX notifications_org_user ON notifications (org_id, user_id, created_at DESC);
