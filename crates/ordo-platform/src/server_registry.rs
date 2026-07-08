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
//!   GET    /api/v1/servers/:id/metrics               — request metrics over NATS
//!   GET    /api/v1/servers/:id/health                — request health over NATS
//!   PUT    /api/v1/orgs/:oid/projects/:pid/server    — bind project to server

use crate::{
    error::{ApiResult, PlatformError},
    models::{
        derive_server_id, Claims, ConnectTokenInfo, Role, ServerInfo, ServerNode, ServerStatus,
    },
    org::load_org_and_check_role,
    proxy::find_project_membership,
    rbac::{require_permission, PERM_SERVER_MANAGE, PERM_SERVER_VIEW},
    sync, AppState,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    Extension, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

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
    #[serde(default)]
    pub capabilities: serde_json::Value,
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
    // Resolve the connect token (if any) to its owning org. A valid connect
    // token both authorizes registration and scopes the engine to that org.
    let connect_org = match headers.get("x-connect-token").and_then(|v| v.to_str().ok()) {
        Some(tok) if !tok.is_empty() => Some(
            state
                .store
                .org_for_connect_token(tok)
                .await
                .map_err(PlatformError::Internal)?
                .ok_or_else(|| PlatformError::unauthorized("Invalid connect token"))?,
        ),
        _ => None,
    };
    // Without a connect token, fall back to the global registration secret.
    if connect_org.is_none() {
        if let Some(required_secret) = state.config.registration_secret.as_deref() {
            let provided = headers
                .get("x-registration-secret")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if provided != required_secret {
                return Err(PlatformError::unauthorized(
                    "Invalid registration secret or connect token",
                ));
            }
        }
    }
    if req.server_id.is_empty() || req.token.is_empty() || req.url.is_empty() || req.name.is_empty()
    {
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
        org_id: connect_org.or(req.org_id),
        labels: serde_json::Value::Object(Default::default()),
        version: req.version,
        status: ServerStatus::Online,
        last_seen: Some(Utc::now()),
        registered_at: existing.map(|s| s.registered_at).unwrap_or_else(Utc::now),
        capabilities: req.capabilities,
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

    // Only servers scoped to an org the caller belongs to. Unassigned (global)
    // servers are intentionally NOT surfaced here — that leaked every engine
    // into every org. They are managed/assigned via the connect-token flow.
    let mut result = Vec::new();
    for org in &orgs {
        let servers = state
            .store
            .list_servers(Some(&org.id))
            .await
            .map_err(PlatformError::Internal)?;
        result.extend(servers.into_iter().map(ServerInfo::from));
    }
    Ok(Json(result))
}

// ── org connect tokens (admin) ──

#[derive(Deserialize)]
pub struct CreateConnectTokenRequest {
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Serialize)]
pub struct CreateConnectTokenResponse {
    pub id: String,
    /// The raw token — shown once, on creation, and never again.
    pub token: String,
    pub label: Option<String>,
}

/// POST /api/v1/orgs/:oid/connect-tokens — mint a connect token for the org.
pub async fn create_connect_token(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(oid): Path<String>,
    Json(req): Json<CreateConnectTokenRequest>,
) -> ApiResult<Json<CreateConnectTokenResponse>> {
    require_permission(&state, &oid, &claims.sub, PERM_SERVER_MANAGE).await?;
    let id = Uuid::new_v4().to_string();
    let token = format!("ordo_connect_{}", Uuid::new_v4().simple());
    state
        .store
        .create_connect_token(&id, &oid, &token, req.label.as_deref(), &claims.sub)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(CreateConnectTokenResponse {
        id,
        token,
        label: req.label,
    }))
}

/// GET /api/v1/orgs/:oid/connect-tokens — list token metadata (never the raw token).
pub async fn list_connect_tokens(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(oid): Path<String>,
) -> ApiResult<Json<Vec<ConnectTokenInfo>>> {
    require_permission(&state, &oid, &claims.sub, PERM_SERVER_VIEW).await?;
    let tokens = state
        .store
        .list_connect_tokens(&oid)
        .await
        .map_err(PlatformError::Internal)?;
    Ok(Json(tokens))
}

/// DELETE /api/v1/orgs/:oid/connect-tokens/:id — revoke a connect token.
pub async fn delete_connect_token(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((oid, id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    require_permission(&state, &oid, &claims.sub, PERM_SERVER_MANAGE).await?;
    let deleted = state
        .store
        .delete_connect_token(&oid, &id)
        .await
        .map_err(PlatformError::Internal)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(PlatformError::not_found("Connect token not found"))
    }
}

/// Authorize a read of `server` by `user_id`. `server_id` is derived from the
/// engine URL (`derive_server_id` = sha256 of the normalized URL), so it is
/// guessable — a bare `get_server(id)` with no org check leaks another org's
/// engine (url, version, capabilities) and, for the metrics/health variants,
/// lets any authenticated user trigger a control-plane RPC to it. An org-owned
/// server therefore requires the caller to be a member of that org; a global
/// (unowned) server stays readable by any authenticated user, matching how
/// `delete_server` gates only when an org owns the server.
async fn authorize_server_read(
    state: &AppState,
    server: &ServerNode,
    user_id: &str,
) -> ApiResult<()> {
    if let Some(org_id) = &server.org_id {
        load_org_and_check_role(state, org_id, user_id, Role::Viewer).await?;
    }
    Ok(())
}

/// GET /api/v1/servers/:id
pub async fn get_server(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> ApiResult<Json<ServerInfo>> {
    let server = state
        .store
        .get_server(&id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Server not found"))?;
    authorize_server_read(&state, &server, &claims.sub).await?;
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

const SERVER_RPC_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
struct ServerRpcResponse {
    status: u16,
    content_type: Option<String>,
    body: serde_json::Value,
    error: Option<String>,
}

async fn request_server_rpc(
    state: &AppState,
    server: &ServerNode,
    endpoint: &str,
) -> ApiResult<ServerRpcResponse> {
    let client = state.nats_client.as_ref().ok_or_else(|| {
        PlatformError::internal("NATS is not configured for server control requests")
    })?;
    let subject = sync::server_rpc_subject(&state.config.nats_subject_prefix, &server.id, endpoint);
    let response = tokio::time::timeout(
        SERVER_RPC_TIMEOUT,
        client.request(subject.clone(), Vec::new().into()),
    )
    .await
    .map_err(|_| PlatformError::internal(format!("NATS request timed out: {}", subject)))?
    .map_err(|e| PlatformError::internal(format!("NATS request failed: {}", e)))?;

    serde_json::from_slice(&response.payload)
        .map_err(|e| PlatformError::internal(format!("Invalid NATS response: {}", e)))
}

fn rpc_body_as_text(body: &serde_json::Value) -> String {
    match body {
        serde_json::Value::String(value) => value.clone(),
        value => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}

/// GET /api/v1/servers/:id/metrics — request server Prometheus metrics over NATS
pub async fn get_server_metrics(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let server = state
        .store
        .get_server(&id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Server not found"))?;
    authorize_server_read(&state, &server, &claims.sub).await?;

    let rpc = request_server_rpc(&state, &server, "metrics").await?;
    let status =
        axum::http::StatusCode::from_u16(rpc.status).unwrap_or(axum::http::StatusCode::BAD_GATEWAY);
    let content_type = rpc.content_type.unwrap_or_else(|| "text/plain".to_string());
    let body = rpc.error.unwrap_or_else(|| rpc_body_as_text(&rpc.body));

    Ok(Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .unwrap())
}

/// GET /api/v1/servers/:id/health — request server health over NATS
pub async fn get_server_health(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let server = state
        .store
        .get_server(&id)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Server not found"))?;
    authorize_server_read(&state, &server, &claims.sub).await?;

    match request_server_rpc(&state, &server, "health").await {
        Ok(rpc) => {
            let online = (200..300).contains(&rpc.status);
            let mut payload = serde_json::json!({
                "online": online,
                "response": rpc_body_as_text(&rpc.body),
                "url": server.url,
                "transport": "nats",
            });
            if let Some(error) = rpc.error {
                payload["error"] = serde_json::Value::String(error);
            }
            Ok(Json(payload))
        }
        Err(e) => Ok(Json(serde_json::json!({
            "online": false,
            "error": e.to_string(),
            "url": server.url,
            "transport": "nats",
        }))),
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

    // If a server is provided, verify it exists AND belongs to this org — a
    // project may only bind to an engine registered to its own organization.
    if let Some(ref sid) = req.server_id {
        let server = state
            .store
            .get_server(sid)
            .await
            .map_err(PlatformError::Internal)?
            .ok_or_else(|| PlatformError::not_found("Server not found"))?;
        if server.org_id.as_deref() != Some(org_id.as_str()) {
            return Err(PlatformError::bad_request(
                "server is not registered to this organization",
            ));
        }
    }

    state
        .store
        .bind_project_server(&org_id, &project_id, req.server_id.as_deref())
        .await
        .map_err(PlatformError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}
