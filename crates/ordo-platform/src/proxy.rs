//! Authenticated proxy to ordo-server engine API.
//!
//! All requests to `/api/v1/engine/*` are:
//!   1. JWT-authenticated (via require_auth middleware)
//!   2. Role-checked against the project's org membership
//!   3. Forwarded to ordo-server with X-Tenant-ID set to the project ID
//!
//! URL mapping:
//!   /api/v1/engine/{project_id}/{rest...} → {engine_url}/api/v1/{rest...}
//!   with X-Tenant-ID: {project_id}

use crate::{
    error::{ApiResult, PlatformError},
    models::{Claims, Role, RulesetHistorySource},
    ruleset_history::append_history_entry_for_actor,
    sync::SyncEvent,
    AppState,
};
use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::Response,
    Extension,
};
use serde_json::json;
use std::str::FromStr;

/// Handler for ANY /api/v1/engine/:project_id/*path
///
/// Validates:
/// - Caller is a member of an org that owns the project
/// - For write operations (POST/PUT/DELETE): caller has editor+ role
pub async fn proxy_engine(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, rest)): Path<(String, String)>,
    req: Request<Body>,
) -> Result<Response, PlatformError> {
    // Find the org that owns this project and verify membership
    let (role, org_id) = find_project_membership(&state, &project_id, &claims.sub).await?;

    // Write operations require editor+
    let method = req.method().clone();
    if is_write_method(&method) && role < Role::Editor {
        return Err(PlatformError::forbidden(
            "Editor role required for write operations",
        ));
    }

    let sync_write = state
        .sync_publisher
        .as_ref()
        .and_then(|_| parse_syncable_ruleset_write(&method, &rest));

    // Resolve engine base URL: use project-bound server if set, else default
    let base_url = resolve_engine_url(&state, &project_id, &org_id).await;
    let engine_url = format!("{}/api/v1/{}", base_url, rest);
    let engine_url = if let Some(query) = req.uri().query() {
        format!("{}?{}", engine_url, query)
    } else {
        engine_url
    };

    // Forward request headers (excluding host), add X-Tenant-ID
    let mut forward_headers = HeaderMap::new();
    for (name, value) in req.headers() {
        let name_str = name.as_str().to_lowercase();
        if name_str != "host" && name_str != "authorization" {
            forward_headers.insert(name.clone(), value.clone());
        }
    }
    forward_headers.insert(
        HeaderName::from_static("x-tenant-id"),
        HeaderValue::from_str(&project_id)
            .map_err(|_| PlatformError::internal("Invalid project ID"))?,
    );

    // Read body
    let body_bytes = axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024)
        .await
        .map_err(|e| PlatformError::internal(format!("Failed to read body: {}", e)))?;

    if let Some(sync_write) = sync_write {
        return handle_sync_ruleset_write(
            &state,
            &base_url,
            &project_id,
            &org_id,
            &claims,
            sync_write,
            &body_bytes,
        )
        .await;
    }

    // Forward to engine
    let upstream = state
        .http_client
        .request(
            reqwest::Method::from_str(method.as_str())
                .map_err(|_| PlatformError::internal("Invalid HTTP method"))?,
            &engine_url,
        )
        .headers(to_reqwest_headers(&forward_headers))
        .body(body_bytes.to_vec())
        .send()
        .await
        .map_err(|e| PlatformError::internal(format!("Engine unreachable: {}", e)))?;

    // Stream response back
    let status = StatusCode::from_u16(upstream.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response_headers = HeaderMap::new();
    for (name, value) in upstream.headers() {
        response_headers.insert(name.clone(), value.clone());
    }

    let body = upstream
        .bytes()
        .await
        .map_err(|e| PlatformError::internal(format!("Failed to read engine response: {}", e)))?;

    let mut resp = Response::new(Body::from(body));
    *resp.status_mut() = status;
    *resp.headers_mut() = response_headers;
    Ok(resp)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Find which org owns the project and return the caller's role in that org.
pub async fn find_project_membership(
    state: &AppState,
    project_id: &str,
    user_id: &str,
) -> ApiResult<(Role, String)> {
    let orgs = state
        .store
        .list_user_orgs(user_id)
        .await
        .map_err(PlatformError::Internal)?;

    for org in &orgs {
        if state
            .store
            .get_project(&org.id, project_id)
            .await
            .map_err(PlatformError::Internal)?
            .is_some()
        {
            let role = org
                .members
                .iter()
                .find(|m| m.user_id == user_id)
                .map(|m| m.role)
                .unwrap_or(Role::Viewer);
            return Ok((role, org.id.clone()));
        }
    }

    Err(PlatformError::not_found(
        "Project not found or you are not a member of its organization",
    ))
}

fn is_write_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::DELETE | Method::PATCH
    )
}

/// Resolve the engine base URL for a project, applying canary traffic splitting.
///
/// Resolution order:
/// 1. Look up the project's default environment.
/// 2. If the default env has a canary config and `rand < canary_percentage`, route to the canary env.
/// 3. Otherwise route to the default env's server.
/// 4. Fall back to the platform's configured `engine_url`.
async fn resolve_engine_url(state: &AppState, project_id: &str, _org_id: &str) -> String {
    let Ok(Some(prod_env)) = state.store.get_default_environment(project_id).await else {
        // No environments configured yet — use legacy project.server_id lookup or default
        return state.config.engine_url.clone();
    };

    // Canary check
    if prod_env.canary_percentage > 0 {
        if let Some(ref canary_env_id) = prod_env.canary_target_env_id {
            let roll = (rand::random::<u8>() % 100) as i32;
            if roll < prod_env.canary_percentage {
                if let Ok(Some(canary_env)) =
                    state.store.get_environment(project_id, canary_env_id).await
                {
                    if let Some(url) = resolve_server_url(state, &canary_env).await {
                        tracing::debug!(
                            project = %project_id,
                            canary_env = %canary_env.name,
                            roll,
                            "Canary: routing to canary environment"
                        );
                        return url;
                    }
                }
            }
        }
    }

    resolve_server_url(state, &prod_env)
        .await
        .unwrap_or_else(|| state.config.engine_url.clone())
}

async fn resolve_server_url(
    state: &AppState,
    env: &crate::models::ProjectEnvironment,
) -> Option<String> {
    if let Some(ref sid) = env.server_id {
        if let Ok(Some(server)) = state.store.get_server(sid).await {
            return Some(server.url);
        }
    }
    None
}

enum SyncableRulesetWrite {
    Upsert,
    Delete { name: String },
    Rollback { name: String },
}

fn parse_syncable_ruleset_write(method: &Method, rest: &str) -> Option<SyncableRulesetWrite> {
    match *method {
        Method::POST if rest == "rulesets" => Some(SyncableRulesetWrite::Upsert),
        Method::POST => {
            let name = rest.strip_prefix("rulesets/")?.strip_suffix("/rollback")?;
            if name.is_empty() || name.contains('/') {
                return None;
            }
            Some(SyncableRulesetWrite::Rollback {
                name: name.to_string(),
            })
        }
        Method::DELETE => {
            let name = rest.strip_prefix("rulesets/")?;
            if name.is_empty() || name.contains('/') {
                return None;
            }
            Some(SyncableRulesetWrite::Delete {
                name: name.to_string(),
            })
        }
        _ => None,
    }
}

async fn handle_sync_ruleset_write(
    state: &AppState,
    base_url: &str,
    project_id: &str,
    org_id: &str,
    claims: &Claims,
    op: SyncableRulesetWrite,
    body_bytes: &[u8],
) -> Result<Response, PlatformError> {
    let publisher = state
        .sync_publisher
        .as_ref()
        .ok_or_else(|| PlatformError::internal("NATS publisher is not configured"))?;

    match op {
        SyncableRulesetWrite::Upsert => {
            let mut payload: serde_json::Value =
                serde_json::from_slice(body_bytes)
                    .map_err(|e| PlatformError::invalid_ruleset_payload(e.to_string()))?;
            let config = payload
                .get_mut("config")
                .and_then(|value| value.as_object_mut())
                .ok_or_else(|| PlatformError::bad_request("ruleset.config is required"))?;

            let name = config
                .get("name")
                .and_then(|value| value.as_str())
                .ok_or_else(|| PlatformError::bad_request("ruleset.config.name is required"))?
                .to_string();
            let version = config
                .get("version")
                .and_then(|value| value.as_str())
                .unwrap_or("0.0.0")
                .to_string();

            config.insert(
                "tenant_id".to_string(),
                serde_json::Value::String(project_id.to_string()),
            );

            let existed = ruleset_exists_on_engine(state, base_url, project_id, &name).await;

            publisher
                .publish(SyncEvent::RulePut {
                    tenant_id: project_id.to_string(),
                    name: name.clone(),
                    ruleset_json: serde_json::to_string(&payload).map_err(|e| {
                        PlatformError::internal(format!("Failed to serialize ruleset: {}", e))
                    })?,
                    version,
                })
                .await
                .map_err(|e| {
                    PlatformError::internal(format!(
                        "Failed to publish ruleset update to NATS: {}",
                        e
                    ))
                })?;

            if let Err(err) = append_history_entry_for_actor(
                state,
                org_id,
                project_id,
                &name,
                if matches!(existed, Some(false)) {
                    RulesetHistorySource::Create
                } else {
                    RulesetHistorySource::Save
                },
                if matches!(existed, Some(false)) {
                    format!("Created ruleset '{}'", name)
                } else {
                    format!("Saved ruleset '{}'", name)
                },
                payload,
                &claims.sub,
                &claims.email,
            )
            .await
            {
                tracing::warn!(
                    error = %err,
                    ruleset = %name,
                    tenant_id = %project_id,
                    "Ruleset was published but history append failed"
                );
            }

            let (status, result_status) = match existed {
                Some(true) => (StatusCode::OK, "updated"),
                Some(false) => (StatusCode::CREATED, "created"),
                None => (StatusCode::ACCEPTED, "accepted"),
            };

            let body = Body::from(
                serde_json::to_vec(&json!({
                    "status": result_status,
                    "name": name,
                }))
                .map_err(|e| {
                    PlatformError::internal(format!("Failed to encode response: {}", e))
                })?,
            );

            let mut resp = Response::new(body);
            *resp.status_mut() = status;
            resp.headers_mut().insert(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("application/json"),
            );
            Ok(resp)
        }
        SyncableRulesetWrite::Delete { name } => {
            let existed = ruleset_exists_on_engine(state, base_url, project_id, &name).await;
            if matches!(existed, Some(false)) {
                return Err(PlatformError::ruleset_by_name_not_found(name));
            }

            publisher
                .publish(SyncEvent::RuleDeleted {
                    tenant_id: project_id.to_string(),
                    name,
                })
                .await
                .map_err(|e| {
                    PlatformError::internal(format!(
                        "Failed to publish ruleset deletion to NATS: {}",
                        e
                    ))
                })?;

            Ok(Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty())
                .unwrap())
        }
        SyncableRulesetWrite::Rollback { name } => {
            let request: RollbackRequest = serde_json::from_slice(body_bytes)
                .map_err(|e| PlatformError::invalid_rollback_payload(e.to_string()))?;
            let current_ruleset =
                fetch_ruleset_from_engine(state, base_url, project_id, &name).await?;
            let rollback_ruleset =
                fetch_ruleset_version_from_engine(state, base_url, project_id, &name, request.seq)
                    .await?;

            let from_version = current_ruleset
                .get("config")
                .and_then(|config| config.get("version"))
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            let to_version = rollback_ruleset
                .get("config")
                .and_then(|config| config.get("version"))
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();

            publisher
                .publish(SyncEvent::RulePut {
                    tenant_id: project_id.to_string(),
                    name: name.clone(),
                    ruleset_json: serde_json::to_string(&rollback_ruleset).map_err(|e| {
                        PlatformError::internal(format!(
                            "Failed to serialize rollback ruleset: {}",
                            e
                        ))
                    })?,
                    version: to_version.clone(),
                })
                .await
                .map_err(|e| {
                    PlatformError::internal(format!("Failed to publish rollback to NATS: {}", e))
                })?;

            if let Err(err) = append_history_entry_for_actor(
                state,
                org_id,
                project_id,
                &name,
                RulesetHistorySource::Restore,
                format!("Rolled back '{}' to version #{}", name, request.seq),
                rollback_ruleset,
                &claims.sub,
                &claims.email,
            )
            .await
            {
                tracing::warn!(
                    error = %err,
                    ruleset = %name,
                    tenant_id = %project_id,
                    seq = request.seq,
                    "Rollback was published but history append failed"
                );
            }

            let body = Body::from(
                serde_json::to_vec(&json!({
                    "status": "rolled_back",
                    "name": name,
                    "from_version": from_version,
                    "to_version": to_version,
                }))
                .map_err(|e| {
                    PlatformError::internal(format!("Failed to encode response: {}", e))
                })?,
            );

            let mut resp = Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert(
                HeaderName::from_static("content-type"),
                HeaderValue::from_static("application/json"),
            );
            Ok(resp)
        }
    }
}

#[derive(serde::Deserialize)]
struct RollbackRequest {
    seq: u32,
}

async fn fetch_ruleset_from_engine(
    state: &AppState,
    base_url: &str,
    project_id: &str,
    name: &str,
) -> Result<serde_json::Value, PlatformError> {
    let url = format!("{}/api/v1/rulesets/{}", base_url, name);
    fetch_ruleset_json(state, project_id, &url)
        .await
        .map_err(|err| match err {
            PlatformError::NotFound(_) => PlatformError::ruleset_by_name_not_found(name),
            other => other,
        })
}

async fn fetch_ruleset_version_from_engine(
    state: &AppState,
    base_url: &str,
    project_id: &str,
    name: &str,
    seq: u32,
) -> Result<serde_json::Value, PlatformError> {
    let url = format!("{}/api/v1/rulesets/{}/versions/{}", base_url, name, seq);
    fetch_ruleset_json(state, project_id, &url)
        .await
        .map_err(|err| match err {
            PlatformError::NotFound(_) => PlatformError::ruleset_version_not_found(name, seq),
            other => other,
        })
}

async fn fetch_ruleset_json(
    state: &AppState,
    project_id: &str,
    url: &str,
) -> Result<serde_json::Value, PlatformError> {
    let resp = state
        .http_client
        .get(url)
        .header("X-Tenant-ID", project_id)
        .send()
        .await
        .map_err(|e| PlatformError::internal(format!("Engine unreachable: {}", e)))?;

    match resp.status() {
        status if status.is_success() => resp.json::<serde_json::Value>().await.map_err(|e| {
            PlatformError::internal(format!("Failed to decode engine response: {}", e))
        }),
        reqwest::StatusCode::NOT_FOUND => Err(PlatformError::not_found("Not found")),
        status => {
            let body = resp.text().await.unwrap_or_default();
            Err(PlatformError::internal(format!(
                "Engine request failed: HTTP {} — {}",
                status, body
            )))
        }
    }
}

async fn ruleset_exists_on_engine(
    state: &AppState,
    base_url: &str,
    project_id: &str,
    name: &str,
) -> Option<bool> {
    let url = format!("{}/api/v1/rulesets/{}", base_url, name);
    match state
        .http_client
        .get(&url)
        .header("X-Tenant-ID", project_id)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => Some(true),
        Ok(resp) if resp.status() == reqwest::StatusCode::NOT_FOUND => Some(false),
        Ok(resp) => {
            tracing::warn!(
                status = %resp.status(),
                ruleset = %name,
                tenant_id = %project_id,
                "Engine existence check returned unexpected status; proceeding with NATS publish"
            );
            None
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                ruleset = %name,
                tenant_id = %project_id,
                "Engine existence check failed; proceeding with NATS publish"
            );
            None
        }
    }
}

fn to_reqwest_headers(headers: &HeaderMap) -> reqwest::header::HeaderMap {
    let mut map = reqwest::header::HeaderMap::new();
    for (name, value) in headers {
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_str(name.as_str()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            map.insert(name, value);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_syncable_ruleset_write_supports_upsert_delete_and_rollback() {
        assert!(matches!(
            parse_syncable_ruleset_write(&Method::POST, "rulesets"),
            Some(SyncableRulesetWrite::Upsert)
        ));
        assert!(matches!(
            parse_syncable_ruleset_write(&Method::DELETE, "rulesets/fraud-check"),
            Some(SyncableRulesetWrite::Delete { name }) if name == "fraud-check"
        ));
        assert!(matches!(
            parse_syncable_ruleset_write(&Method::POST, "rulesets/fraud-check/rollback"),
            Some(SyncableRulesetWrite::Rollback { name }) if name == "fraud-check"
        ));
    }

    #[test]
    fn parse_syncable_ruleset_write_rejects_nested_names() {
        assert!(parse_syncable_ruleset_write(&Method::DELETE, "rulesets/foo/bar").is_none());
        assert!(parse_syncable_ruleset_write(&Method::POST, "rulesets/foo/bar/rollback").is_none());
    }
}
