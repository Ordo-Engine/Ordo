//! JWT authentication middleware and role-based access guards

use crate::{auth::verify_token, i18n, AppState};
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Extract and validate JWT from Authorization header.
/// Inserts `Claims` as request extension on success.
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let token = match extract_bearer(&req) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": i18n::localize_auth_missing_header()})),
            )
                .into_response()
        }
    };

    match verify_token(token, &state.config.jwt_secret) {
        Ok(claims) => {
            req.extensions_mut().insert(claims);
            next.run(req).await
        }
        Err(e) => e.into_response(),
    }
}

fn extract_bearer(req: &Request<Body>) -> Option<&str> {
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}
