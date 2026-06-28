use super::*;

impl PlatformStore {
    pub async fn list_release_policies(
        &self,
        org_id: &str,
        project_id: Option<&str>,
    ) -> Result<Vec<ReleasePolicy>> {
        let rows = sqlx::query(
            "SELECT id, org_id, project_id, name, scope, target_type, target_id, description,
                    min_approvals, allow_self_approval, approver_ids, rollout_strategy,
                    rollback_policy, created_at, updated_at
             FROM release_policies
             WHERE org_id = $1 AND ($2::text IS NULL OR project_id = $2 OR project_id IS NULL)
             ORDER BY project_id NULLS FIRST, name",
        )
        .bind(org_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_release_policy).collect()
    }

    pub async fn get_release_policy(
        &self,
        org_id: &str,
        project_id: &str,
        policy_id: &str,
    ) -> Result<Option<ReleasePolicy>> {
        let row = sqlx::query(
            "SELECT id, org_id, project_id, name, scope, target_type, target_id, description,
                    min_approvals, allow_self_approval, approver_ids, rollout_strategy,
                    rollback_policy, created_at, updated_at
             FROM release_policies
             WHERE id = $1 AND org_id = $2 AND (project_id = $3 OR project_id IS NULL)",
        )
        .bind(policy_id)
        .bind(org_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_release_policy).transpose()
    }

    pub async fn create_release_policy(
        &self,
        id: &str,
        org_id: &str,
        project_id: Option<&str>,
        req: &CreateReleasePolicyRequest,
    ) -> Result<ReleasePolicy> {
        sqlx::query(
            "INSERT INTO release_policies
             (id, org_id, project_id, name, scope, target_type, target_id, description,
              min_approvals, allow_self_approval, approver_ids, rollout_strategy,
              rollback_policy, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW(), NOW())",
        )
        .bind(id)
        .bind(org_id)
        .bind(project_id)
        .bind(&req.name)
        .bind(req.scope.to_string())
        .bind(req.target_type.to_string())
        .bind(&req.target_id)
        .bind(&req.description)
        .bind(req.min_approvals)
        .bind(req.allow_self_approval)
        .bind(&req.approver_ids)
        .bind(sqlx::types::Json(req.rollout_strategy.clone()))
        .bind(sqlx::types::Json(req.rollback_policy.clone()))
        .execute(&self.pool)
        .await?;
        Ok(self
            .get_release_policy(org_id, project_id.unwrap_or_default(), id)
            .await?
            .expect("just inserted"))
    }

    pub async fn update_release_policy(
        &self,
        org_id: &str,
        project_id: &str,
        policy_id: &str,
        req: &UpdateReleasePolicyRequest,
    ) -> Result<bool> {
        let Some(current) = self
            .get_release_policy(org_id, project_id, policy_id)
            .await?
        else {
            return Ok(false);
        };
        let result = sqlx::query(
            "UPDATE release_policies SET
               name = $1,
               description = $2,
               min_approvals = $3,
               allow_self_approval = $4,
               approver_ids = $5,
               rollout_strategy = $6,
               rollback_policy = $7,
               updated_at = NOW()
             WHERE id = $8 AND org_id = $9 AND (project_id = $10 OR project_id IS NULL)",
        )
        .bind(req.name.as_deref().unwrap_or(&current.name))
        .bind(req.description.as_ref().or(current.description.as_ref()))
        .bind(req.min_approvals.unwrap_or(current.min_approvals))
        .bind(
            req.allow_self_approval
                .unwrap_or(current.allow_self_approval),
        )
        .bind(req.approver_ids.as_ref().unwrap_or(&current.approver_ids))
        .bind(sqlx::types::Json(
            req.rollout_strategy
                .clone()
                .unwrap_or_else(|| current.rollout_strategy.clone()),
        ))
        .bind(sqlx::types::Json(
            req.rollback_policy
                .clone()
                .unwrap_or_else(|| current.rollback_policy.clone()),
        ))
        .bind(policy_id)
        .bind(org_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_release_policy(
        &self,
        org_id: &str,
        project_id: &str,
        policy_id: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM release_policies
             WHERE id = $1 AND org_id = $2 AND (project_id = $3 OR project_id IS NULL)",
        )
        .bind(policy_id)
        .bind(org_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn find_matching_release_policy(
        &self,
        org_id: &str,
        project_id: &str,
        environment_id: &str,
    ) -> Result<Option<ReleasePolicy>> {
        let row = sqlx::query(
            "SELECT id, org_id, project_id, name, scope, target_type, target_id, description,
                    min_approvals, allow_self_approval, approver_ids, rollout_strategy,
                    rollback_policy, created_at, updated_at
             FROM release_policies
             WHERE org_id = $1
               AND (project_id = $2 OR project_id IS NULL)
               AND (
                 (target_type = 'environment' AND target_id = $3)
                 OR (target_type = 'project' AND target_id = $2)
               )
             ORDER BY
               CASE WHEN target_type = 'environment' THEN 0 ELSE 1 END,
               CASE WHEN project_id IS NULL THEN 1 ELSE 0 END,
               updated_at DESC
             LIMIT 1",
        )
        .bind(org_id)
        .bind(project_id)
        .bind(environment_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_release_policy).transpose()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_release_request(
        &self,
        id: &str,
        org_id: &str,
        project_id: &str,
        created_by: &str,
        created_by_name: Option<&str>,
        created_by_email: Option<&str>,
        req: &CreateReleaseRequest,
        current_version: Option<&str>,
        version_diff: &ReleaseVersionDiff,
        content_diff: &ReleaseContentDiffSummary,
        request_snapshot: &ReleaseRequestSnapshot,
    ) -> Result<ReleaseRequest> {
        sqlx::query(
            "INSERT INTO release_requests
             (id, org_id, project_id, ruleset_name, version, environment_id, policy_id, status,
              title, change_summary, release_note, affected_instance_count, rollback_version,
              created_by, created_by_name, created_by_email, current_version, version_diff, content_diff,
              request_snapshot, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                     $18, $19, $20, NOW(), NOW())",
        )
        .bind(id)
        .bind(org_id)
        .bind(project_id)
        .bind(&req.ruleset_name)
        .bind(&req.version)
        .bind(&req.environment_id)
        .bind(&req.policy_id)
        .bind(ReleaseRequestStatus::PendingApproval.to_string())
        .bind(&req.title)
        .bind(&req.change_summary)
        .bind(&req.release_note)
        .bind(req.affected_instance_count.unwrap_or_default())
        .bind(&req.rollback_version)
        .bind(created_by)
        .bind(created_by_name)
        .bind(created_by_email)
        .bind(current_version)
        .bind(sqlx::types::Json(version_diff))
        .bind(sqlx::types::Json(content_diff))
        .bind(sqlx::types::Json(request_snapshot))
        .execute(&self.pool)
        .await?;
        Ok(self
            .get_release_request(org_id, project_id, id)
            .await?
            .expect("just inserted"))
    }

    pub async fn list_release_requests(
        &self,
        org_id: &str,
        project_id: &str,
    ) -> Result<Vec<ReleaseRequest>> {
        let rows = sqlx::query(
            "SELECT rr.id, rr.org_id, rr.project_id, rr.ruleset_name, rr.version, rr.environment_id,
                    env.name AS environment_name, rr.policy_id, rr.status, rr.title,
                    rr.change_summary, rr.release_note, rr.affected_instance_count,
                    rp.rollout_strategy,
                    (SELECT COUNT(*)::int FROM release_executions re WHERE re.release_request_id = rr.id) AS execution_attempts,
                    rr.rollback_version, rr.created_by, COALESCE(rr.created_by_name, u.display_name) AS created_by_name,
                    rr.created_by_email, rr.version_diff, rr.content_diff, rr.request_snapshot,
                    rr.created_at, rr.updated_at
             FROM release_requests rr
             LEFT JOIN project_environments env ON env.id = rr.environment_id
             LEFT JOIN release_policies rp ON rp.id = rr.policy_id
             LEFT JOIN users u ON u.id = rr.created_by
             WHERE rr.org_id = $1 AND rr.project_id = $2
             ORDER BY rr.created_at DESC",
        )
        .bind(org_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let mut item = row_to_release_request(&row)?;
            item.approvals = self.list_release_approvals(&item.id).await?;
            items.push(item);
        }
        Ok(items)
    }

    pub async fn get_release_request(
        &self,
        org_id: &str,
        project_id: &str,
        release_id: &str,
    ) -> Result<Option<ReleaseRequest>> {
        let row = sqlx::query(
            "SELECT rr.id, rr.org_id, rr.project_id, rr.ruleset_name, rr.version, rr.environment_id,
                    env.name AS environment_name, rr.policy_id, rr.status, rr.title,
                    rr.change_summary, rr.release_note, rr.affected_instance_count,
                    rp.rollout_strategy,
                    (SELECT COUNT(*)::int FROM release_executions re WHERE re.release_request_id = rr.id) AS execution_attempts,
                    rr.rollback_version, rr.created_by, COALESCE(rr.created_by_name, u.display_name) AS created_by_name,
                    rr.created_by_email, rr.version_diff, rr.content_diff, rr.request_snapshot,
                    rr.created_at, rr.updated_at
             FROM release_requests rr
             LEFT JOIN project_environments env ON env.id = rr.environment_id
             LEFT JOIN release_policies rp ON rp.id = rr.policy_id
             LEFT JOIN users u ON u.id = rr.created_by
             WHERE rr.id = $1 AND rr.org_id = $2 AND rr.project_id = $3",
        )
        .bind(release_id)
        .bind(org_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let mut item = row_to_release_request(&row)?;
        item.approvals = self.list_release_approvals(&item.id).await?;
        Ok(Some(item))
    }

    pub async fn get_release_request_by_id(
        &self,
        release_id: &str,
    ) -> Result<Option<ReleaseRequest>> {
        let row = sqlx::query(
            "SELECT rr.id, rr.org_id, rr.project_id, rr.ruleset_name, rr.version, rr.environment_id,
                    env.name AS environment_name, rr.policy_id, rr.status, rr.title,
                    rr.change_summary, rr.release_note, rr.affected_instance_count,
                    rp.rollout_strategy,
                    (SELECT COUNT(*)::int FROM release_executions re WHERE re.release_request_id = rr.id) AS execution_attempts,
                    rr.rollback_version, rr.created_by, COALESCE(rr.created_by_name, u.display_name) AS created_by_name,
                    rr.created_by_email, rr.version_diff, rr.content_diff, rr.request_snapshot,
                    rr.created_at, rr.updated_at
             FROM release_requests rr
             LEFT JOIN project_environments env ON env.id = rr.environment_id
             LEFT JOIN release_policies rp ON rp.id = rr.policy_id
             LEFT JOIN users u ON u.id = rr.created_by
             WHERE rr.id = $1",
        )
        .bind(release_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let mut item = row_to_release_request(&row)?;
        item.approvals = self.list_release_approvals(&item.id).await?;
        Ok(Some(item))
    }

    pub async fn create_release_approval(
        &self,
        id: &str,
        release_request_id: &str,
        stage: i32,
        reviewer_id: &str,
        reviewer_name: Option<&str>,
        reviewer_email: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO release_approvals
             (id, release_request_id, stage, reviewer_id, reviewer_name, reviewer_email, decision, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())",
        )
        .bind(id)
        .bind(release_request_id)
        .bind(stage)
        .bind(reviewer_id)
        .bind(reviewer_name)
        .bind(reviewer_email)
        .bind(ReleaseApprovalDecision::Pending.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_release_approvals(
        &self,
        release_request_id: &str,
    ) -> Result<Vec<ReleaseApprovalRecord>> {
        let rows = sqlx::query(
            "SELECT ra.id, ra.release_request_id, ra.stage, ra.reviewer_id,
                    COALESCE(ra.reviewer_name, u.display_name) AS reviewer_name,
                    COALESCE(ra.reviewer_email, u.email) AS reviewer_email,
                    ra.decision, ra.comment, ra.decided_at, ra.created_at
             FROM release_approvals ra
             LEFT JOIN users u ON u.id = ra.reviewer_id
             WHERE ra.release_request_id = $1
             ORDER BY ra.stage, ra.created_at",
        )
        .bind(release_request_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_release_approval).collect()
    }

    pub async fn review_release_request(
        &self,
        release_request_id: &str,
        reviewer_id: &str,
        decision: ReleaseApprovalDecision,
        comment: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE release_approvals
             SET decision = $1, comment = $2, decided_at = NOW()
             WHERE release_request_id = $3 AND reviewer_id = $4 AND decision = 'pending'",
        )
        .bind(decision.to_string())
        .bind(comment)
        .bind(release_request_id)
        .bind(reviewer_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_release_request_status(
        &self,
        release_request_id: &str,
        status: ReleaseRequestStatus,
    ) -> Result<()> {
        sqlx::query("UPDATE release_requests SET status = $1, updated_at = NOW() WHERE id = $2")
            .bind(status.to_string())
            .bind(release_request_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_release_execution(
        &self,
        id: &str,
        release_request_id: &str,
        status: ReleaseExecutionStatus,
        current_batch: i32,
        total_batches: i32,
        strategy: &RolloutStrategy,
        triggered_by: Option<&str>,
    ) -> Result<ReleaseExecution> {
        sqlx::query(
            "INSERT INTO release_executions
             (id, release_request_id, status, current_batch, total_batches, next_batch_at, strategy_snapshot, started_at, triggered_by)
             VALUES ($1, $2, $3, $4, $5, NULL, $6, NOW(), $7)",
        )
        .bind(id)
        .bind(release_request_id)
        .bind(status.to_string())
        .bind(current_batch)
        .bind(total_batches)
        .bind(sqlx::types::Json(strategy.clone()))
        .bind(triggered_by)
        .execute(&self.pool)
        .await?;
        Ok(self
            .get_release_execution(id)
            .await?
            .expect("just inserted"))
    }

    pub async fn update_release_execution_status(
        &self,
        execution_id: &str,
        status: ReleaseExecutionStatus,
        current_batch: Option<i32>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE release_executions
             SET status = $1,
                 current_batch = COALESCE($2, current_batch),
                 finished_at = CASE
                   WHEN $1 IN ('completed', 'failed', 'rollback_failed') THEN NOW()
                   WHEN $1 IN ('preparing', 'waiting_start', 'rolling_out', 'paused', 'verifying', 'rollback_in_progress') THEN NULL
                   ELSE finished_at
                 END
             WHERE id = $3",
        )
        .bind(status.to_string())
        .bind(current_batch)
        .bind(execution_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_release_execution_next_batch_at(
        &self,
        execution_id: &str,
        next_batch_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE release_executions
             SET next_batch_at = $1
             WHERE id = $2",
        )
        .bind(next_batch_at)
        .bind(execution_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn create_release_execution_event(
        &self,
        id: &str,
        execution_id: &str,
        instance_id: Option<&str>,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO release_execution_events
             (id, release_execution_id, instance_id, event_type, payload, created_at)
             VALUES ($1, $2, $3, $4, $5, NOW())",
        )
        .bind(id)
        .bind(execution_id)
        .bind(instance_id)
        .bind(event_type)
        .bind(payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_release_request_history(
        &self,
        id: &str,
        release_request_id: &str,
        release_execution_id: Option<&str>,
        instance_id: Option<&str>,
        scope: ReleaseHistoryScope,
        action: &str,
        actor: &ReleaseHistoryActor,
        from_status: Option<&str>,
        to_status: Option<&str>,
        detail: serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO release_request_history
             (id, release_request_id, release_execution_id, instance_id, scope, action,
              actor_type, actor_id, actor_name, actor_email, from_status, to_status, detail, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW())",
        )
        .bind(id)
        .bind(release_request_id)
        .bind(release_execution_id)
        .bind(instance_id)
        .bind(scope.to_string())
        .bind(action)
        .bind(actor.actor_type.to_string())
        .bind(&actor.actor_id)
        .bind(&actor.actor_name)
        .bind(&actor.actor_email)
        .bind(from_status)
        .bind(to_status)
        .bind(detail)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn create_release_execution_instance(
        &self,
        instance: &ReleaseExecutionInstance,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO release_execution_instances
             (id, release_execution_id, instance_id, instance_name, zone, batch_index, current_version, target_version,
              status, scheduled_at, message, metric_summary, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, COALESCE($12::jsonb, '{}'::jsonb), $13)",
        )
        .bind(&instance.id)
        .bind(&instance.release_execution_id)
        .bind(&instance.instance_id)
        .bind(&instance.instance_name)
        .bind(&instance.zone)
        .bind(instance.batch_index)
        .bind(&instance.current_version)
        .bind(&instance.target_version)
        .bind(instance.status.to_string())
        .bind(instance.scheduled_at)
        .bind(&instance.message)
        .bind(instance.metric_summary.as_ref())
        .bind(instance.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_release_execution_instance(
        &self,
        instance_id: &str,
        status: ReleaseInstanceStatus,
        message: Option<&str>,
        metric_summary: Option<&str>,
    ) -> Result<()> {
        let promote_current_version = matches!(
            status,
            ReleaseInstanceStatus::Success | ReleaseInstanceStatus::RolledBack
        );
        sqlx::query(
            "UPDATE release_execution_instances
             SET status = $1,
                 message = $2,
                 scheduled_at = NULL,
                 current_version = CASE WHEN $3 THEN target_version ELSE current_version END,
                 metric_summary = COALESCE($4::jsonb, metric_summary),
                 updated_at = NOW()
             WHERE id = $5",
        )
        .bind(status.to_string())
        .bind(message)
        .bind(promote_current_version)
        .bind(metric_summary.map(|value| serde_json::json!({ "event": value })))
        .bind(instance_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_release_execution_instance_plan(
        &self,
        instance_id: &str,
        target_version: &str,
        status: ReleaseInstanceStatus,
        scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
        message: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE release_execution_instances
             SET target_version = $1,
                 status = $2,
                 scheduled_at = $3,
                 message = $4,
                 updated_at = NOW()
             WHERE id = $5",
        )
        .bind(target_version)
        .bind(status.to_string())
        .bind(scheduled_at)
        .bind(message)
        .bind(instance_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_release_execution_instance_schedule(
        &self,
        instance_id: &str,
        status: ReleaseInstanceStatus,
        scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
        message: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE release_execution_instances
             SET status = $1,
                 scheduled_at = $2,
                 message = COALESCE($3, message),
                 updated_at = NOW()
             WHERE id = $4",
        )
        .bind(status.to_string())
        .bind(scheduled_at)
        .bind(message)
        .bind(instance_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_release_execution_batch_schedule(
        &self,
        execution_id: &str,
        batch_index: i32,
        status: ReleaseInstanceStatus,
        scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
        message: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE release_execution_instances
             SET status = $1,
                 scheduled_at = $2,
                 message = $3,
                 updated_at = NOW()
             WHERE release_execution_id = $4
               AND batch_index = $5
               AND status NOT IN ('success', 'failed', 'rolled_back', 'skipped')",
        )
        .bind(status.to_string())
        .bind(scheduled_at)
        .bind(message)
        .bind(execution_id)
        .bind(batch_index)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_release_execution_instance_by_target(
        &self,
        execution_id: &str,
        target_instance_id: &str,
    ) -> Result<Option<ReleaseExecutionInstance>> {
        let row = sqlx::query(
            "SELECT id, release_execution_id, instance_id, instance_name, zone, batch_index, current_version,
                    target_version, status, scheduled_at, message, metric_summary, updated_at
             FROM release_execution_instances
             WHERE release_execution_id = $1 AND instance_id = $2
             LIMIT 1",
        )
        .bind(execution_id)
        .bind(target_instance_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| row_to_release_execution_instance(&r))
            .transpose()
    }

    pub async fn get_release_execution_instance(
        &self,
        instance_id: &str,
    ) -> Result<Option<ReleaseExecutionInstance>> {
        let row = sqlx::query(
            "SELECT id, release_execution_id, instance_id, instance_name, zone, batch_index, current_version,
                    target_version, status, scheduled_at, message, metric_summary, updated_at
             FROM release_execution_instances
             WHERE id = $1
             LIMIT 1",
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| row_to_release_execution_instance(&r))
            .transpose()
    }

    pub async fn list_release_execution_instances(
        &self,
        execution_id: &str,
    ) -> Result<Vec<ReleaseExecutionInstance>> {
        let rows = sqlx::query(
            "SELECT id, release_execution_id, instance_id, instance_name, zone, batch_index, current_version,
                    target_version, status, scheduled_at, message, metric_summary, updated_at
             FROM release_execution_instances
             WHERE release_execution_id = $1
             ORDER BY batch_index ASC, instance_name ASC, updated_at DESC",
        )
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_release_execution_instance).collect()
    }

    pub async fn get_release_execution(
        &self,
        execution_id: &str,
    ) -> Result<Option<ReleaseExecution>> {
        let row = sqlx::query(
            "SELECT id, release_request_id, status, current_batch, total_batches, next_batch_at, strategy_snapshot, started_at
             FROM release_executions
             WHERE id = $1",
        )
        .bind(execution_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut item = row_to_release_execution(&row)?;
        item.instances = self.list_release_execution_instances(&item.id).await?;
        item.summary = summarize_release_execution_instances(&item.instances);
        Ok(Some(item))
    }

    pub async fn find_release_execution_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<ReleaseExecution>> {
        let row = sqlx::query(
            "SELECT id, release_request_id, status, current_batch, total_batches, next_batch_at, strategy_snapshot, started_at
             FROM release_executions
             WHERE release_request_id = $1
             ORDER BY started_at DESC
             LIMIT 1",
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut item = row_to_release_execution(&row)?;
        item.instances = self.list_release_execution_instances(&item.id).await?;
        item.summary = summarize_release_execution_instances(&item.instances);
        Ok(Some(item))
    }

    pub async fn list_worker_claimable_release_executions(
        &self,
        limit: i64,
    ) -> Result<Vec<ReleaseExecution>> {
        let rows = sqlx::query(
            "SELECT id, release_request_id, status, current_batch, total_batches, next_batch_at, strategy_snapshot, started_at
             FROM release_executions
             WHERE status IN ('preparing', 'waiting_start', 'rolling_out', 'rollback_in_progress')
             ORDER BY started_at ASC
             LIMIT $1",
        )
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let mut item = row_to_release_execution(&row)?;
            item.instances = self.list_release_execution_instances(&item.id).await?;
            item.summary = summarize_release_execution_instances(&item.instances);
            items.push(item);
        }
        Ok(items)
    }

    pub async fn try_lock_release_execution(
        &self,
        execution_id: &str,
    ) -> Result<Option<sqlx::pool::PoolConnection<sqlx::Postgres>>> {
        let mut conn = self.pool.acquire().await?;
        let lock_key = format!("ordo-platform:release-execution:{execution_id}");
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtext($1), 0)")
            .bind(lock_key)
            .fetch_one(&mut *conn)
            .await?;

        if locked {
            Ok(Some(conn))
        } else {
            Ok(None)
        }
    }

    pub async fn count_release_executions_by_request(&self, request_id: &str) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint
             FROM release_executions
             WHERE release_request_id = $1",
        )
        .bind(request_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    /// On startup, mark any release execution stuck in an active non-terminal state as terminal.
    /// These are executions where the platform spawned a background task that was killed mid-run.
    /// Rollback flows become `rollback_failed`; rollout flows become `failed`.
    pub async fn fail_stuck_active_executions(&self) -> Result<u64> {
        let result = sqlx::query(
            "WITH stuck AS (
                UPDATE release_executions
                SET status = CASE
                        WHEN status = 'rollback_in_progress' THEN 'rollback_failed'
                        ELSE 'failed'
                    END,
                    finished_at = NOW()
                WHERE status IN ('preparing', 'waiting_start', 'rolling_out', 'verifying', 'rollback_in_progress')
                RETURNING release_request_id, status
            )
            UPDATE release_requests
            SET status = CASE
                    WHEN EXISTS (
                        SELECT 1 FROM stuck
                        WHERE stuck.release_request_id = release_requests.id
                          AND stuck.status = 'rollback_failed'
                    ) THEN 'rollback_failed'
                    ELSE 'failed'
                END
            WHERE id IN (SELECT release_request_id FROM stuck)
              AND status = 'executing'",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn find_latest_project_release_execution(
        &self,
        org_id: &str,
        project_id: &str,
    ) -> Result<Option<ReleaseExecution>> {
        let row = sqlx::query(
            "SELECT re.id, re.release_request_id, re.status, re.current_batch, re.total_batches,
                    re.next_batch_at, re.strategy_snapshot, re.started_at
             FROM release_executions re
             INNER JOIN release_requests rr ON rr.id = re.release_request_id
             WHERE rr.org_id = $1 AND rr.project_id = $2
             ORDER BY
               CASE WHEN re.status IN ('preparing', 'waiting_start', 'rolling_out', 'paused', 'verifying', 'rollback_in_progress') THEN 0 ELSE 1 END,
               re.started_at DESC
             LIMIT 1",
        )
        .bind(org_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut item = row_to_release_execution(&row)?;
        item.instances = self.list_release_execution_instances(&item.id).await?;
        item.summary = summarize_release_execution_instances(&item.instances);
        Ok(Some(item))
    }

    pub async fn update_execution_instance_metric_summary(
        &self,
        instance_id: &str,
        summary: serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE release_execution_instances
             SET metric_summary = $1, updated_at = NOW()
             WHERE id = $2",
        )
        .bind(summary)
        .bind(instance_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_release_execution_events(
        &self,
        execution_id: &str,
    ) -> Result<Vec<ReleaseExecutionEvent>> {
        let rows = sqlx::query(
            "SELECT id, release_execution_id, instance_id, event_type, payload, created_at
             FROM release_execution_events
             WHERE release_execution_id = $1
             ORDER BY created_at ASC",
        )
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_release_execution_event).collect()
    }

    pub async fn list_release_request_history(
        &self,
        release_request_id: &str,
    ) -> Result<Vec<ReleaseRequestHistoryEntry>> {
        let rows = sqlx::query(
            "SELECT id, release_request_id, release_execution_id, instance_id, scope, action,
                    actor_type, actor_id, actor_name, actor_email, from_status, to_status,
                    detail, created_at
             FROM release_request_history
             WHERE release_request_id = $1
             ORDER BY created_at ASC, id ASC",
        )
        .bind(release_request_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_release_request_history).collect()
    }
}
