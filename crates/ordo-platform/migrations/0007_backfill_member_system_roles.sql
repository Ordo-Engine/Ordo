-- Seed built-in org-level system roles for every existing organization.
WITH system_roles(name, description, permissions) AS (
    VALUES
        (
            'owner',
            'Organization owner — full access',
            ARRAY[
                'org:view','org:manage',
                'member:view','member:invite','member:remove',
                'role:view','role:manage',
                'project:view','project:create','project:manage','project:delete',
                'ruleset:view','ruleset:edit','ruleset:publish',
                'environment:view','environment:manage',
                'server:view','server:manage',
                'test:run',
                'deployment:view','deployment:redeploy',
                'canary:manage'
            ]::TEXT[]
        ),
        (
            'admin',
            'Administrator — manages members and deployments',
            ARRAY[
                'org:view','org:manage',
                'member:view','member:invite','member:remove',
                'role:view','role:manage',
                'project:view','project:create','project:manage',
                'ruleset:view','ruleset:edit','ruleset:publish',
                'environment:view','environment:manage',
                'server:view','server:manage',
                'test:run',
                'deployment:view','deployment:redeploy',
                'canary:manage'
            ]::TEXT[]
        ),
        (
            'editor',
            'Editor — authors and tests rules',
            ARRAY[
                'org:view',
                'member:view',
                'role:view',
                'project:view',
                'ruleset:view','ruleset:edit',
                'environment:view',
                'server:view',
                'test:run',
                'deployment:view'
            ]::TEXT[]
        ),
        (
            'viewer',
            'Viewer — read-only access',
            ARRAY[
                'org:view',
                'member:view',
                'role:view',
                'project:view',
                'ruleset:view',
                'environment:view',
                'server:view',
                'deployment:view'
            ]::TEXT[]
        )
)
INSERT INTO org_roles (id, org_id, name, description, permissions, is_system, created_at)
SELECT
    gen_random_uuid()::TEXT,
    org.id,
    sr.name,
    sr.description,
    sr.permissions,
    TRUE,
    NOW()
FROM organizations org
CROSS JOIN system_roles sr
LEFT JOIN org_roles existing
    ON existing.org_id = org.id
   AND existing.name = sr.name
   AND existing.is_system = TRUE
WHERE existing.id IS NULL;

-- Migrate org-level member roles into the new role-assignment table.
INSERT INTO user_org_roles (user_id, org_id, role_id, assigned_at, assigned_by)
SELECT
    m.user_id,
    m.org_id,
    r.id,
    m.invited_at,
    COALESCE(m.invited_by, m.user_id)
FROM members m
JOIN org_roles r
    ON r.org_id = m.org_id
   AND r.name = LOWER(m.role)
   AND r.is_system = TRUE
ON CONFLICT (user_id, org_id, role_id) DO NOTHING;
