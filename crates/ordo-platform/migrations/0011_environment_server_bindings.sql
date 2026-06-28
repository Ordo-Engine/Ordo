CREATE TABLE project_environment_servers (
    environment_id TEXT NOT NULL REFERENCES project_environments(id) ON DELETE CASCADE,
    server_id      TEXT NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (environment_id, server_id)
);

CREATE INDEX ON project_environment_servers(server_id);

INSERT INTO project_environment_servers (environment_id, server_id)
SELECT id, server_id
FROM project_environments
WHERE server_id IS NOT NULL
ON CONFLICT (environment_id, server_id) DO NOTHING;
