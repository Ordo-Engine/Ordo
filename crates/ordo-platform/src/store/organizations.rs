use super::*;

impl PlatformStore {
    pub async fn save_org(&self, org: &Organization) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO organizations (id, name, description, created_at, created_by, parent_org_id, depth)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name,
               description = EXCLUDED.description",
        )
        .bind(&org.id)
        .bind(&org.name)
        .bind(&org.description)
        .bind(org.created_at)
        .bind(&org.created_by)
        .bind(&org.parent_org_id)
        .bind(org.depth)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM members WHERE org_id = $1")
            .bind(&org.id)
            .execute(&mut *tx)
            .await?;

        for member in &org.members {
            sqlx::query(
                "INSERT INTO members (org_id, user_id, email, display_name, role, invited_at, invited_by)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&org.id)
            .bind(&member.user_id)
            .bind(&member.email)
            .bind(&member.display_name)
            .bind(role_to_str(&member.role))
            .bind(member.invited_at)
            .bind(&member.invited_by)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_org(&self, id: &str) -> Result<Option<Organization>> {
        let row = sqlx::query(
            "SELECT id, name, description, created_at, created_by, parent_org_id, depth
             FROM organizations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let members = self.fetch_members(id).await?;
        Ok(Some(Organization {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            created_at: row.get("created_at"),
            created_by: row.get("created_by"),
            parent_org_id: row.get("parent_org_id"),
            depth: row.get("depth"),
            members,
        }))
    }

    pub async fn list_user_orgs(&self, user_id: &str) -> Result<Vec<Organization>> {
        let rows = sqlx::query(
            "SELECT o.id, o.name, o.description, o.created_at, o.created_by, o.parent_org_id, o.depth
             FROM organizations o
             JOIN members m ON o.id = m.org_id
             WHERE m.user_id = $1
             ORDER BY o.depth, o.created_at",
        )
        .bind(user_id)
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

    pub async fn delete_org(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM organizations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_child_orgs(&self, parent_id: &str) -> Result<Vec<Organization>> {
        let rows = sqlx::query(
            "SELECT id, name, description, created_at, created_by, parent_org_id, depth
             FROM organizations WHERE parent_org_id = $1 ORDER BY created_at",
        )
        .bind(parent_id)
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

    pub async fn count_child_orgs(&self, parent_id: &str) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS cnt FROM organizations WHERE parent_org_id = $1")
            .bind(parent_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get("cnt"))
    }

    pub(crate) async fn fetch_members(&self, org_id: &str) -> Result<Vec<Member>> {
        let rows = sqlx::query(
            "SELECT user_id, email, display_name, role, invited_at, invited_by
             FROM members WHERE org_id = $1 ORDER BY invited_at",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let role_str: String = row.get("role");
                Ok(Member {
                    user_id: row.get("user_id"),
                    email: row.get("email"),
                    display_name: row.get("display_name"),
                    role: str_to_role(&role_str)?,
                    invited_at: row.get("invited_at"),
                    invited_by: row.get("invited_by"),
                })
            })
            .collect()
    }
}
