use super::*;

fn row_to_notification(r: &sqlx::postgres::PgRow) -> Result<PlatformNotification> {
    let payload: sqlx::types::Json<serde_json::Value> = r.get("payload");
    Ok(PlatformNotification {
        id: r.get("id"),
        org_id: r.get("org_id"),
        user_id: r.get("user_id"),
        notif_type: r.get("type"),
        ref_id: r.get("ref_id"),
        ref_type: r.get("ref_type"),
        payload: payload.0,
        read_at: r.get("read_at"),
        created_at: r.get("created_at"),
    })
}

impl PlatformStore {
    #[allow(clippy::too_many_arguments)]
    pub async fn create_notification(
        &self,
        id: &str,
        org_id: &str,
        user_id: &str,
        notif_type: &str,
        ref_id: Option<&str>,
        ref_type: Option<&str>,
        payload: serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO notifications (id, org_id, user_id, type, ref_id, ref_type, payload, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(org_id)
        .bind(user_id)
        .bind(notif_type)
        .bind(ref_id)
        .bind(ref_type)
        .bind(payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_notifications(
        &self,
        user_id: &str,
        org_id: &str,
        limit: i64,
        offset: i64,
        unread_only: bool,
    ) -> Result<Vec<PlatformNotification>> {
        let rows = if unread_only {
            sqlx::query(
                "SELECT id, org_id, user_id, type, ref_id, ref_type, payload, read_at, created_at
                 FROM notifications
                 WHERE user_id = $1 AND org_id = $2 AND read_at IS NULL
                 ORDER BY created_at DESC
                 LIMIT $3 OFFSET $4",
            )
            .bind(user_id)
            .bind(org_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, org_id, user_id, type, ref_id, ref_type, payload, read_at, created_at
                 FROM notifications
                 WHERE user_id = $1 AND org_id = $2
                 ORDER BY created_at DESC
                 LIMIT $3 OFFSET $4",
            )
            .bind(user_id)
            .bind(org_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };
        rows.iter().map(row_to_notification).collect()
    }

    pub async fn mark_notification_read(&self, id: &str, user_id: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE notifications SET read_at = NOW()
             WHERE id = $1 AND user_id = $2 AND read_at IS NULL",
        )
        .bind(id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_all_notifications_read(&self, user_id: &str, org_id: &str) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE notifications SET read_at = NOW()
             WHERE user_id = $1 AND org_id = $2 AND read_at IS NULL",
        )
        .bind(user_id)
        .bind(org_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn get_unread_notification_count(&self, user_id: &str, org_id: &str) -> Result<i64> {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM notifications
             WHERE user_id = $1 AND org_id = $2 AND read_at IS NULL",
        )
        .bind(user_id)
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn list_pending_approvals_for_reviewer(
        &self,
        org_id: &str,
        reviewer_id: &str,
    ) -> Result<Vec<ReleaseRequest>> {
        let rows = sqlx::query(
            "SELECT DISTINCT rr.id, rr.org_id, rr.project_id, rr.ruleset_name, rr.version,
                    rr.environment_id, rr.policy_id, rr.status, rr.title, rr.change_summary,
                    rr.release_note, rr.affected_instance_count, rr.rollback_version,
                    rr.created_by, rr.created_at, rr.updated_at
             FROM release_requests rr
             INNER JOIN release_approvals ra ON ra.release_request_id = rr.id
             WHERE rr.org_id = $1
               AND ra.reviewer_id = $2
               AND ra.decision = 'pending'
               AND rr.status = 'pending_approval'
             ORDER BY rr.created_at DESC",
        )
        .bind(org_id)
        .bind(reviewer_id)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in &rows {
            let id: String = row.get("id");
            let project_id: String = row.get("project_id");
            if let Ok(Some(r)) = self.get_release_request(org_id, &project_id, &id).await {
                results.push(r);
            }
        }
        Ok(results)
    }
}
