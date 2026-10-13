//! Tests for the decision model gateway, against a mock System One upstream.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tower::ServiceExt;

use crate::{
    api,
    audit::AuditLogger,
    capability_registry::build_server_executor,
    debug::DebugSessionManager,
    decide::{self, DecideGateway},
    metrics::PrometheusMetricSink,
    middleware,
    rate_limiter::RateLimiter,
    store::RuleStore,
    tenant::{TenantDefaults, TenantManager},
    AppState, ServerConfig,
};

const REFUND_ROUTING: &str =
    include_str!("../../../examples/rule-packs/jev-routing/rulesets/refund-routing.json");

/// What the mock upstream saw: (authorization header, body) per call.
type Seen = Arc<Mutex<Vec<(Option<String>, Value)>>>;

async fn mock_systemone(
    State(seen): State<Seen>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    seen.lock().unwrap().push((auth, body.clone()));
    match body["state"].as_str() {
        Some("rate-limit") => (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "3")],
            Json(json!({"error": {"message": "slow down"}})),
        )
            .into_response(),
        Some("not-an-object") => Json(json!(["unexpected"])).into_response(),
        _ => {
            let confidence = body["state"]
                .as_str()
                .and_then(|s| s.strip_prefix("refund "))
                .and_then(|c| c.parse::<f64>().ok())
                .unwrap_or(0.95);
            Json(json!({
                "model": "jev-1.13.0",
                "answers": {
                    "intent": {
                        "choice": "refund",
                        "probabilities": {"refund": confidence, "other": 1.0 - confidence},
                        "confidence": confidence
                    },
                    "urgency": {"score": 0.3, "probabilities": [0.7, 0.3, 0.0], "confidence": 0.4},
                    "chargeback": {"noul": 0.02}
                }
            }))
            .into_response()
        }
    }
}

/// Start a mock upstream; returns its base URL and the record of calls.
async fn start_upstream() -> (String, Seen) {
    let seen: Seen = Arc::default();
    let app = Router::new()
        .route("/v1/systemone", post(mock_systemone))
        .route(
            "/v1/models",
            get(|| async { Json(json!({"data": [{"id": "jev-1.13.0"}]})) }),
        )
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), seen)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ordo-decide-{tag}-{}-{}",
        std::process::id(),
        rand::random::<u32>()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A config with the gateway pointed at `upstream`. `ServerConfig` has private
/// fields, so tests adjust a default instead of using struct-update syntax.
#[allow(clippy::field_reassign_with_default)]
fn gateway_config(upstream: String, tweak: impl FnOnce(&mut ServerConfig)) -> ServerConfig {
    let mut c = ServerConfig::default();
    c.decide_upstream = Some(upstream);
    tweak(&mut c);
    c
}

async fn build_app(config: ServerConfig) -> Router {
    let config = Arc::new(config);
    let metric_sink = Arc::new(PrometheusMetricSink::new());
    let audit_logger = Arc::new(AuditLogger::new(None, 10));
    let executor = build_server_executor(metric_sink.clone(), Some(audit_logger.clone()));
    let tenant_manager = Arc::new(
        TenantManager::new(
            None,
            TenantDefaults {
                default_qps_limit: None,
                default_burst_limit: None,
                default_timeout_ms: 100,
            },
        )
        .await
        .unwrap(),
    );
    tenant_manager.ensure_default("default").await.unwrap();
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let state = AppState {
        store: Arc::new(RwLock::new(RuleStore::new())),
        audit_logger,
        capture: Arc::new(crate::capture::CaptureLogger::new(None, 100)),
        metric_sink,
        executor,
        decide: DecideGateway::from_config(&config).unwrap().map(Arc::new),
        config,
        signature_verifier: None,
        debug_sessions: Arc::new(DebugSessionManager::new()),
        tenant_manager,
        rate_limiter: Arc::new(RateLimiter::new()),
        webhook_manager: crate::webhook::WebhookManager::new(shutdown_rx),
    };
    Router::new()
        .route("/api/v1/rulesets", post(api::create_ruleset))
        .route("/systemone/v1/systemone", post(decide::systemone))
        .route("/systemone/v1/models", get(decide::models))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::tenant::tenant_middleware,
        ))
        .with_state(state)
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    auth: Option<&str>,
    body: Option<&Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(auth) = auth {
        req = req.header("authorization", auth);
    }
    let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn log_lines(dir: &Path) -> Vec<Value> {
    let mut lines = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("decisions-"));
        for line in std::fs::read_to_string(path).unwrap().lines() {
            lines.push(serde_json::from_str(line).unwrap());
        }
    }
    lines
}

fn questions() -> Value {
    json!({
        "intent": {"type": "choice", "instructions": "What does the customer want?",
                   "criteria": {"refund": "Money back", "other": "Anything else"}},
        "urgency": {"type": "score", "instructions": "How urgent?",
                    "criteria": ["Routine", "Soon", "Stuck"]},
        "chargeback": {"type": "noul", "instructions": "Threatens a chargeback."}
    })
}

async fn upload_refund_routing(app: &Router) {
    let ruleset: Value = serde_json::from_str(REFUND_ROUTING).unwrap();
    let (status, _, body) = call(app, "POST", "/api/v1/rulesets", None, Some(&ruleset)).await;
    assert!(status.is_success(), "upload failed: {status} {body}");
}

#[tokio::test]
async fn passes_the_answer_through_and_logs_it() {
    let (upstream, seen) = start_upstream().await;
    let log_dir = temp_dir("pass");
    let app = build_app(gateway_config(upstream, |c| {
        c.decide_log_dir = Some(log_dir.clone());
    }))
    .await;

    let request =
        json!({"model": "jev-1.13", "state": "I was charged twice", "questions": questions()});
    let (status, _, body) = call(
        &app,
        "POST",
        "/systemone/v1/systemone",
        Some("Bearer caller-key"),
        Some(&request),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["model"], "jev-1.13.0");
    assert_eq!(body["answers"]["intent"]["choice"], "refund");
    assert!(body.get("ordo").is_none());

    let calls = seen.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0.as_deref(), Some("Bearer caller-key"));
    assert_eq!(calls[0].1, request);

    let lines = log_lines(&log_dir);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["model"], "jev-1.13.0");
    assert_eq!(lines[0]["upstream_status"], 200);
    assert_eq!(lines[0]["answers"]["chargeback"]["noul"], 0.02);
    // No ruleset ran, so there is no rule input (ordo impact skips the line),
    // and the customer text is not logged by default.
    assert!(lines[0]["input"].is_null());
    assert!(lines[0].get("request").is_none());

    let (status, _, models) = call(&app, "GET", "/systemone/v1/models", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(models["data"][0]["id"], "jev-1.13.0");
}

#[tokio::test]
async fn runs_the_ruleset_on_the_answers() {
    let (upstream, seen) = start_upstream().await;
    let log_dir = temp_dir("rules");
    let app = build_app(gateway_config(upstream, |c| {
        c.decide_log_dir = Some(log_dir.clone());
        c.decide_log_requests = true;
    }))
    .await;
    upload_refund_routing(&app).await;

    let request = json!({
        "model": "jev-1.13",
        "state": "refund 0.72",
        "questions": questions(),
        "ordo": {
            "ruleset": "refund-routing",
            "facts": {
                "order": {"amount": "129.00", "days_since_delivery": 1},
                "customer": {"refunds_90d": 0}
            }
        }
    });
    let (status, _, body) = call(
        &app,
        "POST",
        "/systemone/v1/systemone",
        None,
        Some(&request),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["answers"]["intent"]["confidence"], 0.72);
    assert_eq!(body["ordo"]["ruleset"], "refund-routing");
    assert_eq!(body["ordo"]["version"], "1.0.0");
    assert_eq!(body["ordo"]["code"], "LLM_DRAFT");
    assert_eq!(body["ordo"]["output"]["reason"], "medium_confidence");
    assert!(body["ordo"]["decision_id"]
        .as_str()
        .unwrap()
        .starts_with("dec_"));

    // The `ordo` field is Ordo's and never reaches the model.
    let calls = seen.lock().unwrap().clone();
    assert!(calls[0].1.get("ordo").is_none());

    let lines = log_lines(&log_dir);
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert_eq!(line["decision_id"], body["ordo"]["decision_id"]);
    assert_eq!(line["ruleset"], "refund-routing");
    assert_eq!(line["rule_version"], "1.0.0");
    assert_eq!(line["code"], "LLM_DRAFT");
    assert_eq!(line["input"]["jev"]["intent"]["confidence"], 0.72);
    assert_eq!(line["input"]["order"]["amount"], "129.00");
    assert_eq!(line["request"]["state"], "refund 0.72");

    // The same request with a higher confidence is refunded automatically.
    let mut sure = request.clone();
    sure["state"] = json!("refund 0.9");
    let (_, _, body) = call(&app, "POST", "/systemone/v1/systemone", None, Some(&sure)).await;
    assert_eq!(body["ordo"]["code"], "AUTO_REFUND");
}

#[tokio::test]
async fn upstream_errors_come_back_unchanged() {
    let (upstream, _) = start_upstream().await;
    let log_dir = temp_dir("err");
    let app = build_app(gateway_config(upstream, |c| {
        c.decide_log_dir = Some(log_dir.clone());
    }))
    .await;

    let request = json!({"model": "jev-1.13", "state": "rate-limit", "questions": questions()});
    let (status, headers, body) = call(
        &app,
        "POST",
        "/systemone/v1/systemone",
        None,
        Some(&request),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(headers.get("retry-after").unwrap(), "3");
    assert_eq!(body["error"]["message"], "slow down");

    let lines = log_lines(&log_dir);
    assert_eq!(lines[0]["upstream_status"], 429);
    assert!(lines[0]["input"].is_null());
}

#[tokio::test]
async fn an_answer_that_is_not_an_object_comes_back_unchanged() {
    let (upstream, _) = start_upstream().await;
    let app = build_app(gateway_config(upstream, |_| {})).await;
    upload_refund_routing(&app).await;

    let request = json!({
        "model": "jev-1.13",
        "state": "not-an-object",
        "questions": questions(),
        "ordo": {"ruleset": "refund-routing"}
    });
    let (status, _, body) = call(
        &app,
        "POST",
        "/systemone/v1/systemone",
        None,
        Some(&request),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!(["unexpected"]));
}

#[tokio::test]
async fn server_key_replaces_the_callers() {
    let (upstream, seen) = start_upstream().await;
    let app = build_app(gateway_config(format!("{upstream}/"), |c| {
        c.decide_api_key = Some("server-key".to_string());
    }))
    .await;

    let request = json!({"model": "jev-1.13", "state": "hi", "questions": questions()});
    let (status, _, _) = call(
        &app,
        "POST",
        "/systemone/v1/systemone",
        Some("Bearer caller-key"),
        Some(&request),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        seen.lock().unwrap()[0].0.as_deref(),
        Some("Bearer server-key")
    );
}

#[tokio::test]
async fn unknown_ruleset_is_rejected_before_the_model_is_asked() {
    let (upstream, seen) = start_upstream().await;
    let app = build_app(gateway_config(upstream, |_| {})).await;

    let request = json!({"state": "hi", "questions": questions(), "ordo": {"ruleset": "nope"}});
    let (status, _, body) = call(
        &app,
        "POST",
        "/systemone/v1/systemone",
        None,
        Some(&request),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "NOT_FOUND");
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_rule_error_keeps_the_answers() {
    let (upstream, _) = start_upstream().await;
    let app = build_app(gateway_config(upstream, |_| {})).await;
    upload_refund_routing(&app).await;

    // `order` is required by the ruleset's input schema.
    let request = json!({"state": "refund 0.9", "questions": questions(),
                         "ordo": {"ruleset": "refund-routing", "facts": {}}});
    let (status, _, body) = call(
        &app,
        "POST",
        "/systemone/v1/systemone",
        None,
        Some(&request),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["answers"]["intent"]["choice"], "refund");
    assert_eq!(body["ordo"]["error"]["code"], "INVALID_INPUT");
    assert!(body["ordo"].get("code").is_none());
}

#[test]
fn studio_format_rulesets_load_from_the_rules_dir() {
    let dir = temp_dir("studio");
    std::fs::write(dir.join("refund-routing.json"), REFUND_ROUTING).unwrap();
    let mut store = RuleStore::new_with_persistence(dir);
    assert_eq!(store.load_from_dir().unwrap(), 1);
    let ruleset = store.get("refund-routing").expect("loaded");
    assert_eq!(ruleset.config.entry_step, "escalation");
}
