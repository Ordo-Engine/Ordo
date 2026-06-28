CREATE TABLE sub_rule_assets (
    id               TEXT        PRIMARY KEY,
    org_id           TEXT        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id       TEXT        REFERENCES projects(id) ON DELETE CASCADE,
    scope            TEXT        NOT NULL CHECK (scope IN ('org', 'project')),
    name             TEXT        NOT NULL,
    display_name     TEXT,
    description      TEXT,
    draft            JSONB       NOT NULL,
    input_schema     JSONB       NOT NULL DEFAULT '[]',
    output_schema    JSONB       NOT NULL DEFAULT '[]',
    draft_seq        BIGINT      NOT NULL DEFAULT 1,
    draft_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    draft_updated_by TEXT        REFERENCES users(id) ON DELETE SET NULL,
    published_version TEXT,
    published_at     TIMESTAMPTZ,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by       TEXT        REFERENCES users(id) ON DELETE SET NULL,
    CHECK (
        (scope = 'org' AND project_id IS NULL)
        OR (scope = 'project' AND project_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX sub_rule_assets_org_unique
    ON sub_rule_assets(org_id, name)
    WHERE scope = 'org';

CREATE UNIQUE INDEX sub_rule_assets_project_unique
    ON sub_rule_assets(project_id, name)
    WHERE scope = 'project';

CREATE INDEX sub_rule_assets_org_lookup
    ON sub_rule_assets(org_id, scope, name);

CREATE INDEX sub_rule_assets_project_lookup
    ON sub_rule_assets(project_id, name);

CREATE TABLE sub_rule_versions (
    id            TEXT        PRIMARY KEY,
    asset_id      TEXT        NOT NULL REFERENCES sub_rule_assets(id) ON DELETE CASCADE,
    version       TEXT        NOT NULL,
    snapshot      JSONB       NOT NULL,
    input_schema  JSONB       NOT NULL DEFAULT '[]',
    output_schema JSONB       NOT NULL DEFAULT '[]',
    release_note  TEXT,
    published_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    published_by  TEXT        REFERENCES users(id) ON DELETE SET NULL,
    UNIQUE (asset_id, version)
);

CREATE INDEX sub_rule_versions_asset_lookup
    ON sub_rule_versions(asset_id, published_at DESC);
