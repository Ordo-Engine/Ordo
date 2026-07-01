use super::*;

impl PlatformStore {
    pub async fn upsert_server(&self, server: &ServerNode) -> Result<()> {
        sqlx::query(
            "INSERT INTO servers (id, name, url, token, org_id, labels, version, status, last_seen, registered_at, capabilities)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name,
               url = EXCLUDED.url,
               token = EXCLUDED.token,
               -- Keep an already-assigned org; only a non-null incoming value
               -- (an explicit reassignment) overrides it. This stops NATS/HTTP
               -- re-registration from clobbering the connect-token-derived org.
               org_id = COALESCE(EXCLUDED.org_id, servers.org_id),
               version = EXCLUDED.version,
               capabilities = EXCLUDED.capabilities,
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
        .bind(sqlx::types::Json(&server.capabilities))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_server(&self, id: &str) -> Result<Option<ServerNode>> {
        let row = sqlx::query(
            "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at, capabilities
             FROM servers WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| row_to_server(&r)))
    }

    pub async fn find_server_by_token(&self, token: &str) -> Result<Option<ServerNode>> {
        let row = sqlx::query(
            "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at, capabilities
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
                "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at, capabilities
                 FROM servers WHERE org_id = $1 ORDER BY registered_at DESC",
            )
            .bind(oid)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at, capabilities
                 FROM servers ORDER BY registered_at DESC",
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.iter().map(row_to_server).collect())
    }

    pub async fn update_server_heartbeat(&self, server_id: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "UPDATE servers SET last_seen = NOW(), status = 'online'
             WHERE id = $1 RETURNING id",
        )
        .bind(server_id)
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

    // ── org connect tokens ──

    pub async fn create_connect_token(
        &self,
        id: &str,
        org_id: &str,
        token: &str,
        label: Option<&str>,
        created_by: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO org_connect_tokens (id, org_id, token, label, created_by)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(org_id)
        .bind(token)
        .bind(label)
        .bind(created_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Resolve a connect token to its org id, stamping `last_used_at`.
    pub async fn org_for_connect_token(&self, token: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "UPDATE org_connect_tokens SET last_used_at = NOW()
             WHERE token = $1 RETURNING org_id",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.get("org_id")))
    }

    pub async fn list_connect_tokens(&self, org_id: &str) -> Result<Vec<ConnectTokenInfo>> {
        let rows = sqlx::query(
            "SELECT id, label, created_at, last_used_at
             FROM org_connect_tokens WHERE org_id = $1 ORDER BY created_at DESC",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| ConnectTokenInfo {
                id: r.get("id"),
                label: r.get("label"),
                created_at: r.get("created_at"),
                last_used_at: r.get("last_used_at"),
            })
            .collect())
    }

    pub async fn delete_connect_token(&self, org_id: &str, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM org_connect_tokens WHERE id = $1 AND org_id = $2")
            .bind(id)
            .bind(org_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
