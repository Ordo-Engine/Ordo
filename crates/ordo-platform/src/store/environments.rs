use super::*;

impl PlatformStore {
    pub async fn list_environments(&self, project_id: &str) -> Result<Vec<ProjectEnvironment>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name,
                    COALESCE(ARRAY(
                        SELECT pes.server_id
                        FROM project_environment_servers pes
                        WHERE pes.environment_id = project_environments.id
                        ORDER BY pes.created_at, pes.server_id
                    ), ARRAY[]::TEXT[]) AS server_ids,
                    nats_subject_prefix, is_default,
                    canary_target_env_id, canary_percentage, created_at
             FROM project_environments WHERE project_id = $1 ORDER BY created_at",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_environment).collect())
    }

    pub async fn get_environment(
        &self,
        project_id: &str,
        env_id: &str,
    ) -> Result<Option<ProjectEnvironment>> {
        let row = sqlx::query(
            "SELECT id, project_id, name,
                    COALESCE(ARRAY(
                        SELECT pes.server_id
                        FROM project_environment_servers pes
                        WHERE pes.environment_id = project_environments.id
                        ORDER BY pes.created_at, pes.server_id
                    ), ARRAY[]::TEXT[]) AS server_ids,
                    nats_subject_prefix, is_default,
                    canary_target_env_id, canary_percentage, created_at
             FROM project_environments WHERE id = $1 AND project_id = $2",
        )
        .bind(env_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(row_to_environment))
    }

    pub async fn get_default_environment(
        &self,
        project_id: &str,
    ) -> Result<Option<ProjectEnvironment>> {
        let row = sqlx::query(
            "SELECT id, project_id, name,
                    COALESCE(ARRAY(
                        SELECT pes.server_id
                        FROM project_environment_servers pes
                        WHERE pes.environment_id = project_environments.id
                        ORDER BY pes.created_at, pes.server_id
                    ), ARRAY[]::TEXT[]) AS server_ids,
                    nats_subject_prefix, is_default,
                    canary_target_env_id, canary_percentage, created_at
             FROM project_environments WHERE project_id = $1 AND is_default = true LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(row_to_environment))
    }

    pub async fn create_environment(
        &self,
        id: &str,
        project_id: &str,
        req: &CreateEnvironmentRequest,
        is_default: bool,
    ) -> Result<ProjectEnvironment> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO project_environments
             (id, project_id, name, server_id, nats_subject_prefix, is_default,
              canary_target_env_id, canary_percentage, created_at)
             VALUES ($1, $2, $3, NULL, $4, $5, NULL, 0, NOW())",
        )
        .bind(id)
        .bind(project_id)
        .bind(&req.name)
        .bind(&req.nats_subject_prefix)
        .bind(is_default)
        .execute(&mut *tx)
        .await?;

        for server_id in &req.server_ids {
            sqlx::query(
                "INSERT INTO project_environment_servers (environment_id, server_id)
                 VALUES ($1, $2)
                 ON CONFLICT (environment_id, server_id) DO NOTHING",
            )
            .bind(id)
            .bind(server_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(self
            .get_environment(project_id, id)
            .await?
            .expect("just inserted"))
    }

    pub async fn update_environment(
        &self,
        project_id: &str,
        env_id: &str,
        req: &UpdateEnvironmentRequest,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE project_environments SET
               name = COALESCE($1, name),
               nats_subject_prefix = CASE WHEN $2::boolean THEN $3 ELSE nats_subject_prefix END
             WHERE id = $4 AND project_id = $5",
        )
        .bind(&req.name)
        .bind(req.nats_subject_prefix.is_some())
        .bind(&req.nats_subject_prefix)
        .bind(env_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        let updated = result.rows_affected() > 0;
        if updated {
            if let Some(server_ids) = &req.server_ids {
                sqlx::query("DELETE FROM project_environment_servers WHERE environment_id = $1")
                    .bind(env_id)
                    .execute(&mut *tx)
                    .await?;
                for server_id in server_ids {
                    sqlx::query(
                        "INSERT INTO project_environment_servers (environment_id, server_id)
                         VALUES ($1, $2)
                         ON CONFLICT (environment_id, server_id) DO NOTHING",
                    )
                    .bind(env_id)
                    .bind(server_id)
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }
        tx.commit().await?;
        Ok(updated)
    }

    pub async fn set_canary(
        &self,
        project_id: &str,
        env_id: &str,
        canary_target_env_id: Option<&str>,
        canary_percentage: i32,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE project_environments
             SET canary_target_env_id = $1, canary_percentage = $2
             WHERE id = $3 AND project_id = $4",
        )
        .bind(canary_target_env_id)
        .bind(canary_percentage)
        .bind(env_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_environment(&self, project_id: &str, env_id: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM project_environments WHERE id = $1 AND project_id = $2 AND is_default = false",
        )
        .bind(env_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
