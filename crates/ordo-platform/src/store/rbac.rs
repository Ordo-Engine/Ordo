use super::*;

impl PlatformStore {
    pub async fn list_org_roles(&self, org_id: &str) -> Result<Vec<OrgRole>> {
        let rows = sqlx::query(
            "SELECT id, org_id, name, description, permissions, is_system, created_at
             FROM org_roles WHERE org_id = $1 ORDER BY is_system DESC, name",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_org_role).collect())
    }

    pub async fn get_org_role(&self, org_id: &str, role_id: &str) -> Result<Option<OrgRole>> {
        let row = sqlx::query(
            "SELECT id, org_id, name, description, permissions, is_system, created_at
             FROM org_roles WHERE id = $1 AND org_id = $2",
        )
        .bind(role_id)
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(row_to_org_role))
    }

    pub async fn create_org_role(
        &self,
        id: &str,
        org_id: &str,
        req: &CreateRoleRequest,
        is_system: bool,
    ) -> Result<OrgRole> {
        sqlx::query(
            "INSERT INTO org_roles (id, org_id, name, description, permissions, is_system, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, NOW())",
        )
        .bind(id)
        .bind(org_id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.permissions)
        .bind(is_system)
        .execute(&self.pool)
        .await?;
        Ok(self.get_org_role(org_id, id).await?.expect("just inserted"))
    }

    pub async fn update_org_role(
        &self,
        org_id: &str,
        role_id: &str,
        req: &UpdateRoleRequest,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE org_roles SET
               name = COALESCE($1, name),
               description = CASE WHEN $2::boolean THEN $3 ELSE description END,
               permissions = COALESCE($4, permissions)
             WHERE id = $5 AND org_id = $6 AND is_system = false",
        )
        .bind(&req.name)
        .bind(req.description.is_some())
        .bind(&req.description)
        .bind(req.permissions.as_deref())
        .bind(role_id)
        .bind(org_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_org_role(&self, org_id: &str, role_id: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM org_roles WHERE id = $1 AND org_id = $2 AND is_system = false",
        )
        .bind(role_id)
        .bind(org_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_user_roles(
        &self,
        org_id: &str,
        user_id: &str,
    ) -> Result<Vec<UserRoleAssignment>> {
        let rows = sqlx::query(
            "SELECT uur.user_id, uur.org_id, uur.role_id, r.name AS role_name,
                    uur.assigned_at, uur.assigned_by
             FROM user_org_roles uur
             JOIN org_roles r ON r.id = uur.role_id
             WHERE uur.org_id = $1 AND uur.user_id = $2
             ORDER BY uur.assigned_at",
        )
        .bind(org_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_user_role).collect())
    }

    pub async fn assign_role(
        &self,
        org_id: &str,
        user_id: &str,
        role_id: &str,
        assigned_by: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO user_org_roles (user_id, org_id, role_id, assigned_at, assigned_by)
             VALUES ($1, $2, $3, NOW(), $4)
             ON CONFLICT (user_id, org_id, role_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(org_id)
        .bind(role_id)
        .bind(assigned_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn revoke_role(&self, org_id: &str, user_id: &str, role_id: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM user_org_roles WHERE user_id = $1 AND org_id = $2 AND role_id = $3",
        )
        .bind(user_id)
        .bind(org_id)
        .bind(role_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn clear_user_role_assignments(&self, org_id: &str, user_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM user_org_roles WHERE user_id = $1 AND org_id = $2")
            .bind(user_id)
            .bind(org_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn sync_member_system_role(
        &self,
        org_id: &str,
        user_id: &str,
        member_role: &Role,
        assigned_by: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "DELETE FROM user_org_roles
             WHERE org_id = $1 AND user_id = $2
               AND role_id IN (
                 SELECT id FROM org_roles WHERE org_id = $1 AND is_system = true
               )",
        )
        .bind(org_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        let role_name = member_role.to_string();
        let role_row = sqlx::query(
            "SELECT id FROM org_roles WHERE org_id = $1 AND name = $2 AND is_system = true",
        )
        .bind(org_id)
        .bind(&role_name)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = role_row {
            let role_id: String = row.get("id");
            sqlx::query(
                "INSERT INTO user_org_roles (user_id, org_id, role_id, assigned_at, assigned_by)
                 VALUES ($1, $2, $3, NOW(), $4)
                 ON CONFLICT (user_id, org_id, role_id) DO NOTHING",
            )
            .bind(user_id)
            .bind(org_id)
            .bind(&role_id)
            .bind(assigned_by)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_user_permissions(
        &self,
        org_id: &str,
        user_id: &str,
    ) -> Result<std::collections::HashSet<String>> {
        let rows = sqlx::query(
            "SELECT DISTINCT unnest(r.permissions) AS perm
             FROM user_org_roles uur
             JOIN org_roles r ON r.id = uur.role_id
             WHERE uur.org_id = $1 AND uur.user_id = $2",
        )
        .bind(org_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.get("perm")).collect())
    }
}
