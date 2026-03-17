//! Read-only enforcement middleware for reader instances.
//!
//! When the server runs with `--role reader`, all mutating HTTP methods
//! (`POST`, `PUT`, `DELETE`) on rule/tenant management paths are rejected
//! with `409 Conflict` and a JSON body pointing to the writer address.
//! Read and execute routes are unaffected.

use crate::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// HTTP middleware that rejects write operations on reader instances.
pub async fn read_only_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if !state.config.is_read_only() {
        return next.run(req).await;
    }

    if is_write_request(req.method(), req.uri().path()) {
        let writer = state.config.writer_addr.as_deref().unwrap_or("unknown");
        return (
            StatusCode::CONFLICT,
            axum::Json(serde_json::json!({
                "error": "read_only",
                "message": "This instance is a read-only reader. Mutations must be sent to the writer.",
                "writer": writer,
            })),
        )
            .into_response();
    }

    next.run(req).await
}

/// Returns `true` when the request is a mutation against management APIs.
/// Execute (`POST /api/v1/execute/...`) and read paths are always allowed.
fn is_write_request(method: &Method, path: &str) -> bool {
    match *method {
        Method::GET | Method::HEAD | Method::OPTIONS => return false,
        _ => {}
    }

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 3 || segments[0] != "api" || segments[1] != "v1" {
        return false;
    }

    match segments[2] {
        "rulesets" => true,
        "tenants" => true,
        "config" => true,
        "admin" => true,
        "execute" => false,
        "eval" => false,
        "debug" => false,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_is_never_blocked() {
        assert!(!is_write_request(&Method::GET, "/api/v1/rulesets"));
        assert!(!is_write_request(&Method::GET, "/api/v1/rulesets/my-rule"));
        assert!(!is_write_request(&Method::GET, "/api/v1/tenants"));
    }

    #[test]
    fn test_post_rulesets_is_write() {
        assert!(is_write_request(&Method::POST, "/api/v1/rulesets"));
    }

    #[test]
    fn test_put_ruleset_is_write() {
        assert!(is_write_request(&Method::PUT, "/api/v1/rulesets/my-rule"));
    }

    #[test]
    fn test_delete_ruleset_is_write() {
        assert!(is_write_request(
            &Method::DELETE,
            "/api/v1/rulesets/my-rule"
        ));
    }

    #[test]
    fn test_post_tenants_is_write() {
        assert!(is_write_request(&Method::POST, "/api/v1/tenants"));
        assert!(is_write_request(&Method::PUT, "/api/v1/tenants/t1"));
        assert!(is_write_request(&Method::DELETE, "/api/v1/tenants/t1"));
    }

    #[test]
    fn test_execute_is_allowed() {
        assert!(!is_write_request(&Method::POST, "/api/v1/execute/my-rule"));
        assert!(!is_write_request(
            &Method::POST,
            "/api/v1/execute/my-rule/batch"
        ));
    }

    #[test]
    fn test_eval_is_allowed() {
        assert!(!is_write_request(&Method::POST, "/api/v1/eval"));
    }

    #[test]
    fn test_health_is_allowed() {
        assert!(!is_write_request(&Method::GET, "/health"));
        assert!(!is_write_request(&Method::GET, "/metrics"));
    }

    #[test]
    fn test_config_write_is_blocked() {
        assert!(is_write_request(
            &Method::PUT,
            "/api/v1/config/audit-sample-rate"
        ));
    }

    #[test]
    fn test_admin_reload_is_blocked() {
        assert!(is_write_request(&Method::POST, "/api/v1/admin/reload"));
    }
}
