//! Server registry: ordo-server instances register here and platform manages them.
//!
//! Internal endpoints (token-auth, called by ordo-server):
//!   POST /api/v1/internal/register   — register / re-register
//!   POST /api/v1/internal/heartbeat  — keep-alive (sets status=online)
//!
//! Protected endpoints (JWT-auth, called by platform UI/users):
//!   GET    /api/v1/servers                           — list all visible servers
//!   GET    /api/v1/servers/:id                       — server detail
//!   DELETE /api/v1/servers/:id                       — remove server (admin+)
//!   GET    /api/v1/servers/:id/metrics               — proxy to /metrics
//!   GET    /api/v1/servers/:id/health                — proxy to /health
//!   PUT    /api/v1/orgs/:oid/projects/:pid/server    — bind project to server

use crate::{
    error::{ApiResult, PlatformError},
    models::{derive_server_id, Claims, Role, ServerInfo, ServerNode, ServerStatus},
    org::load_org_and_check_role,
    proxy::find_project_membership,
    AppState,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    Extension, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

// ── Internal (token-auth) ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub server_id: String,
    pub name: String,
    pub url: String,
    pub token: String,
    pub version: Option<String>,
    /// Optional org to associate this server with
    pub org_id: Option<String>,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub id: String,
    pub status: String,
}

/// POST /api/v1/internal/register
///
/// Called by ordo-server on startup. Upserts the server entry by stable server_id.
pub async fn register_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterRequest>,
) -> ApiResult<Json<RegisterResponse>> {
    if let Some(required_secret) = state.config.registration_secret.as_deref() {
        let provided = headers
            .get("x-registration-secret")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if provided != required_secret {
            return Err(PlatformError::unauthorized("Invalid registration secret"));
        }
    }
    if req.server_id.is_empty() || req.token.is_empty() || req.url.is_empty() || req.name.is_empty() {
        return Err(PlatformError::bad_request(
            "server_id, name, url and token are required",
        ));
    }
    let derived_server_id = derive_server_id(&req.url).map_err(PlatformError::Internal)?;
    if derived_server_id != req.server_id {
        return Err(PlatformError::bad_request(
            "server_id does not match the normalized server url",
        ));
    }

    if let Some(existing) = state
        .store
        .find_server_by_token(&req.token)
        .await
        .map_err(PlatformError::Internal)?
    {
        if existing.id != req.server_id {
            return Err(PlatformError::conflict(
                "server token is already associated with another server",
            ));
        }
    }
    let existing = state
        .store
        .get_server(&req.server_id)
        .await
        .map_err(PlatformError::Internal)?;

    let server = ServerNode {
        id: req.server_id.clone(),
        name: req.name,
        url: req.url,
        token: req.token,
        org_id: req.org_id,
        labels: serde_json::Value::Object(Default::default()),
        version: req.version,
        status: ServerStatus::Online,
        last_seen: Some(Utc::now()),
        registered_at: existing.map(|s| s.registered_at).unwrap_or_else(Utc::now),
    };

    state
        .store
        .upsert_server(&server)
        .await
        .map_err(PlatformError::Internal)?;

    tracing::info!(server_id = %server.id, "Server registered");
    Ok(Json(RegisterResponse {
        id: server.id,
        status: "ok".into(),
    }))
}

#[derive(Deserialize)]
pub struct HeartbeatRequest {
    pub server_id: String,
}

/// POST /api/v1/internal/heartbeat
pub async fn server_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<HeartbeatRequest>,
) -> ApiResult<StatusCode> {
    if let Some(required_secret) = state.config.registration_secret.as_deref() {
        let provided = headers
            .get("x-registration-secret")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if provided != required_secret {
            return Err(PlatformError::unauthorized("Invalid registration secret"));
        }
    }
    state
        .store
        .update_server_heartbeat(&req.server_id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Protected (JWT-auth) ──────────────────────────────────────────────────────

/// GET /api/v1/servers — list all servers visible to the caller
pub async fn list_servers(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<Json<Vec<ServerInfo>>> {
    // Collect org IDs the user belongs to
    let orgs = state
        .store
        .list_user_orgs(&claims.sub)
        .await
        .map_err(PlatformError::Internal)?;

    let mut result = Vec::new();
    for org in &orgs {
        let servers = state
            .store
            .list_servers(Some(&org.id))
            .await
            .map_err(PlatformError::Internal)?;
        result.extend(servers.into_iter().map(ServerInfo::from));
    }
    // Also include global servers (no org_id)
    let global = state
        .store
        .list_servers(None)
        .await
        .map_err(PlatformError::Internal)?;
    for s in global {
        if s.org_id.is_none() && !result.iter().any(|r: &ServerInfo| r.id == s.id) {
            result.push(s.into());
        }
    }
    Ok(Json(result))
}

/// GET /api/v1/servers/:id
pub async fn get_server(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
) -> ApiResult<Json<ServerInfo>> {
    let server = state
        .store
        .get_server(&id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Server not found"))?;
    Ok(Json(server.into()))
}

/// DELETE /api/v1/servers/:id — requires admin in the server's org (or no org = global)
pub async fn delete_server(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let server = state
        .store
        .get_server(&id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Server not found"))?;

    if let Some(ref org_id) = server.org_id {
        load_org_and_check_role(&state, org_id, &claims.sub, Role::Admin).await?;
    }

    state
        .store
        .delete_server(&id)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/v1/servers/:id/metrics — proxy to server's Prometheus /metrics endpoint
pub async fn get_server_metrics(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let server = state
        .store
        .get_server(&id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Server not found"))?;

    let resp = state
        .http_client
        .get(format!("{}/metrics", server.url))
        .send()
        .await
        .map_err(|e| PlatformError::internal(format!("Server unreachable: {}", e)))?;

    let status = axum::http::StatusCode::from_u16(resp.status().as_u16())
        .unwrap_or(axum::http::StatusCode::BAD_GATEWAY);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("text/plain")
        .to_string();
    let body = resp
        .bytes()
        .await
        .map_err(|e| PlatformError::internal(format!("Failed to read metrics: {}", e)))?;

    Ok(Response::builder()
        .status(status)
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap())
}

/// GET /api/v1/servers/:id/health — proxy to server's /health endpoint
pub async fn get_server_health(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let server = state
        .store
        .get_server(&id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Server not found"))?;

    match state
        .http_client
        .get(format!("{}/health", server.url))
        .send()
        .await
    {
        Ok(resp) => {
            let ok = resp.status().is_success();
            let text = resp.text().await.unwrap_or_default();
            Ok(Json(
                serde_json::json!({ "online": ok, "response": text, "url": server.url }),
            ))
        }
        Err(e) => Ok(Json(
            serde_json::json!({ "online": false, "error": e.to_string(), "url": server.url }),
        )),
    }
}

// ── Project ↔ Server binding ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct BindServerRequest {
    /// Pass null to unbind
    pub server_id: Option<String>,
}

/// PUT /api/v1/orgs/:oid/projects/:pid/server
pub async fn bind_project_server(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((org_id, project_id)): Path<(String, String)>,
    Json(req): Json<BindServerRequest>,
) -> ApiResult<StatusCode> {
    // Verify caller has at least Admin role in this org
    load_org_and_check_role(&state, &org_id, &claims.sub, Role::Admin).await?;

    // Verify project exists in this org
    let _ = find_project_membership(&state, &project_id, &claims.sub).await?;

    // If server_id is provided, verify it exists
    if let Some(ref sid) = req.server_id {
        state
            .store
            .get_server(sid)
            .await
            .map_err(PlatformError::Internal)?
            .ok_or_else(|| PlatformError::not_found("Server not found"))?;
    }

    state
        .store
        .bind_project_server(&org_id, &project_id, req.server_id.as_deref())
        .await
        .map_err(PlatformError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}
