use super::*;

pub(super) fn row_to_user(r: &sqlx::postgres::PgRow) -> User {
    User {
        id: r.get("id"),
        email: r.get("email"),
        password_hash: r.get("password_hash"),
        display_name: r.get("display_name"),
        created_at: r.get("created_at"),
        last_login: r.get("last_login"),
    }
}

pub(super) fn row_to_project(r: &sqlx::postgres::PgRow) -> Project {
    Project {
        id: r.get("id"),
        name: r.get("name"),
        description: r.get("description"),
        org_id: r.get("org_id"),
        created_at: r.get("created_at"),
        created_by: r.get("created_by"),
        server_id: r.get("server_id"),
    }
}

pub(super) fn row_to_server(r: &sqlx::postgres::PgRow) -> ServerNode {
    use std::str::FromStr;
    let labels: sqlx::types::Json<serde_json::Value> = r.get("labels");
    let capabilities: sqlx::types::Json<serde_json::Value> = r.get("capabilities");
    let status_str: String = r.get("status");
    ServerNode {
        id: r.get("id"),
        name: r.get("name"),
        url: r.get("url"),
        token: r.get("token"),
        org_id: r.get("org_id"),
        labels: labels.0,
        version: r.get("version"),
        status: ServerStatus::from_str(&status_str).unwrap_or(ServerStatus::Offline),
        last_seen: r.get("last_seen"),
        registered_at: r.get("registered_at"),
        capabilities: capabilities.0,
    }
}

pub(super) fn row_to_environment(r: &sqlx::postgres::PgRow) -> ProjectEnvironment {
    ProjectEnvironment {
        id: r.get("id"),
        project_id: r.get("project_id"),
        name: r.get("name"),
        server_ids: r.get("server_ids"),
        nats_subject_prefix: r.get("nats_subject_prefix"),
        is_default: r.get("is_default"),
        canary_target_env_id: r.get("canary_target_env_id"),
        canary_percentage: r.get("canary_percentage"),
        created_at: r.get("created_at"),
    }
}

pub(super) fn row_to_ruleset_meta(r: &sqlx::postgres::PgRow) -> ProjectRulesetMeta {
    ProjectRulesetMeta {
        id: r.get("id"),
        project_id: r.get("project_id"),
        name: r.get("name"),
        draft_seq: r.get("draft_seq"),
        draft_updated_at: r.get("draft_updated_at"),
        draft_updated_by: r.get("draft_updated_by"),
        draft_version: r.get("draft_version"),
        published_version: r.get("published_version"),
        published_at: r.get("published_at"),
        created_at: r.get("created_at"),
    }
}

pub(super) fn row_to_ruleset(r: &sqlx::postgres::PgRow) -> ProjectRuleset {
    let draft: sqlx::types::Json<serde_json::Value> = r.get("draft");
    ProjectRuleset {
        meta: row_to_ruleset_meta(r),
        draft: draft.0,
    }
}

pub(super) fn row_to_deployment(r: &sqlx::postgres::PgRow) -> Result<RulesetDeployment> {
    use std::str::FromStr;
    let snapshot: sqlx::types::Json<serde_json::Value> = r.get("snapshot");
    let status_str: String = r.get("status");
    Ok(RulesetDeployment {
        id: r.get("id"),
        project_id: r.get("project_id"),
        environment_id: r.get("environment_id"),
        environment_name: r.try_get("environment_name").ok(),
        ruleset_name: r.get("ruleset_name"),
        version: r.get("version"),
        release_note: r.get("release_note"),
        snapshot: snapshot.0,
        deployed_at: r.get("deployed_at"),
        deployed_by: r.get("deployed_by"),
        status: DeploymentStatus::from_str(&status_str).unwrap_or(DeploymentStatus::Failed),
    })
}

pub(super) fn row_to_release_policy(r: &sqlx::postgres::PgRow) -> Result<ReleasePolicy> {
    use std::str::FromStr;
    let rollout_strategy: sqlx::types::Json<RolloutStrategy> = r.get("rollout_strategy");
    let rollback_policy: sqlx::types::Json<RollbackPolicy> = r.get("rollback_policy");
    let scope: String = r.get("scope");
    let target_type: String = r.get("target_type");
    Ok(ReleasePolicy {
        id: r.get("id"),
        org_id: r.get("org_id"),
        project_id: r.get("project_id"),
        name: r.get("name"),
        scope: ReleasePolicyScope::from_str(&scope).map_err(|e| anyhow::anyhow!(e))?,
        target_type: ReleasePolicyTargetType::from_str(&target_type)
            .map_err(|e| anyhow::anyhow!(e))?,
        target_id: r.get("target_id"),
        description: r.get("description"),
        min_approvals: r.get("min_approvals"),
        allow_self_approval: r.get("allow_self_approval"),
        approver_ids: r.get("approver_ids"),
        rollout_strategy: rollout_strategy.0,
        rollback_policy: rollback_policy.0,
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    })
}

pub(super) fn row_to_release_request(r: &sqlx::postgres::PgRow) -> Result<ReleaseRequest> {
    use std::str::FromStr;
    let status: String = r.get("status");
    let parsed_status = ReleaseRequestStatus::from_str(&status).map_err(|e| anyhow::anyhow!(e))?;
    let rollout_strategy: Option<sqlx::types::Json<RolloutStrategy>> =
        r.try_get("rollout_strategy").ok();
    let version_diff: Option<sqlx::types::Json<ReleaseVersionDiff>> =
        r.try_get("version_diff").ok();
    let content_diff: Option<sqlx::types::Json<ReleaseContentDiffSummary>> =
        r.try_get("content_diff").ok();
    let request_snapshot: Option<sqlx::types::Json<ReleaseRequestSnapshot>> =
        r.try_get("request_snapshot").ok();
    Ok(ReleaseRequest {
        id: r.get("id"),
        org_id: r.get("org_id"),
        project_id: r.get("project_id"),
        ruleset_name: r.get("ruleset_name"),
        version: r.get("version"),
        environment_id: r.get("environment_id"),
        environment_name: r.try_get("environment_name").ok(),
        policy_id: r.get("policy_id"),
        status: parsed_status.clone(),
        title: r.get("title"),
        change_summary: r.get("change_summary"),
        release_note: r.get("release_note"),
        affected_instance_count: r.get("affected_instance_count"),
        rollout_strategy: rollout_strategy.map(|v| v.0).unwrap_or_default(),
        rollback_version: r.get("rollback_version"),
        created_by: r.get("created_by"),
        created_by_name: r.try_get("created_by_name").ok(),
        created_by_email: r.try_get("created_by_email").ok(),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        version_diff: version_diff.map(|v| v.0).unwrap_or_default(),
        content_diff: content_diff.map(|v| v.0).unwrap_or_default(),
        request_snapshot: request_snapshot.map(|v| v.0).unwrap_or_default(),
        execution_attempts: r.try_get("execution_attempts").unwrap_or(0),
        max_execution_attempts: crate::release::MAX_RELEASE_EXECUTION_ATTEMPTS as i32,
        is_closed: matches!(
            parsed_status,
            ReleaseRequestStatus::RolledBack | ReleaseRequestStatus::Cancelled
        ),
        approvals: Vec::new(),
    })
}

pub(super) fn row_to_release_approval(r: &sqlx::postgres::PgRow) -> Result<ReleaseApprovalRecord> {
    use std::str::FromStr;
    let decision: String = r.get("decision");
    Ok(ReleaseApprovalRecord {
        id: r.get("id"),
        release_request_id: r.get("release_request_id"),
        stage: r.get("stage"),
        reviewer_id: r.get("reviewer_id"),
        reviewer_name: r.try_get("reviewer_name").ok(),
        reviewer_email: r.try_get("reviewer_email").ok(),
        decision: ReleaseApprovalDecision::from_str(&decision).map_err(|e| anyhow::anyhow!(e))?,
        comment: r.get("comment"),
        decided_at: r.get("decided_at"),
        created_at: r.get("created_at"),
    })
}

pub(super) fn row_to_release_execution(r: &sqlx::postgres::PgRow) -> Result<ReleaseExecution> {
    use std::str::FromStr;
    let status: String = r.get("status");
    let strategy: sqlx::types::Json<RolloutStrategy> = r.get("strategy_snapshot");
    Ok(ReleaseExecution {
        id: r.get("id"),
        request_id: r.get("release_request_id"),
        status: ReleaseExecutionStatus::from_str(&status).map_err(|e| anyhow::anyhow!(e))?,
        started_at: r.get("started_at"),
        current_batch: r.get("current_batch"),
        total_batches: r.get("total_batches"),
        next_batch_at: r.try_get("next_batch_at").ok(),
        strategy: strategy.0,
        summary: ReleaseExecutionSummary::default(),
        instances: Vec::new(),
    })
}

pub(super) fn row_to_release_execution_instance(
    r: &sqlx::postgres::PgRow,
) -> Result<ReleaseExecutionInstance> {
    use std::str::FromStr;
    let status: String = r.get("status");
    let metric_summary: Option<sqlx::types::Json<JsonValue>> = r.try_get("metric_summary").ok();
    Ok(ReleaseExecutionInstance {
        id: r.get("id"),
        release_execution_id: r.get("release_execution_id"),
        instance_id: r.get("instance_id"),
        instance_name: r.get("instance_name"),
        zone: r.try_get("zone").ok(),
        batch_index: r.get("batch_index"),
        current_version: r.get("current_version"),
        target_version: r.get("target_version"),
        status: ReleaseInstanceStatus::from_str(&status).map_err(|e| anyhow::anyhow!(e))?,
        scheduled_at: r.try_get("scheduled_at").ok(),
        updated_at: r.get("updated_at"),
        message: r.try_get("message").ok(),
        metric_summary: metric_summary.map(|v| v.0),
    })
}

pub(super) fn summarize_release_execution_instances(
    instances: &[ReleaseExecutionInstance],
) -> ReleaseExecutionSummary {
    let total_instances = instances.len() as i32;
    let succeeded_instances = instances
        .iter()
        .filter(|item| {
            item.status == ReleaseInstanceStatus::Success
                || item.status == ReleaseInstanceStatus::RolledBack
        })
        .count() as i32;
    let failed_instances = instances
        .iter()
        .filter(|item| item.status == ReleaseInstanceStatus::Failed)
        .count() as i32;
    let pending_instances = instances
        .iter()
        .filter(|item| {
            !matches!(
                item.status,
                ReleaseInstanceStatus::Success
                    | ReleaseInstanceStatus::RolledBack
                    | ReleaseInstanceStatus::Failed
                    | ReleaseInstanceStatus::Skipped
            )
        })
        .count() as i32;

    ReleaseExecutionSummary {
        total_instances,
        succeeded_instances,
        failed_instances,
        pending_instances,
    }
}

pub(super) fn row_to_release_execution_event(
    r: &sqlx::postgres::PgRow,
) -> Result<ReleaseExecutionEvent> {
    let payload: sqlx::types::Json<serde_json::Value> = r.get("payload");
    Ok(ReleaseExecutionEvent {
        id: r.get("id"),
        release_execution_id: r.get("release_execution_id"),
        instance_id: r.try_get("instance_id").ok(),
        event_type: r.get("event_type"),
        payload: payload.0,
        created_at: r.get("created_at"),
    })
}

pub(super) fn row_to_release_request_history(
    r: &sqlx::postgres::PgRow,
) -> Result<ReleaseRequestHistoryEntry> {
    use std::str::FromStr;

    let scope: String = r.get("scope");
    let actor_type: String = r.get("actor_type");
    let detail: sqlx::types::Json<serde_json::Value> = r.get("detail");

    Ok(ReleaseRequestHistoryEntry {
        id: r.get("id"),
        release_request_id: r.get("release_request_id"),
        release_execution_id: r.try_get("release_execution_id").ok(),
        instance_id: r.try_get("instance_id").ok(),
        scope: ReleaseHistoryScope::from_str(&scope).map_err(|e| anyhow::anyhow!(e))?,
        action: r.get("action"),
        actor_type: ReleaseHistoryActorType::from_str(&actor_type)
            .map_err(|e| anyhow::anyhow!(e))?,
        actor_id: r.try_get("actor_id").ok(),
        actor_name: r.try_get("actor_name").ok(),
        actor_email: r.try_get("actor_email").ok(),
        from_status: r.try_get("from_status").ok(),
        to_status: r.try_get("to_status").ok(),
        detail: detail.0,
        created_at: r.get("created_at"),
    })
}

pub(super) fn row_to_org_role(r: &sqlx::postgres::PgRow) -> OrgRole {
    let permissions: Vec<String> = r.get("permissions");
    OrgRole {
        id: r.get("id"),
        org_id: r.get("org_id"),
        name: r.get("name"),
        description: r.get("description"),
        permissions,
        is_system: r.get("is_system"),
        created_at: r.get("created_at"),
    }
}

pub(super) fn row_to_user_role(r: &sqlx::postgres::PgRow) -> UserRoleAssignment {
    UserRoleAssignment {
        user_id: r.get("user_id"),
        org_id: r.get("org_id"),
        role_id: r.get("role_id"),
        role_name: r.try_get("role_name").ok(),
        assigned_at: r.get("assigned_at"),
        assigned_by: r.get("assigned_by"),
    }
}
