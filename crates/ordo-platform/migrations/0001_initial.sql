CREATE TABLE users (
    id TEXT PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    display_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    last_login TIMESTAMPTZ
);

CREATE TABLE organizations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    created_by TEXT NOT NULL
);

CREATE TABLE members (
    org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    email TEXT NOT NULL,
    display_name TEXT NOT NULL,
    role TEXT NOT NULL,
    invited_at TIMESTAMPTZ NOT NULL,
    invited_by TEXT NOT NULL,
    PRIMARY KEY (org_id, user_id)
);

CREATE INDEX idx_members_user ON members(user_id);

CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    created_by TEXT NOT NULL
);

CREATE INDEX idx_projects_org ON projects(org_id);

CREATE TABLE facts (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    data_type TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT '',
    latency_ms INTEGER,
    null_policy TEXT NOT NULL DEFAULT 'error',
    description TEXT,
    owner TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (project_id, name)
);

CREATE TABLE concepts (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    data_type TEXT NOT NULL,
    expression TEXT NOT NULL,
    dependencies JSONB NOT NULL DEFAULT '[]',
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (project_id, name)
);

CREATE TABLE contracts (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    ruleset_name TEXT NOT NULL,
    version_pattern TEXT NOT NULL DEFAULT '',
    owner TEXT NOT NULL DEFAULT '',
    sla_p99_ms INTEGER,
    input_fields JSONB NOT NULL DEFAULT '[]',
    output_fields JSONB NOT NULL DEFAULT '[]',
    notes TEXT,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (project_id, ruleset_name)
);

CREATE TABLE ruleset_history (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    ruleset_name TEXT NOT NULL,
    action TEXT NOT NULL,
    source TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    author_id TEXT NOT NULL,
    author_email TEXT NOT NULL,
    author_display_name TEXT NOT NULL,
    snapshot JSONB NOT NULL
);

CREATE INDEX idx_ruleset_history_lookup
    ON ruleset_history(project_id, ruleset_name, created_at DESC);

CREATE TABLE test_cases (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    ruleset_name TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    input JSONB NOT NULL,
    expect JSONB NOT NULL,
    tags JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    created_by TEXT NOT NULL
);

CREATE INDEX idx_test_cases_lookup ON test_cases(project_id, ruleset_name);
