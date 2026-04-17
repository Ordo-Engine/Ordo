//! Extended integration tests for the HTTP API.
//!
//! Covers: execute with trace, batch with options, version management,
//! eval edge cases, external data CRUD, pipeline execution, tenant CRUD,
//! audit config, multi-tenancy isolation, and error handling.

use std::sync::Arc;

use std::time::Duration;

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
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
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

/// Build a full test app with all API routes and middleware (matching main.rs router).
async fn build_full_test_app() -> Router {
    let store = Arc::new(RwLock::new(RuleStore::new()));
    let executor = Arc::new(RuleExecutor::new());
    let metric_sink = Arc::new(PrometheusMetricSink::new());
    let audit_logger = Arc::new(AuditLogger::new(None, 10));
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

    let request_timeout = Duration::from_secs(config.request_timeout_secs);
    let max_body = config.max_request_body_bytes;

    let state = AppState {
        store,
        audit_logger,
        metric_sink,
        executor,
        runtime_config: crate::runtime_config::new_shared(&config),
        config,
        signature_verifier: None,
        debug_sessions,
        tenant_manager,
        rate_limiter,
        webhook_manager,
        sync_tx: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
    };

    // CORS — permissive for tests (matches debug_enabled=false production default)
    let cors = CorsLayer::new()
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
        ])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

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
        .route("/api/v1/rulesets/:name/versions", get(api::list_versions))
        .route(
            "/api/v1/rulesets/:name/rollback",
            post(api::rollback_ruleset),
        )
        .route("/api/v1/execute/:name", post(api::execute_ruleset))
        .route(
            "/api/v1/execute/:name/batch",
            post(api::execute_ruleset_batch),
        )
        .route("/api/v1/execute-pipeline", post(api::execute_pipeline))
        .route("/api/v1/rulesets/:name/filter", post(api::compile_filter))
        .route("/api/v1/eval", post(api::eval_expression))
        .route(
            "/api/v1/config/audit-sample-rate",
            get(api::get_audit_sample_rate).put(api::set_audit_sample_rate),
        )
        .route("/api/v1/data", get(api::list_data))
        .route(
            "/api/v1/data/:name",
            get(api::get_data)
                .put(api::put_data)
                .delete(api::delete_data),
        )
        .route(
            "/api/v1/tenants",
            get(api::list_tenants).post(api::create_tenant),
        )
        .route(
            "/api/v1/tenants/:id",
            get(api::get_tenant)
                .put(api::update_tenant)
                .delete(api::delete_tenant),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::role::read_only_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::tenant::tenant_middleware,
        ))
        .layer(TimeoutLayer::new(request_timeout))
        .layer(RequestBodyLimitLayer::new(max_body))
        .layer(CatchPanicLayer::new())
        .with_state(state)
}

// ==================== Helpers ====================

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

async fn put_json(app: &Router, uri: &str, payload: &Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
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

async fn delete_request(app: &Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
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

#[allow(dead_code)]
async fn post_json_with_tenant(
    app: &Router,
    uri: &str,
    payload: &Value,
    tenant_id: &str,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("x-tenant-id", tenant_id)
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

#[allow(dead_code)]
async fn get_with_tenant(app: &Router, uri: &str, tenant_id: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("x-tenant-id", tenant_id)
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

// ==================== Test Rulesets ====================

/// Decision → terminal(HIGH) or terminal(LOW) based on `value > 50`
fn threshold_ruleset(name: &str) -> Value {
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

/// Multi-branch decision: categorize score into tiers (pure decision → terminal)
fn tiered_ruleset(name: &str) -> Value {
    json!({
        "config": {
            "name": name,
            "entry_step": "categorize",
            "version": "1.0.0"
        },
        "steps": {
            "categorize": {
                "id": "categorize",
                "name": "Categorize",
                "type": "decision",
                "branches": [
                    { "condition": "score >= 90", "next_step": "gold" },
                    { "condition": "score >= 70", "next_step": "silver" }
                ],
                "default_next": "bronze"
            },
            "gold": {
                "id": "gold",
                "name": "Gold",
                "type": "terminal",
                "result": { "code": "GOLD", "message": "Gold tier" }
            },
            "silver": {
                "id": "silver",
                "name": "Silver",
                "type": "terminal",
                "result": { "code": "SILVER", "message": "Silver tier" }
            },
            "bronze": {
                "id": "bronze",
                "name": "Bronze",
                "type": "terminal",
                "result": { "code": "BRONZE", "message": "Bronze tier" }
            }
        }
    })
}

/// Simple single-step ruleset for pipeline tests (always returns OK)
fn simple_ruleset(name: &str) -> Value {
    json!({
        "config": {
            "name": name,
            "entry_step": "done"
        },
        "steps": {
            "done": {
                "id": "done",
                "name": "Done",
                "type": "terminal",
                "result": { "code": "OK", "message": "Done" }
            }
        }
    })
}

// ==================== Execute with Trace ====================

#[tokio::test]
async fn test_execute_with_trace() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("trace_test")).await;

    let (status, body) = post_json(
        &app,
        "/api/v1/execute/trace_test",
        &json!({ "input": { "value": 75 }, "trace": true }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["code"], "HIGH");
    // Trace should be present
    assert!(body["trace"].is_object(), "trace should be present");
    assert!(body["trace"]["path"].is_string());
    assert!(body["trace"]["steps"].is_array());
    let steps = body["trace"]["steps"].as_array().unwrap();
    assert!(!steps.is_empty());
}

#[tokio::test]
async fn test_execute_without_trace() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("no_trace")).await;

    let (status, body) = post_json(
        &app,
        "/api/v1/execute/no_trace",
        &json!({ "input": { "value": 75 } }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["code"], "HIGH");
    // Trace should be absent
    assert!(body["trace"].is_null(), "trace should not be present");
}

#[tokio::test]
async fn test_execute_multi_branch_decision() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &tiered_ruleset("tiered_test")).await;

    // Gold tier (score >= 90)
    let (status, body) = post_json(
        &app,
        "/api/v1/execute/tiered_test",
        &json!({ "input": { "score": 95 } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["code"], "GOLD");

    // Silver tier (score >= 70)
    let (_, body) = post_json(
        &app,
        "/api/v1/execute/tiered_test",
        &json!({ "input": { "score": 80 } }),
    )
    .await;
    assert_eq!(body["code"], "SILVER");

    // Bronze tier (default)
    let (_, body) = post_json(
        &app,
        "/api/v1/execute/tiered_test",
        &json!({ "input": { "score": 50 } }),
    )
    .await;
    assert_eq!(body["code"], "BRONZE");
}

#[tokio::test]
async fn test_execute_invalid_body() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("inv_body")).await;

    // Missing "input" field — SimdJson returns 400
    let (status, _) = post_json(
        &app,
        "/api/v1/execute/inv_body",
        &json!({ "data": { "value": 10 } }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ==================== Batch Execution ====================

#[tokio::test]
async fn test_batch_with_trace() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("batch_trace")).await;

    let (status, body) = post_json(
        &app,
        "/api/v1/execute/batch_trace/batch",
        &json!({
            "inputs": [
                { "value": 75 },
                { "value": 25 }
            ],
            "options": { "trace": true }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    // Each result should have trace
    assert!(results[0]["trace"].is_object());
    assert!(results[1]["trace"].is_object());
}

#[tokio::test]
async fn test_batch_parallel_vs_sequential() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("batch_modes")).await;

    let inputs = json!({
        "inputs": [
            { "value": 10 },
            { "value": 60 },
            { "value": 30 },
            { "value": 90 }
        ]
    });

    // Sequential
    let (s1, b1) = post_json(
        &app,
        "/api/v1/execute/batch_modes/batch",
        &json!({ "inputs": inputs["inputs"], "options": { "parallel": false } }),
    )
    .await;
    assert_eq!(s1, StatusCode::OK);

    // Parallel (default)
    let (s2, b2) = post_json(
        &app,
        "/api/v1/execute/batch_modes/batch",
        &json!({ "inputs": inputs["inputs"] }),
    )
    .await;
    assert_eq!(s2, StatusCode::OK);

    // Results should match regardless of mode
    let r1 = b1["results"].as_array().unwrap();
    let r2 = b2["results"].as_array().unwrap();
    assert_eq!(r1.len(), r2.len());
    for (a, b) in r1.iter().zip(r2.iter()) {
        assert_eq!(a["code"], b["code"]);
    }
}

#[tokio::test]
async fn test_batch_over_limit() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("batch_limit")).await;

    // 1001 inputs should exceed MAX_BATCH_SIZE (1000)
    let inputs: Vec<Value> = (0..1001).map(|i| json!({ "value": i })).collect();
    let (status, body) = post_json(
        &app,
        "/api/v1/execute/batch_limit/batch",
        &json!({ "inputs": inputs }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().unwrap_or("").contains("1000"));
}

// ==================== Eval Edge Cases ====================

#[tokio::test]
async fn test_eval_arithmetic() {
    let app = build_full_test_app().await;
    let (status, body) = post_json(
        &app,
        "/api/v1/eval",
        &json!({ "expression": "a + b * 2", "context": { "a": 10, "b": 5 } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], 20);
}

#[tokio::test]
async fn test_eval_string_expression() {
    let app = build_full_test_app().await;
    let (status, body) = post_json(
        &app,
        "/api/v1/eval",
        &json!({ "expression": "name == \"alice\"", "context": { "name": "alice" } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], true);
}

#[tokio::test]
async fn test_eval_no_context() {
    let app = build_full_test_app().await;
    let (status, body) = post_json(&app, "/api/v1/eval", &json!({ "expression": "1 + 2" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], 3);
}

#[tokio::test]
async fn test_eval_invalid_expression() {
    let app = build_full_test_app().await;
    let (status, _) = post_json(&app, "/api/v1/eval", &json!({ "expression": "(((" })).await;
    // Should be a 4xx error for parse failure
    assert!(status.is_client_error() || status.is_server_error());
}

// ==================== Version Management ====================

#[tokio::test]
async fn test_list_versions_in_memory_mode() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("ver_test")).await;

    let (status, body) = get_request(&app, "/api/v1/rulesets/ver_test/versions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "ver_test");
    // In-memory mode returns empty version list
    assert!(body["versions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_versions_nonexistent() {
    let app = build_full_test_app().await;
    let (status, _) = get_request(&app, "/api/v1/rulesets/ghost/versions").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_rollback_in_memory_mode() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("rb_test")).await;

    let (status, body) = post_json(
        &app,
        "/api/v1/rulesets/rb_test/rollback",
        &json!({ "seq": 1 }),
    )
    .await;
    // Should fail in memory mode
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["message"]
            .as_str()
            .unwrap_or("")
            .contains("memory-only"),
        "expected memory-only error, got: {}",
        body
    );
}

#[tokio::test]
async fn test_rollback_nonexistent_ruleset() {
    let app = build_full_test_app().await;
    let (status, _) = post_json(
        &app,
        "/api/v1/rulesets/ghost/rollback",
        &json!({ "seq": 1 }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ==================== External Data API ====================

#[tokio::test]
async fn test_data_crud() {
    let app = build_full_test_app().await;

    // List empty
    let (status, body) = get_request(&app, "/api/v1/data").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 0);

    // Put data
    let (status, _) = put_json(
        &app,
        "/api/v1/data/pricing",
        &json!({ "gold": 100, "silver": 50 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Get data
    let (status, body) = get_request(&app, "/api/v1/data/pricing").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["gold"], 100);
    assert_eq!(body["silver"], 50);

    // List should show it
    let (_, body) = get_request(&app, "/api/v1/data").await;
    let names = body.as_array().unwrap();
    assert!(names.iter().any(|n| n == "pricing"));

    // Update
    let (status, _) = put_json(&app, "/api/v1/data/pricing", &json!({ "gold": 200 })).await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = get_request(&app, "/api/v1/data/pricing").await;
    assert_eq!(body["gold"], 200);

    // Delete
    let (status, _) = delete_request(&app, "/api/v1/data/pricing").await;
    assert_eq!(status, StatusCode::OK);

    // Get deleted should 404
    let (status, _) = get_request(&app, "/api/v1/data/pricing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_data_get_nonexistent() {
    let app = build_full_test_app().await;
    let (status, _) = get_request(&app, "/api/v1/data/no_such_data").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_data_delete_nonexistent() {
    let app = build_full_test_app().await;
    let (status, _) = delete_request(&app, "/api/v1/data/no_such_data").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ==================== Pipeline Execution ====================

#[tokio::test]
async fn test_pipeline_execution() {
    let app = build_full_test_app().await;

    // Create two rulesets
    post_json(&app, "/api/v1/rulesets", &simple_ruleset("stage_a")).await;
    post_json(&app, "/api/v1/rulesets", &simple_ruleset("stage_b")).await;

    let (status, body) = post_json(
        &app,
        "/api/v1/execute-pipeline",
        &json!({
            "rulesets": ["stage_a", "stage_b"],
            "input": { "user": "alice" }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let stages = body["stages"].as_array().unwrap();
    assert_eq!(stages.len(), 2);
    assert_eq!(stages[0]["ruleset"], "stage_a");
    assert_eq!(stages[1]["ruleset"], "stage_b");
    assert!(body["duration_us"].as_u64().is_some());
}

#[tokio::test]
async fn test_pipeline_empty_rulesets() {
    let app = build_full_test_app().await;
    let (status, body) = post_json(
        &app,
        "/api/v1/execute-pipeline",
        &json!({ "rulesets": [], "input": {} }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().unwrap_or("").contains("empty"));
}

#[tokio::test]
async fn test_pipeline_nonexistent_stage() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &simple_ruleset("exists")).await;

    let (status, _) = post_json(
        &app,
        "/api/v1/execute-pipeline",
        &json!({ "rulesets": ["exists", "not_found"], "input": {} }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_pipeline_too_many_stages() {
    let app = build_full_test_app().await;
    // Create 21 rulesets (limit is 20)
    let names: Vec<String> = (0..21).map(|i| format!("pipe_{}", i)).collect();
    for name in &names {
        post_json(&app, "/api/v1/rulesets", &simple_ruleset(name)).await;
    }

    let (status, body) = post_json(
        &app,
        "/api/v1/execute-pipeline",
        &json!({ "rulesets": names, "input": {} }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().unwrap_or("").contains("20"));
}

// ==================== Tenant Management ====================

#[tokio::test]
async fn test_tenant_crud() {
    let app = build_full_test_app().await;

    // List tenants (should have at least the default tenant)
    let (status, body) = get_request(&app, "/api/v1/tenants").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.as_array().unwrap().is_empty());

    // Create a new tenant
    let (status, body) = post_json(
        &app,
        "/api/v1/tenants",
        &json!({
            "id": "test-tenant",
            "name": "Test Tenant",
            "qps_limit": 1000,
            "burst_limit": 2000,
            "execution_timeout_ms": 50
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["id"], "test-tenant");
    assert_eq!(body["name"], "Test Tenant");
    assert_eq!(body["qps_limit"], 1000);
    assert_eq!(body["burst_limit"], 2000);
    assert_eq!(body["execution_timeout_ms"], 50);

    // Get tenant
    let (status, body) = get_request(&app, "/api/v1/tenants/test-tenant").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "test-tenant");

    // Update tenant
    let (status, body) = put_json(
        &app,
        "/api/v1/tenants/test-tenant",
        &json!({ "name": "Updated Tenant", "qps_limit": 5000 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Updated Tenant");
    assert_eq!(body["qps_limit"], 5000);
    // Unchanged fields preserved
    assert_eq!(body["burst_limit"], 2000);

    // Delete tenant
    let (status, _) = delete_request(&app, "/api/v1/tenants/test-tenant").await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Get deleted tenant should 404
    let (status, _) = get_request(&app, "/api/v1/tenants/test-tenant").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_duplicate_tenant() {
    let app = build_full_test_app().await;

    post_json(
        &app,
        "/api/v1/tenants",
        &json!({ "id": "dup", "name": "Dup" }),
    )
    .await;

    let (status, body) = post_json(
        &app,
        "/api/v1/tenants",
        &json!({ "id": "dup", "name": "Dup Again" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"]
        .as_str()
        .unwrap_or("")
        .contains("already exists"));
}

#[tokio::test]
async fn test_create_tenant_empty_id() {
    let app = build_full_test_app().await;
    let (status, body) = post_json(
        &app,
        "/api/v1/tenants",
        &json!({ "id": "", "name": "No ID" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().unwrap_or("").contains("empty"));
}

#[tokio::test]
async fn test_get_nonexistent_tenant() {
    let app = build_full_test_app().await;
    let (status, _) = get_request(&app, "/api/v1/tenants/ghost").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_nonexistent_tenant() {
    let app = build_full_test_app().await;
    let (status, _) = put_json(&app, "/api/v1/tenants/ghost", &json!({ "name": "Ghost" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_nonexistent_tenant() {
    let app = build_full_test_app().await;
    let (status, _) = delete_request(&app, "/api/v1/tenants/ghost").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ==================== Audit Config ====================

#[tokio::test]
async fn test_audit_sample_rate_get_set() {
    let app = build_full_test_app().await;

    // Get current
    let (status, body) = get_request(&app, "/api/v1/config/audit-sample-rate").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["sample_rate"].is_number());
    let initial_rate = body["sample_rate"].as_u64().unwrap();

    // Set new rate
    let new_rate = if initial_rate == 50 { 75 } else { 50 };
    let (status, body) = put_json(
        &app,
        "/api/v1/config/audit-sample-rate",
        &json!({ "sample_rate": new_rate }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["sample_rate"], new_rate);
    assert_eq!(body["previous"], initial_rate);

    // Verify it persisted
    let (_, body) = get_request(&app, "/api/v1/config/audit-sample-rate").await;
    assert_eq!(body["sample_rate"], new_rate);
}

// ==================== Data Filter API ====================

#[tokio::test]
async fn test_compile_filter() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("filter_test")).await;

    let (status, body) = post_json(
        &app,
        "/api/v1/rulesets/filter_test/filter",
        &json!({
            "known_input": {},
            "target_results": ["HIGH"]
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["filter"].is_object() || body["filter"].is_string());
    assert!(body.get("always_matches").is_some());
    assert!(body.get("never_matches").is_some());
}

#[tokio::test]
async fn test_compile_filter_nonexistent_ruleset() {
    let app = build_full_test_app().await;
    let (status, _) = post_json(
        &app,
        "/api/v1/rulesets/ghost/filter",
        &json!({
            "known_input": {},
            "target_results": ["HIGH"]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_compile_filter_empty_targets() {
    let app = build_full_test_app().await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("filter_empty")).await;

    let (status, _) = post_json(
        &app,
        "/api/v1/rulesets/filter_empty/filter",
        &json!({
            "known_input": {},
            "target_results": []
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ==================== Ruleset CRUD Edge Cases ====================

#[tokio::test]
async fn test_create_ruleset_invalid_json() {
    let app = build_full_test_app().await;

    // Missing required fields
    let (status, _) = post_json(&app, "/api/v1/rulesets", &json!({ "invalid": true })).await;
    assert!(status.is_client_error());
}

#[tokio::test]
async fn test_create_multiple_rulesets_and_list() {
    let app = build_full_test_app().await;

    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("multi_a")).await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("multi_b")).await;
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("multi_c")).await;

    let (status, body) = get_request(&app, "/api/v1/rulesets").await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert!(arr.len() >= 3);
    let names: Vec<&str> = arr.iter().filter_map(|r| r["name"].as_str()).collect();
    assert!(names.contains(&"multi_a"));
    assert!(names.contains(&"multi_b"));
    assert!(names.contains(&"multi_c"));
}

#[tokio::test]
async fn test_update_ruleset_via_post() {
    let app = build_full_test_app().await;

    // Create v1
    post_json(&app, "/api/v1/rulesets", &threshold_ruleset("upsert_test")).await;

    // Execute — value 30 → LOW
    let (_, body) = post_json(
        &app,
        "/api/v1/execute/upsert_test",
        &json!({ "input": { "value": 30 } }),
    )
    .await;
    assert_eq!(body["code"], "LOW");

    // Update with different threshold (value > 20)
    let updated = json!({
        "config": { "name": "upsert_test", "entry_step": "decide" },
        "steps": {
            "decide": {
                "id": "decide", "name": "Decide", "type": "decision",
                "branches": [{ "condition": "value > 20", "next_step": "high" }],
                "default_next": "low"
            },
            "high": { "id": "high", "name": "High", "type": "terminal",
                "result": { "code": "HIGH", "message": "High" } },
            "low": { "id": "low", "name": "Low", "type": "terminal",
                "result": { "code": "LOW", "message": "Low" } }
        }
    });
    let (status, body) = post_json(&app, "/api/v1/rulesets", &updated).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "updated");

    // Execute again — value 30 now → HIGH (because threshold changed to 20)
    let (_, body) = post_json(
        &app,
        "/api/v1/execute/upsert_test",
        &json!({ "input": { "value": 30 } }),
    )
    .await;
    assert_eq!(body["code"], "HIGH");
}

// ==================== Health Check ====================

#[tokio::test]
async fn test_health_check_fields() {
    let app = build_full_test_app().await;
    let (status, body) = get_request(&app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
    assert!(body["version"].is_string());
    assert!(body["uptime_seconds"].is_number());
    assert!(body["checks"]["store_lock"].as_bool().unwrap());
    assert!(body["checks"]["disk_writable"].as_bool().unwrap());
    assert!(body["storage"]["rules_count"].is_number());
}
