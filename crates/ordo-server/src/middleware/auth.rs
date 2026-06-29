//! Optional shared-secret (bearer token) auth for the HTTP data API.
//!
//! This is **opt-in**: when [`ServerConfig::api_token`](crate::config::ServerConfig)
//! is unset (the default) the middleware is never installed and every request
//! passes through, preserving the historical no-auth behaviour. When a token is
//! configured, requests to the data routes must present a matching
//! `Authorization: Bearer <token>` or `X-API-Token: <token>` header.
//!
//! Health (`/healthz*`, `/health`) and `/metrics` are intentionally *not* gated
//! — the middleware is only layered onto the data routes in `main.rs`.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::AppState;

const HEADER_API_TOKEN: &str = "x-api-token";

/// Constant-time byte comparison to avoid leaking the token via timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Extract the presented token from either `Authorization: Bearer <token>`
/// or the `X-API-Token` header.
fn presented_token(req: &Request<Body>) -> Option<&str> {
    if let Some(bearer) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(str::trim)
    {
        return Some(bearer);
    }

    req.headers()
        .get(HEADER_API_TOKEN)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
}

/// Reject requests that do not carry the configured shared secret.
///
/// Only installed when `config.api_token` is `Some`, so the `None` branch here
/// is just defensive.
pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let Some(expected) = state.config.api_token.as_deref() else {
        return next.run(req).await;
    };

    match presented_token(&req) {
        Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => {
            next.run(req).await
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "code": "UNAUTHORIZED",
                "message": "Missing or invalid API token",
            })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secrte"));
        assert!(!constant_time_eq(b"secret", b"secret-longer"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }
}
