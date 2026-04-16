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
    models::{Claims, Role},
    AppState,
};
use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::Response,
    Extension,
};
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
    let (role, _org_id) = find_project_membership(&state, &project_id, &claims.sub).await?;

    // Write operations require editor+
    let method = req.method().clone();
    if is_write_method(&method) && role < Role::Editor {
        return Err(PlatformError::forbidden(
            "Editor role required for write operations",
        ));
    }

    // Build the upstream URL
    let engine_url = format!("{}/api/v1/{}", state.config.engine_url, rest);
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
async fn find_project_membership(
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
