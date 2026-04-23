use super::*;

impl PlatformStore {
    pub async fn seed_system_roles(&self, org_id: &str) -> Result<()> {
        let existing = self.list_org_roles(org_id).await?;
        let find_existing = |name: &str| existing.iter().find(|r| r.name == name && r.is_system);

        let system_roles: &[(&str, &str, &[&str])] = &[
            (
                "owner",
                "Organization owner — full access",
                &[
                    "org:view",
                    "org:manage",
                    "member:view",
                    "member:invite",
                    "member:remove",
                    "role:view",
                    "role:manage",
                    "project:view",
                    "project:create",
                    "project:manage",
                    "project:delete",
                    "ruleset:view",
                    "ruleset:edit",
                    "ruleset:publish",
                    "environment:view",
                    "environment:manage",
                    "server:view",
                    "server:manage",
                    "test:run",
                    "deployment:view",
                    "deployment:redeploy",
                    "canary:manage",
                    "release:policy.manage",
                    "release:request.create",
                    "release:request.view",
                    "release:request.approve",
                    "release:request.reject",
                    "release:execute",
                    "release:pause",
                    "release:resume",
                    "release:rollback",
                    "release:instance.view",
                ],
            ),
            (
                "admin",
                "Administrator — manages members and deployments",
                &[
                    "org:view",
                    "org:manage",
                    "member:view",
                    "member:invite",
                    "member:remove",
                    "role:view",
                    "role:manage",
                    "project:view",
                    "project:create",
                    "project:manage",
                    "ruleset:view",
                    "ruleset:edit",
                    "ruleset:publish",
                    "environment:view",
                    "environment:manage",
                    "server:view",
                    "server:manage",
                    "test:run",
                    "deployment:view",
                    "deployment:redeploy",
                    "canary:manage",
                    "release:policy.manage",
                    "release:request.create",
                    "release:request.view",
                    "release:request.approve",
                    "release:request.reject",
                    "release:execute",
                    "release:pause",
                    "release:resume",
                    "release:rollback",
                    "release:instance.view",
                ],
            ),
            (
                "editor",
                "Editor — edits and tests rulesets",
                &[
                    "org:view",
                    "member:view",
                    "role:view",
                    "project:view",
                    "ruleset:view",
                    "ruleset:edit",
                    "environment:view",
                    "server:view",
                    "test:run",
                    "deployment:view",
                    "release:request.create",
                    "release:request.view",
                ],
            ),
            (
                "viewer",
                "Viewer — read-only access",
                &[
                    "org:view",
                    "member:view",
                    "role:view",
                    "project:view",
                    "ruleset:view",
                    "environment:view",
                    "server:view",
                    "deployment:view",
                    "release:request.view",
                ],
            ),
        ];

        for (name, desc, perms) in system_roles {
            let desired_perms: Vec<String> = perms.iter().map(|s| s.to_string()).collect();
            if let Some(role) = find_existing(name) {
                let needs_update =
                    role.description.as_deref() != Some(*desc) || role.permissions != desired_perms;
                if needs_update {
                    sqlx::query(
                        "UPDATE org_roles
                         SET description = $1, permissions = $2
                         WHERE id = $3 AND org_id = $4 AND is_system = true",
                    )
                    .bind(*desc)
                    .bind(&desired_perms)
                    .bind(&role.id)
                    .bind(org_id)
                    .execute(&self.pool)
                    .await?;
                }
                continue;
            }
            let id = uuid::Uuid::new_v4().to_string();
            self.create_org_role(
                &id,
                org_id,
                &CreateRoleRequest {
                    name: name.to_string(),
                    description: Some(desc.to_string()),
                    permissions: desired_perms,
                },
                true,
            )
            .await?;
        }
        Ok(())
    }

    pub async fn list_all_orgs(&self) -> Result<Vec<Organization>> {
        let rows = sqlx::query(
            "SELECT id, name, description, created_at, created_by, parent_org_id, depth
             FROM organizations ORDER BY depth, created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut orgs = Vec::new();
        for row in rows {
            let org_id: String = row.get("id");
            let members = self.fetch_members(&org_id).await?;
            orgs.push(Organization {
                id: org_id,
                name: row.get("name"),
                description: row.get("description"),
                created_at: row.get("created_at"),
                created_by: row.get("created_by"),
                parent_org_id: row.get("parent_org_id"),
                depth: row.get("depth"),
                members,
            });
        }
        Ok(orgs)
    }

    pub async fn list_all_projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query("SELECT * FROM projects ORDER BY created_at")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(row_to_project).collect())
    }

    pub async fn migrate_project_server_to_environment(
        &self,
        project_id: &str,
        server_id: Option<&str>,
    ) -> Result<()> {
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM project_environments WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;

        if existing > 0 {
            return Ok(());
        }

        let env_id = uuid::Uuid::new_v4().to_string();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO project_environments
               (id, project_id, name, server_id, nats_subject_prefix, is_default, canary_percentage)
               VALUES ($1, $2, 'production', NULL, NULL, true, 0)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(&env_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;

        if let Some(server_id) = server_id {
            sqlx::query(
                "INSERT INTO project_environment_servers (environment_id, server_id)
                 VALUES ($1, $2)
                 ON CONFLICT (environment_id, server_id) DO NOTHING",
            )
            .bind(&env_id)
            .bind(server_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        Ok(())
    }
}
