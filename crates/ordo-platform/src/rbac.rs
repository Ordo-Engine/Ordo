//! RBAC permission checking utilities.
//!
//! Permission bits follow the pattern `resource:action`, e.g. `ruleset:publish`.
//! A user's effective permissions are the union of all roles assigned to them in the org.

use crate::{error::PlatformError, AppState};
use std::collections::HashSet;

// ── Permission constants ──────────────────────────────────────────────────────

pub const PERM_ORG_VIEW: &str = "org:view";
pub const PERM_ORG_MANAGE: &str = "org:manage";

pub const PERM_MEMBER_VIEW: &str = "member:view";
pub const PERM_MEMBER_INVITE: &str = "member:invite";
pub const PERM_MEMBER_REMOVE: &str = "member:remove";

pub const PERM_ROLE_VIEW: &str = "role:view";
pub const PERM_ROLE_MANAGE: &str = "role:manage";

pub const PERM_PROJECT_VIEW: &str = "project:view";
pub const PERM_PROJECT_CREATE: &str = "project:create";
pub const PERM_PROJECT_MANAGE: &str = "project:manage";
pub const PERM_PROJECT_DELETE: &str = "project:delete";

pub const PERM_RULESET_VIEW: &str = "ruleset:view";
pub const PERM_RULESET_EDIT: &str = "ruleset:edit";
pub const PERM_RULESET_PUBLISH: &str = "ruleset:publish";

pub const PERM_ENVIRONMENT_VIEW: &str = "environment:view";
pub const PERM_ENVIRONMENT_MANAGE: &str = "environment:manage";

pub const PERM_SERVER_VIEW: &str = "server:view";
pub const PERM_SERVER_MANAGE: &str = "server:manage";

pub const PERM_TEST_RUN: &str = "test:run";

pub const PERM_DEPLOYMENT_VIEW: &str = "deployment:view";
pub const PERM_DEPLOYMENT_REDEPLOY: &str = "deployment:redeploy";

pub const PERM_CANARY_MANAGE: &str = "canary:manage";

pub const PERM_RELEASE_POLICY_MANAGE: &str = "release:policy.manage";
pub const PERM_RELEASE_REQUEST_CREATE: &str = "release:request.create";
pub const PERM_RELEASE_REQUEST_VIEW: &str = "release:request.view";
pub const PERM_RELEASE_REQUEST_APPROVE: &str = "release:request.approve";
pub const PERM_RELEASE_REQUEST_REJECT: &str = "release:request.reject";
pub const PERM_RELEASE_EXECUTE: &str = "release:execute";
pub const PERM_RELEASE_PAUSE: &str = "release:pause";
pub const PERM_RELEASE_RESUME: &str = "release:resume";
pub const PERM_RELEASE_ROLLBACK: &str = "release:rollback";
pub const PERM_RELEASE_INSTANCE_VIEW: &str = "release:instance.view";

// ── Core check functions ──────────────────────────────────────────────────────

/// Return all permissions for `user_id` in `org_id` (union across all roles).
pub async fn user_permissions(
    state: &AppState,
    org_id: &str,
    user_id: &str,
) -> anyhow::Result<HashSet<String>> {
    state.store.get_user_permissions(org_id, user_id).await
}

/// Return `Ok(())` if the user has `perm`, otherwise a 403 `PlatformError`.
pub async fn require_permission(
    state: &AppState,
    org_id: &str,
    user_id: &str,
    perm: &str,
) -> Result<(), PlatformError> {
    // First check that the user is actually a member of the org.
    let is_member = state
        .store
        .get_org(org_id)
        .await
        .map_err(PlatformError::Internal)?
        .map(|org| org.members.iter().any(|m| m.user_id == user_id))
        .unwrap_or(false);

    if !is_member {
        return Err(PlatformError::forbidden(
            "Not a member of this organization",
        ));
    }

    let perms = state
        .store
        .get_user_permissions(org_id, user_id)
        .await
        .map_err(PlatformError::Internal)?;

    if perms.contains(perm) {
        Ok(())
    } else {
        Err(PlatformError::permission_required(perm))
    }
}

/// Check membership only (no specific permission required).
pub async fn require_membership(
    state: &AppState,
    org_id: &str,
    user_id: &str,
) -> Result<(), PlatformError> {
    require_permission(state, org_id, user_id, PERM_ORG_VIEW).await
}

/// Find which org owns a project and check `perm`.
/// Returns `(org_id, project_id)` on success.
pub async fn require_project_permission(
    state: &AppState,
    org_id: &str,
    project_id: &str,
    user_id: &str,
    perm: &str,
) -> Result<(), PlatformError> {
    // Verify project belongs to org
    state
        .store
        .get_project(org_id, project_id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Project not found"))?;

    require_permission(state, org_id, user_id, perm).await
}
