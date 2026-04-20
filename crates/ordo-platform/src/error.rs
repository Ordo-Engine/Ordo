//! Platform API error types

use crate::i18n;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
struct ErrorDescriptor {
    code: &'static str,
    status: StatusCode,
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Permission '{0}' required")]
    PermissionRequired(String),

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Approver '{0}' is not a member of this organization")]
    ApproverNotMember(String),

    #[error("Requires {0} role or higher")]
    RequiresRoleOrHigher(String),

    #[error("Invalid base64 in GitHub content: {0}")]
    InvalidGitHubBase64(String),

    #[error("Invalid JSON in {path}: {details}")]
    InvalidJsonFile { path: String, details: String },

    #[error("{0} not found in this repository")]
    RepositoryFileNotFound(String),

    #[error("ordo-template.json does not match the required schema: {0}")]
    TemplateSchemaInvalid(String),

    #[error("GitHub search failed: {0}")]
    GitHubSearchFailed(String),

    #[error("GitHub repository lookup failed: {0}")]
    GitHubRepositoryLookupFailed(String),

    #[error("Invalid ruleset payload: {0}")]
    InvalidRulesetPayload(String),

    #[error("Invalid rollback payload: {0}")]
    InvalidRollbackPayload(String),

    #[error("RuleSet '{0}' not found")]
    RuleSetByNameNotFound(String),

    #[error("Version {seq} not found for rule '{name}'")]
    RuleSetVersionNotFound { name: String, seq: u32 },

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl PlatformError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }
    pub fn permission_required(permission: impl Into<String>) -> Self {
        Self::PermissionRequired(permission.into())
    }
    pub fn invalid_token(details: impl Into<String>) -> Self {
        Self::InvalidToken(details.into())
    }
    pub fn approver_not_member(user_id: impl Into<String>) -> Self {
        Self::ApproverNotMember(user_id.into())
    }
    pub fn requires_role_or_higher(role: impl Into<String>) -> Self {
        Self::RequiresRoleOrHigher(role.into())
    }
    pub fn invalid_github_base64(details: impl Into<String>) -> Self {
        Self::InvalidGitHubBase64(details.into())
    }
    pub fn invalid_json_file(path: impl Into<String>, details: impl Into<String>) -> Self {
        Self::InvalidJsonFile {
            path: path.into(),
            details: details.into(),
        }
    }
    pub fn repository_file_not_found(path: impl Into<String>) -> Self {
        Self::RepositoryFileNotFound(path.into())
    }
    pub fn template_schema_invalid(details: impl Into<String>) -> Self {
        Self::TemplateSchemaInvalid(details.into())
    }
    pub fn github_search_failed(details: impl Into<String>) -> Self {
        Self::GitHubSearchFailed(details.into())
    }
    pub fn github_repository_lookup_failed(details: impl Into<String>) -> Self {
        Self::GitHubRepositoryLookupFailed(details.into())
    }
    pub fn invalid_ruleset_payload(details: impl Into<String>) -> Self {
        Self::InvalidRulesetPayload(details.into())
    }
    pub fn invalid_rollback_payload(details: impl Into<String>) -> Self {
        Self::InvalidRollbackPayload(details.into())
    }
    pub fn ruleset_by_name_not_found(name: impl Into<String>) -> Self {
        Self::RuleSetByNameNotFound(name.into())
    }
    pub fn ruleset_version_not_found(name: impl Into<String>, seq: u32) -> Self {
        Self::RuleSetVersionNotFound {
            name: name.into(),
            seq,
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(anyhow::anyhow!(msg.into()))
    }
}

impl IntoResponse for PlatformError {
    fn into_response(self) -> Response {
        let (descriptor, message) = match &self {
            PlatformError::NotFound(m) => (
                code_for_message(StatusCode::NOT_FOUND, m),
                i18n::localize_error(StatusCode::NOT_FOUND, m),
            ),
            PlatformError::Unauthorized(m) => (
                code_for_message(StatusCode::UNAUTHORIZED, m),
                i18n::localize_error(StatusCode::UNAUTHORIZED, m),
            ),
            PlatformError::Forbidden(m) => (
                code_for_message(StatusCode::FORBIDDEN, m),
                i18n::localize_error(StatusCode::FORBIDDEN, m),
            ),
            PlatformError::BadRequest(m) => (
                code_for_message(StatusCode::BAD_REQUEST, m),
                i18n::localize_error(StatusCode::BAD_REQUEST, m),
            ),
            PlatformError::Conflict(m) => (
                code_for_message(StatusCode::CONFLICT, m),
                i18n::localize_error(StatusCode::CONFLICT, m),
            ),
            PlatformError::PermissionRequired(permission) => (
                descriptor("auth.permission_required", StatusCode::FORBIDDEN),
                i18n::localize_permission_required(permission),
            ),
            PlatformError::InvalidToken(details) => (
                descriptor("auth.invalid_token", StatusCode::UNAUTHORIZED),
                i18n::localize_invalid_token(details),
            ),
            PlatformError::ApproverNotMember(user_id) => (
                descriptor("release.approver_not_member", StatusCode::BAD_REQUEST),
                i18n::localize_approver_not_member(user_id),
            ),
            PlatformError::RequiresRoleOrHigher(role) => (
                descriptor("auth.role_insufficient", StatusCode::FORBIDDEN),
                i18n::localize_requires_role_or_higher(role),
            ),
            PlatformError::InvalidGitHubBase64(details) => (
                descriptor("github.invalid_base64_content", StatusCode::BAD_REQUEST),
                i18n::localize_invalid_github_base64(details),
            ),
            PlatformError::InvalidJsonFile { path, details } => (
                descriptor("github.invalid_json_file", StatusCode::BAD_REQUEST),
                i18n::localize_invalid_json_file(path, details),
            ),
            PlatformError::RepositoryFileNotFound(path) => (
                descriptor("github.repository_file_not_found", StatusCode::NOT_FOUND),
                i18n::localize_repository_file_not_found(path),
            ),
            PlatformError::TemplateSchemaInvalid(details) => (
                descriptor("template.invalid_schema", StatusCode::BAD_REQUEST),
                i18n::localize_template_schema_invalid(details),
            ),
            PlatformError::GitHubSearchFailed(details) => (
                descriptor("github.search_failed", StatusCode::BAD_REQUEST),
                i18n::localize_github_search_failed(details),
            ),
            PlatformError::GitHubRepositoryLookupFailed(details) => (
                descriptor("github.repository_lookup_failed", StatusCode::BAD_REQUEST),
                i18n::localize_github_repository_lookup_failed(details),
            ),
            PlatformError::InvalidRulesetPayload(details) => (
                descriptor("ruleset.invalid_payload", StatusCode::BAD_REQUEST),
                i18n::localize_invalid_ruleset_payload(details),
            ),
            PlatformError::InvalidRollbackPayload(details) => (
                descriptor("ruleset.invalid_rollback_payload", StatusCode::BAD_REQUEST),
                i18n::localize_invalid_rollback_payload(details),
            ),
            PlatformError::RuleSetByNameNotFound(name) => (
                descriptor("ruleset.not_found", StatusCode::NOT_FOUND),
                i18n::localize_ruleset_by_name_not_found(name),
            ),
            PlatformError::RuleSetVersionNotFound { name, seq } => (
                descriptor("ruleset.version_not_found", StatusCode::NOT_FOUND),
                i18n::localize_ruleset_version_not_found(name, *seq),
            ),
            PlatformError::Internal(e) => {
                tracing::error!("Internal platform error: {:#}", e);
                (
                    descriptor("common.internal_server_error", StatusCode::INTERNAL_SERVER_ERROR),
                    i18n::localize_error(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
                )
            }
        };
        (
            descriptor.status,
            Json(json!({ "code": descriptor.code, "error": message })),
        )
            .into_response()
    }
}

pub type ApiResult<T> = std::result::Result<T, PlatformError>;

fn descriptor(code: &'static str, status: StatusCode) -> ErrorDescriptor {
    ErrorDescriptor { code, status }
}

fn code_for_message(status: StatusCode, message: &str) -> ErrorDescriptor {
    let code = match (status, message.trim()) {
        (StatusCode::UNAUTHORIZED, "Missing Authorization header") => "auth.missing_authorization_header",
        (StatusCode::UNAUTHORIZED, "Invalid email or password") => "auth.invalid_credentials",
        (StatusCode::UNAUTHORIZED, "Current password is incorrect") => "auth.current_password_incorrect",

        (StatusCode::BAD_REQUEST, "Invalid email address") => "auth.invalid_email",
        (StatusCode::BAD_REQUEST, "Password must be at least 8 characters") => "auth.password_too_short",
        (StatusCode::BAD_REQUEST, "New password must be at least 8 characters") => "auth.new_password_too_short",
        (StatusCode::BAD_REQUEST, "Display name is required") => "user.display_name_required",
        (StatusCode::BAD_REQUEST, "Display name cannot be empty") => "user.display_name_empty",
        (StatusCode::BAD_REQUEST, "Organization name is required") => "org.name_required",
        (StatusCode::BAD_REQUEST, "Project name is required") => "project.name_required",
        (StatusCode::BAD_REQUEST, "Role name is required") => "role.name_required",
        (StatusCode::BAD_REQUEST, "Fact name is required") => "catalog.fact_name_required",
        (StatusCode::BAD_REQUEST, "Concept name is required") => "catalog.concept_name_required",
        (StatusCode::BAD_REQUEST, "Test case name is required") => "test.case_name_required",
        (StatusCode::BAD_REQUEST, "Environment name is required") => "environment.name_required",
        (StatusCode::BAD_REQUEST, "Name cannot be empty") => "common.name_empty",
        (StatusCode::BAD_REQUEST, "ruleset.config.name is required") => "ruleset.config_name_required",
        (StatusCode::BAD_REQUEST, "ruleset.config is required") => "ruleset.config_required",
        (StatusCode::BAD_REQUEST, "name and target_id are required") => "release.policy_name_and_target_required",
        (StatusCode::BAD_REQUEST, "template/meta.json must be a JSON object") => "template.meta_invalid_object",
        (StatusCode::BAD_REQUEST, "min_approvals must be at least 1") => "release.min_approvals_too_small",
        (StatusCode::BAD_REQUEST, "approver_ids must satisfy min_approvals") => "release.approver_count_insufficient",
        (StatusCode::BAD_REQUEST, "Project-targeted policy must target the current project") => "release.project_target_mismatch",
        (StatusCode::BAD_REQUEST, "Release policy does not define enough approvers for min_approvals") => "release.policy_approvers_insufficient",
        (StatusCode::BAD_REQUEST, "No release policy matched this project/environment target") => "release.policy_not_matched",
        (StatusCode::BAD_REQUEST, "ruleset_name, version, environment_id, title, and change_summary are required") => "release.request_fields_required",
        (StatusCode::BAD_REQUEST, "GitHub OAuth is not configured on this server") => "github.oauth_not_configured",
        (StatusCode::BAD_REQUEST, "GitHub repository lookup failed") => "github.repository_lookup_failed",
        (StatusCode::BAD_REQUEST, "Cannot change your own role") => "member.cannot_change_own_role",
        (StatusCode::BAD_REQUEST, "Rollback publish failed") => "release.rollback_publish_failed",
        (StatusCode::BAD_REQUEST, "Release request has no rollback version") => "release.rollback_version_missing",

        (StatusCode::FORBIDDEN, "Not a member of this organization") => "auth.org_membership_required",
        (StatusCode::FORBIDDEN, "Editor role required for write operations") => "auth.editor_role_required",
        (StatusCode::FORBIDDEN, "Requester cannot approve their own release request") => "release.self_approval_forbidden",
        (StatusCode::FORBIDDEN, "You are not an assigned approver for this release request") => "release.approver_not_assigned",

        (StatusCode::NOT_FOUND, "Not found") => "common.not_found",
        (StatusCode::NOT_FOUND, "User not found") => "user.not_found",
        (StatusCode::NOT_FOUND, "Organization not found") => "org.not_found",
        (StatusCode::NOT_FOUND, "Project not found") => "project.not_found",
        (StatusCode::NOT_FOUND, "Project not found or you are not a member of its organization") => "project.access_not_found",
        (StatusCode::NOT_FOUND, "Role not found") => "role.not_found",
        (StatusCode::NOT_FOUND, "Role not found in this org") => "role.not_found_in_org",
        (StatusCode::NOT_FOUND, "Ruleset not found") => "ruleset.not_found",
        (StatusCode::NOT_FOUND, "Draft ruleset not found") => "ruleset.draft_not_found",
        (StatusCode::NOT_FOUND, "Environment not found") => "environment.not_found",
        (StatusCode::NOT_FOUND, "Deployment not found") => "deployment.not_found",
        (StatusCode::NOT_FOUND, "Fact not found") => "catalog.fact_not_found",
        (StatusCode::NOT_FOUND, "Concept not found") => "catalog.concept_not_found",
        (StatusCode::NOT_FOUND, "Contract not found") => "contract.not_found",
        (StatusCode::NOT_FOUND, "Test case not found") => "test.case_not_found",
        (StatusCode::NOT_FOUND, "Template not found") => "template.not_found",
        (StatusCode::NOT_FOUND, "GitHub repository not found") => "github.repository_not_found",
        (StatusCode::NOT_FOUND, "No user with that email address") => "member.user_email_not_found",
        (StatusCode::NOT_FOUND, "Member not found") => "member.not_found",
        (StatusCode::NOT_FOUND, "Server not found") => "server.not_found",
        (StatusCode::NOT_FOUND, "Release policy not found") => "release.policy_not_found",
        (StatusCode::NOT_FOUND, "Release request not found") => "release.request_not_found",
        (StatusCode::NOT_FOUND, "Release execution not found") => "release.execution_not_found",
        (StatusCode::NOT_FOUND, "Rollback deployment snapshot not found") => "release.rollback_snapshot_not_found",

        (StatusCode::CONFLICT, "Email already registered") => "auth.email_already_registered",
        (StatusCode::CONFLICT, "User is already a member") => "member.already_exists",
        (StatusCode::CONFLICT, "Release request is not pending approval") => "release.request_not_pending_approval",
        (StatusCode::CONFLICT, "No pending approval found for this reviewer") => "release.pending_approval_not_found",
        (StatusCode::CONFLICT, "Release request must be approved before execution") => "release.request_not_approved",
        (StatusCode::CONFLICT, "Release execution is already rolling back") => "release.execution_already_rolling_back",
        (StatusCode::CONFLICT, "Release execution is not active") => "release.execution_not_active",
        (StatusCode::CONFLICT, "Release execution is not paused") => "release.execution_not_paused",
        (StatusCode::CONFLICT, "Release execution cannot be rolled back from its current status") => "release.execution_rollback_invalid_status",

        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error") => "common.internal_server_error",
        _ => match status {
            StatusCode::BAD_REQUEST => "common.bad_request",
            StatusCode::UNAUTHORIZED => "common.unauthorized",
            StatusCode::FORBIDDEN => "common.forbidden",
            StatusCode::NOT_FOUND => "common.not_found",
            StatusCode::CONFLICT => "common.conflict",
            _ => "common.internal_server_error",
        },
    };

    descriptor(code, status)
}
