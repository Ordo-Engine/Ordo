//! Decision model gateway.
//!
//! A System One compatible endpoint (`POST /systemone/v1/systemone`) in front
//! of Jev or any model that speaks the same API. Point an SDK's base URL at
//! `http://<ordo>/systemone` and every call goes through Ordo:
//!
//! - The request is forwarded to the configured upstream and the answer is
//!   returned unchanged, including error statuses and `retry-after`, so the
//!   caller's error handling keeps working.
//! - A request may carry an extra `ordo` field, `{ "ruleset": "...", "facts":
//!   {...} }`. Ordo then runs that ruleset on the facts plus the model's
//!   answers (under `jev` by default) and adds the decision to the response
//!   as `ordo`. The model judges, the rules decide.
//! - Each call is written to the decision log, one JSON line with the model,
//!   its answers and, when a ruleset ran, the rule input and the decision.
//!   `ordo impact --inputs` reads those lines, so a threshold change can be
//!   checked against real requests before it ships.
//!
//! The rule engine itself never calls out: the model is asked first, and the
//! engine runs on the recorded answer, so a decision can be replayed exactly.

use std::time::{Duration, Instant};

use axum::{
    body::{Body, Bytes},
    extract::{Extension, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use ordo_core::prelude::Value;
use ordo_core::rule::ExecutionOptions;
use serde::Deserialize;
use serde_json::{json, Map, Value as JsonValue};

use crate::capture::CaptureLogger;
use crate::config::ServerConfig;
use crate::error::ApiError;
use crate::metrics;
use crate::middleware::tenant::TenantContext;
use crate::AppState;

/// Field the model's answers are placed under in the rule input, unless the
/// request names another one.
const DEFAULT_ANSWERS_FIELD: &str = "jev";

/// Upstream client, configuration and decision log for the gateway.
pub struct DecideGateway {
    client: reqwest::Client,
    decide_url: String,
    models_url: String,
    api_key: Option<String>,
    log: CaptureLogger,
    log_requests: bool,
}

impl DecideGateway {
    /// Build the gateway when `--decide-upstream` is set.
    pub fn from_config(config: &ServerConfig) -> anyhow::Result<Option<Self>> {
        let Some(upstream) = config.decide_upstream.as_deref() else {
            return Ok(None);
        };
        let base = upstream.trim_end_matches('/');
        // Leave a second for Ordo's own response inside the request timeout.
        let timeout = Duration::from_secs(config.request_timeout_secs.saturating_sub(1).max(1));
        let client = reqwest::Client::builder().timeout(timeout).build()?;
        Ok(Some(Self {
            client,
            decide_url: format!("{base}/v1/systemone"),
            models_url: format!("{base}/v1/models"),
            api_key: config.decide_api_key.clone(),
            log: CaptureLogger::with_prefix(config.decide_log_dir.clone(), 100, "decisions"),
            log_requests: config.decide_log_requests,
        }))
    }

    fn authorize(
        &self,
        req: reqwest::RequestBuilder,
        headers: &HeaderMap,
    ) -> reqwest::RequestBuilder {
        match (&self.api_key, headers.get(header::AUTHORIZATION)) {
            (Some(key), _) => req.bearer_auth(key),
            (None, Some(auth)) => req.header(header::AUTHORIZATION, auth),
            (None, None) => req,
        }
    }
}

/// The optional `ordo` field of a gateway request.
#[derive(Deserialize)]
struct DecideExtension {
    /// Ruleset to run on the answers.
    ruleset: String,
    /// Business facts for the rules (order, customer, ...), merged with the answers.
    #[serde(default)]
    facts: Map<String, JsonValue>,
    /// Where the answers go in the rule input.
    #[serde(default)]
    answers_field: Option<String>,
}

/// An upstream reply, kept as bytes so a pass-through returns it exactly.
struct Upstream {
    status: StatusCode,
    content_type: Option<HeaderValue>,
    retry_after: Option<HeaderValue>,
    body: Bytes,
}

impl Upstream {
    fn into_response(self) -> Response {
        let mut res = Response::new(Body::from(self.body));
        *res.status_mut() = self.status;
        let headers = res.headers_mut();
        if let Some(ct) = self.content_type {
            headers.insert(header::CONTENT_TYPE, ct);
        }
        if let Some(ra) = self.retry_after {
            headers.insert(header::RETRY_AFTER, ra);
        }
        res
    }
}

async fn send(req: reqwest::RequestBuilder) -> Result<Upstream, ApiError> {
    let res = req.send().await.map_err(bad_gateway)?;
    let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = header_value(res.headers(), "content-type");
    let retry_after = header_value(res.headers(), "retry-after");
    let body = res.bytes().await.map_err(bad_gateway)?;
    Ok(Upstream {
        status,
        content_type,
        retry_after,
        body,
    })
}

fn header_value(headers: &reqwest::header::HeaderMap, name: &str) -> Option<HeaderValue> {
    headers
        .get(name)
        .and_then(|v| HeaderValue::from_bytes(v.as_bytes()).ok())
}

fn bad_gateway(err: reqwest::Error) -> ApiError {
    ApiError {
        status: if err.is_timeout() {
            StatusCode::GATEWAY_TIMEOUT
        } else {
            StatusCode::BAD_GATEWAY
        },
        code: "UPSTREAM_ERROR".to_string(),
        message: format!("decision model upstream: {err}"),
    }
}

fn new_decision_id() -> String {
    format!(
        "dec_{}{:06x}",
        Utc::now().format("%Y%m%d%H%M%S%3f"),
        rand::random::<u32>() & 0x00ff_ffff
    )
}

/// `GET /systemone/v1/models`, passed through to the upstream.
pub async fn models(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(gw) = state.decide.as_deref() else {
        return ApiError::not_found("decision gateway is not enabled").into_response();
    };
    match send(gw.authorize(gw.client.get(&gw.models_url), &headers)).await {
        Ok(up) => up.into_response(),
        Err(e) => e.into_response(),
    }
}

/// `POST /systemone/v1/systemone`: forward to the model, optionally run a
/// ruleset on the answers, and log the call.
pub async fn systemone(
    State(state): State<AppState>,
    Extension(tenant): Extension<TenantContext>,
    headers: HeaderMap,
    Json(mut body): Json<JsonValue>,
) -> Response {
    match handle(&state, &tenant, &headers, &mut body).await {
        Ok(res) => res,
        Err(e) => e.into_response(),
    }
}

async fn handle(
    state: &AppState,
    tenant: &TenantContext,
    headers: &HeaderMap,
    body: &mut JsonValue,
) -> Result<Response, ApiError> {
    let gw = state
        .decide
        .as_deref()
        .ok_or_else(|| ApiError::not_found("decision gateway is not enabled"))?;
    let obj = body
        .as_object_mut()
        .ok_or_else(|| ApiError::bad_request("request body must be a JSON object"))?;

    // The extension is Ordo's, so it is removed before the request goes upstream.
    let ext = match obj.remove("ordo") {
        None | Some(JsonValue::Null) => None,
        Some(v) => Some(
            serde_json::from_value::<DecideExtension>(v)
                .map_err(|e| ApiError::bad_request(format!("invalid `ordo` field: {e}")))?,
        ),
    };

    // Resolve the ruleset before paying for a model call.
    let rule = match &ext {
        Some(ext) => {
            let store = state.store.read().await;
            let ruleset = store
                .get_for_tenant(&tenant.id, &ext.ruleset)
                .ok_or_else(|| {
                    ApiError::not_found(format!("RuleSet '{}' not found", ext.ruleset))
                })?;
            Some((ruleset, store.get_all_data_for_tenant(&tenant.id)))
        }
        None => None,
    };

    let decision_id = new_decision_id();
    let started = Instant::now();
    let up = send(gw.authorize(gw.client.post(&gw.decide_url).json(&*body), headers)).await;
    let latency_ms = started.elapsed().as_millis() as u64;

    let mut record = json!({
        "decision_id": decision_id,
        "ts": Utc::now().to_rfc3339(),
        "tenant": tenant.id,
        "model": body.get("model"),
        "latency_ms": latency_ms,
    });
    if gw.log_requests {
        // The whole request as sent upstream (the `ordo` field is already
        // removed), so `ordo impact --model` can ask another model the same thing.
        record["request"] = body.clone();
    }

    let up = match up {
        Ok(up) => up,
        Err(e) => {
            record["error"] = json!(e.message);
            record["input"] = JsonValue::Null;
            gw.log.write_record(&record);
            return Err(e);
        }
    };
    record["upstream_status"] = json!(up.status.as_u16());

    let answer: Option<JsonValue> = up
        .status
        .is_success()
        .then(|| serde_json::from_slice(&up.body).ok())
        .flatten()
        .filter(JsonValue::is_object);
    let Some(mut answer) = answer else {
        // Upstream errors (401, 422, 429, 529, ...) and bodies that are not a
        // JSON object go back to the caller exactly as they came.
        record["input"] = JsonValue::Null;
        gw.log.write_record(&record);
        return Ok(up.into_response());
    };
    if let Some(model) = answer.get("model") {
        record["model"] = model.clone();
    }
    let answers = answer.get("answers").cloned().unwrap_or(JsonValue::Null);
    record["answers"] = answers.clone();

    let (Some(ext), Some((ruleset, external_data))) = (ext, rule) else {
        record["input"] = JsonValue::Null;
        gw.log.write_record(&record);
        return Ok(up.into_response());
    };

    let field = ext
        .answers_field
        .unwrap_or_else(|| DEFAULT_ANSWERS_FIELD.to_string());
    let mut input = ext.facts;
    input.insert(field.clone(), answers);
    let input = JsonValue::Object(input);
    record["ruleset"] = json!(ext.ruleset);
    record["answers_field"] = json!(field);
    record["rule_version"] = json!(ruleset.config.version);
    record["input"] = input.clone();

    let mut engine_input: Value = serde_json::from_value(input)
        .map_err(|e| ApiError::bad_request(format!("invalid facts: {e}")))?;
    if !external_data.is_null() {
        if let Value::Object(ref mut map) = engine_input {
            map.insert(std::sync::Arc::from("$data"), external_data);
        }
    }

    // Same tenant timeout and blocking-thread execution as /api/v1/execute.
    let options = (tenant.config.execution_timeout_ms > 0).then_some(ExecutionOptions {
        timeout_ms: Some(tenant.config.execution_timeout_ms),
        enable_trace: None,
        max_depth: None,
    });
    let executor = state.executor.clone();
    let exec_start = Instant::now();
    let outcome = tokio::task::spawn_blocking(move || {
        executor.execute_with_options(&ruleset, engine_input, options.as_ref())
    })
    .await
    .map_err(|e| ApiError::internal(format!("Execution task failed: {e}")))?;
    let exec_secs = exec_start.elapsed().as_secs_f64();

    let decision = match outcome {
        Ok(result) => {
            metrics::record_execution_success(&ext.ruleset, exec_secs);
            metrics::record_tenant_execution_success(&tenant.id, &ext.ruleset, exec_secs);
            metrics::record_terminal_result(&ext.ruleset, &result.code);
            let output = serde_json::to_value(&result.output).unwrap_or(JsonValue::Null);
            record["code"] = json!(result.code);
            record["output"] = output.clone();
            record["duration_us"] = json!(result.duration_us);
            json!({
                "decision_id": decision_id,
                "ruleset": ext.ruleset,
                "version": record["rule_version"],
                "code": result.code,
                "message": result.message,
                "output": output,
                "duration_us": result.duration_us,
            })
        }
        Err(e) => {
            metrics::record_execution_error(&ext.ruleset, exec_secs);
            metrics::record_tenant_execution_error(&tenant.id, &ext.ruleset, exec_secs);
            metrics::record_terminal_result(&ext.ruleset, "error");
            // The model already answered and was paid for, so its answer still
            // goes back; the rule error sits next to it.
            let err = ApiError::from(e);
            record["error"] = json!(err.message);
            json!({
                "decision_id": decision_id,
                "ruleset": ext.ruleset,
                "version": record["rule_version"],
                "error": { "code": err.code, "message": err.message },
            })
        }
    };
    gw.log.write_record(&record);

    answer["ordo"] = decision;
    Ok((StatusCode::OK, Json(answer)).into_response())
}
