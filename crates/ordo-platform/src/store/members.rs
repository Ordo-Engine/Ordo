use super::*;

impl PlatformStore {
    pub async fn add_member(&self, org_id: &str, member: Member) -> Result<()> {
        sqlx::query(
            "INSERT INTO members (org_id, user_id, email, display_name, role, invited_at, invited_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (org_id, user_id) DO UPDATE SET
               email = EXCLUDED.email,
               display_name = EXCLUDED.display_name,
               role = EXCLUDED.role,
               invited_at = EXCLUDED.invited_at,
               invited_by = EXCLUDED.invited_by",
        )
        .bind(org_id)
        .bind(&member.user_id)
        .bind(&member.email)
        .bind(&member.display_name)
        .bind(role_to_str(&member.role))
        .bind(member.invited_at)
        .bind(&member.invited_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_member_role(
        &self,
        org_id: &str,
        user_id: &str,
        role: Role,
    ) -> Result<bool> {
        let result = sqlx::query("UPDATE members SET role = $1 WHERE org_id = $2 AND user_id = $3")
            .bind(role_to_str(&role))
            .bind(org_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn remove_member(&self, org_id: &str, user_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM members WHERE org_id = $1 AND user_id = $2")
            .bind(org_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
