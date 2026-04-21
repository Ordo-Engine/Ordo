use super::*;

impl PlatformStore {
    pub async fn save_project(&self, project: &Project) -> Result<()> {
        sqlx::query(
            "INSERT INTO projects (id, name, description, org_id, created_at, created_by, server_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name,
               description = EXCLUDED.description,
               server_id = EXCLUDED.server_id",
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.description)
        .bind(&project.org_id)
        .bind(project.created_at)
        .bind(&project.created_by)
        .bind(&project.server_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_project(&self, org_id: &str, project_id: &str) -> Result<Option<Project>> {
        let row = sqlx::query(
            "SELECT id, name, description, org_id, created_at, created_by, server_id
             FROM projects WHERE id = $1 AND org_id = $2",
        )
        .bind(project_id)
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| row_to_project(&r)))
    }

    pub async fn list_projects(&self, org_id: &str) -> Result<Vec<Project>> {
        let rows = sqlx::query(
            "SELECT id, name, description, org_id, created_at, created_by, server_id
             FROM projects WHERE org_id = $1 ORDER BY created_at",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_project).collect())
    }

    pub async fn delete_project(&self, org_id: &str, project_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM projects WHERE id = $1 AND org_id = $2")
            .bind(project_id)
            .bind(org_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn bind_project_server(
        &self,
        org_id: &str,
        project_id: &str,
        server_id: Option<&str>,
    ) -> Result<bool> {
        let result =
            sqlx::query("UPDATE projects SET server_id = $1 WHERE id = $2 AND org_id = $3")
                .bind(server_id)
                .bind(project_id)
                .bind(org_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}
