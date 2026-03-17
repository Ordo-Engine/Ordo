use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tower::ServiceExt;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::trace::TraceLayer;

use crate::{
    api,
    audit::AuditLogger,
    debug::DebugSessionManager,
    metrics::PrometheusMetricSink,
    middleware,
    rate_limiter::RateLimiter,
    readiness_check,
    store::RuleStore,
    tenant::{TenantDefaults, TenantManager},
    AppState, ServerConfig,
};
use ordo_core::prelude::RuleExecutor;

async fn build_test_app() -> Router {
    let store = Arc::new(RwLock::new(RuleStore::new()));
    let executor = Arc::new(RuleExecutor::new());
    let metric_sink = Arc::new(PrometheusMetricSink::new());
    let audit_logger = Arc::new(AuditLogger::new(None, 0));
    let debug_sessions = Arc::new(DebugSessionManager::new());
    let defaults = TenantDefaults {
        default_qps_limit: None,
        default_burst_limit: None,
        default_timeout_ms: 100,
    };
    let tenant_manager = Arc::new(TenantManager::new(None, defaults).await.unwrap());
    tenant_manager.ensure_default("default").await.unwrap();
    let rate_limiter = Arc::new(RateLimiter::new());
    let config = Arc::new(ServerConfig::default());
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let webhook_manager = crate::webhook::WebhookManager::new(shutdown_rx);

    let state = AppState {
        store,
        audit_logger,
        metric_sink,
        executor,
        config,
        signature_verifier: None,
        debug_sessions,
        tenant_manager,
        rate_limiter,
        webhook_manager,
    };

    Router::new()
        .route("/health", get(readiness_check))
        .route(
            "/api/v1/rulesets",
            get(api::list_rulesets).post(api::create_ruleset),
        )
        .route(
            "/api/v1/rulesets/:name",
            get(api::get_ruleset).delete(api::delete_ruleset),
        )
        .route("/api/v1/execute/:name", post(api::execute_ruleset))
        .route(
            "/api/v1/execute/:name/batch",
            post(api::execute_ruleset_batch),
        )
        .route("/api/v1/eval", post(api::eval_expression))
        .route("/api/v1/admin/reload", post(api::admin_reload))
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::tenant::tenant_middleware,
        ))
        .layer(CatchPanicLayer::new())
        .with_state(state)
}

/// A minimal two-step ruleset: decision → terminal(HIGH) or terminal(LOW)
fn test_ruleset(name: &str) -> Value {
    json!({
        "config": {
            "name": name,
            "entry_step": "decide"
        },
        "steps": {
            "decide": {
                "id": "decide",
                "name": "Decide",
                "type": "decision",
                "branches": [
                    { "condition": "value > 50", "next_step": "high" }
                ],
                "default_next": "low"
            },
            "high": {
                "id": "high",
                "name": "High",
                "type": "terminal",
                "result": { "code": "HIGH", "message": "High value" }
            },
            "low": {
                "id": "low",
                "name": "Low",
                "type": "terminal",
                "result": { "code": "LOW", "message": "Low value" }
            }
        }
    })
}

async fn get_request(app: &Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn post_json(app: &Router, uri: &str, payload: &Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn delete_request(app: &Router, uri: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

// ==================== Tests ====================

#[tokio::test]
async fn test_health_check() {
    let app = build_test_app().await;
    let (status, body) = get_request(&app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn test_create_ruleset_returns_201() {
    let app = build_test_app().await;
    let (status, body) = post_json(&app, "/api/v1/rulesets", &test_ruleset("create_test")).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["status"], "created");
    assert_eq!(body["name"], "create_test");
}

#[tokio::test]
async fn test_update_existing_ruleset_returns_200() {
    let app = build_test_app().await;
    post_json(&app, "/api/v1/rulesets", &test_ruleset("update_test")).await;
    let (status, body) = post_json(&app, "/api/v1/rulesets", &test_ruleset("update_test")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "updated");
}

#[tokio::test]
async fn test_get_ruleset() {
    let app = build_test_app().await;
    post_json(&app, "/api/v1/rulesets", &test_ruleset("get_test")).await;
    let (status, body) = get_request(&app, "/api/v1/rulesets/get_test").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["config"]["name"], "get_test");
}

#[tokio::test]
async fn test_get_nonexistent_ruleset_returns_404() {
    let app = build_test_app().await;
    let (status, _) = get_request(&app, "/api/v1/rulesets/does_not_exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_rulesets() {
    let app = build_test_app().await;
    post_json(&app, "/api/v1/rulesets", &test_ruleset("list_test")).await;
    let (status, body) = get_request(&app, "/api/v1/rulesets").await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert!(arr.iter().any(|r| r["name"] == "list_test"));
}

#[tokio::test]
async fn test_delete_ruleset() {
    let app = build_test_app().await;
    post_json(&app, "/api/v1/rulesets", &test_ruleset("del_test")).await;

    let status = delete_request(&app, "/api/v1/rulesets/del_test").await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Confirm it is gone
    let (status, _) = get_request(&app, "/api/v1/rulesets/del_test").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_nonexistent_ruleset_returns_404() {
    let app = build_test_app().await;
    let status = delete_request(&app, "/api/v1/rulesets/ghost").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_execute_high_branch() {
    let app = build_test_app().await;
    post_json(&app, "/api/v1/rulesets", &test_ruleset("exec_test")).await;
    let (status, body) = post_json(
        &app,
        "/api/v1/execute/exec_test",
        &json!({ "input": { "value": 75 } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["code"], "HIGH");
}

#[tokio::test]
async fn test_execute_low_branch() {
    let app = build_test_app().await;
    post_json(&app, "/api/v1/rulesets", &test_ruleset("exec_low")).await;
    let (status, body) = post_json(
        &app,
        "/api/v1/execute/exec_low",
        &json!({ "input": { "value": 10 } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["code"], "LOW");
}

#[tokio::test]
async fn test_execute_nonexistent_ruleset_returns_404() {
    let app = build_test_app().await;
    let (status, _) = post_json(&app, "/api/v1/execute/ghost_rule", &json!({ "input": {} })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_batch_execute() {
    let app = build_test_app().await;
    post_json(&app, "/api/v1/rulesets", &test_ruleset("batch_test")).await;

    let (status, body) = post_json(
        &app,
        "/api/v1/execute/batch_test/batch",
        &json!({
            "inputs": [
                { "value": 75 },
                { "value": 10 },
                { "value": 100 }
            ],
            "options": { "parallel": false }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let summary = &body["summary"];
    assert_eq!(summary["total"], 3);
    assert_eq!(summary["success"], 3);
    assert_eq!(summary["failed"], 0);

    let results = body["results"].as_array().unwrap();
    assert_eq!(results[0]["code"], "HIGH");
    assert_eq!(results[1]["code"], "LOW");
    assert_eq!(results[2]["code"], "HIGH");
}

#[tokio::test]
async fn test_batch_empty_inputs_returns_400() {
    let app = build_test_app().await;
    post_json(&app, "/api/v1/rulesets", &test_ruleset("batch_empty")).await;
    let (status, _) = post_json(
        &app,
        "/api/v1/execute/batch_empty/batch",
        &json!({ "inputs": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_eval_expression() {
    let app = build_test_app().await;
    let (status, body) = post_json(
        &app,
        "/api/v1/eval",
        &json!({
            "expression": "age > 18",
            "context": { "age": 25 }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], true);
}

#[tokio::test]
async fn test_eval_expression_false_result() {
    let app = build_test_app().await;
    let (status, body) = post_json(
        &app,
        "/api/v1/eval",
        &json!({
            "expression": "age > 18",
            "context": { "age": 15 }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], false);
}

#[tokio::test]
async fn test_admin_reload_no_persistence() {
    let app = build_test_app().await;
    let (status, body) = post_json(&app, "/api/v1/admin/reload", &json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().unwrap().contains("rules-dir"));
}

#[tokio::test]
async fn test_admin_reload_with_persistence() {
    let dir = tempfile::tempdir().unwrap();
    // Write a rule file
    let rule = test_ruleset("reload-test");
    let path = dir.path().join("reload-test.json");
    std::fs::write(&path, serde_json::to_string_pretty(&rule).unwrap()).unwrap();

    let store = Arc::new(RwLock::new(RuleStore::new_with_persistence(
        dir.path().to_path_buf(),
    )));
    let executor = Arc::new(RuleExecutor::new());
    let metric_sink = Arc::new(PrometheusMetricSink::new());
    let audit_logger = Arc::new(AuditLogger::new(None, 0));
    let debug_sessions = Arc::new(DebugSessionManager::new());
    let defaults = TenantDefaults {
        default_qps_limit: None,
        default_burst_limit: None,
        default_timeout_ms: 100,
    };
    let tenant_manager = Arc::new(TenantManager::new(None, defaults).await.unwrap());
    tenant_manager.ensure_default("default").await.unwrap();
    let rate_limiter = Arc::new(RateLimiter::new());
    let mut config = ServerConfig::default();
    config.rules_dir = Some(dir.path().to_path_buf());
    let config = Arc::new(config);
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let webhook_manager = crate::webhook::WebhookManager::new(shutdown_rx);

    let state = AppState {
        store,
        audit_logger,
        metric_sink,
        executor,
        config,
        signature_verifier: None,
        debug_sessions,
        tenant_manager,
        rate_limiter,
        webhook_manager,
    };

    let app = Router::new()
        .route("/api/v1/admin/reload", post(api::admin_reload))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::tenant::tenant_middleware,
        ))
        .with_state(state);

    let (status, body) = post_json(&app, "/api/v1/admin/reload", &json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "reloaded");
    assert_eq!(body["rules_loaded"], 1);
}
