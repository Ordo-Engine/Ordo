use super::*;

impl PlatformStore {
    pub async fn upsert_server(&self, server: &ServerNode) -> Result<()> {
        sqlx::query(
            "INSERT INTO servers (id, name, url, token, org_id, labels, version, status, last_seen, registered_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (token) DO UPDATE SET
               name = EXCLUDED.name,
               url = EXCLUDED.url,
               org_id = EXCLUDED.org_id,
               version = EXCLUDED.version,
               status = 'online',
               last_seen = NOW()",
        )
        .bind(&server.id)
        .bind(&server.name)
        .bind(&server.url)
        .bind(&server.token)
        .bind(&server.org_id)
        .bind(sqlx::types::Json(&server.labels))
        .bind(&server.version)
        .bind(server.status.to_string())
        .bind(server.last_seen)
        .bind(server.registered_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_server(&self, id: &str) -> Result<Option<ServerNode>> {
        let row = sqlx::query(
            "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at
             FROM servers WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| row_to_server(&r)))
    }

    pub async fn find_server_by_token(&self, token: &str) -> Result<Option<ServerNode>> {
        let row = sqlx::query(
            "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at
             FROM servers WHERE token = $1",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| row_to_server(&r)))
    }

    pub async fn list_servers(&self, org_id: Option<&str>) -> Result<Vec<ServerNode>> {
        let rows = if let Some(oid) = org_id {
            sqlx::query(
                "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at
                 FROM servers WHERE org_id = $1 ORDER BY registered_at DESC",
            )
            .bind(oid)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at
                 FROM servers ORDER BY registered_at DESC",
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.iter().map(row_to_server).collect())
    }

    pub async fn update_server_heartbeat(&self, token: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "UPDATE servers SET last_seen = NOW(), status = 'online'
             WHERE token = $1 RETURNING id",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.get("id")))
    }

    pub async fn mark_stale_servers_degraded(
        &self,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE servers SET status = 'degraded'
             WHERE status = 'online' AND (last_seen IS NULL OR last_seen < $1)",
        )
        .bind(older_than)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn mark_stale_servers_offline(
        &self,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE servers SET status = 'offline'
             WHERE status IN ('online', 'degraded') AND (last_seen IS NULL OR last_seen < $1)",
        )
        .bind(older_than)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_stale_offline_servers(
        &self,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM servers
             WHERE status = 'offline' AND (last_seen IS NULL OR last_seen < $1)",
        )
        .bind(older_than)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_server(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM servers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
