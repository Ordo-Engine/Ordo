use crate::audit::AuditLogger;
use crate::metrics::PrometheusMetricSink;
use ordo_core::context::Value;
use ordo_core::prelude::{
    CapabilityCategory, CapabilityConfig, CapabilityDescriptor, CapabilityInvoker,
    CapabilityProvider, CapabilityRegistry, CapabilityRequest, CapabilityResponse, MetricSink,
    OrdoError, Result, RuleExecutor, TraceConfig,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Server-side runtime registry wrapper that adds tracing around capability calls.
#[derive(Default)]
pub struct ServerCapabilityRegistry {
    inner: CapabilityRegistry,
}

impl ServerCapabilityRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn register(
        &self,
        provider: Arc<dyn CapabilityProvider>,
    ) -> Option<Arc<dyn CapabilityProvider>> {
        self.inner.register(provider)
    }

    pub fn register_metric_sink(&self, sink: Arc<PrometheusMetricSink>) {
        self.register(Arc::new(PrometheusMetricCapability { sink }));
    }

    pub fn register_audit_logger(&self, audit_logger: Arc<AuditLogger>) {
        self.register(Arc::new(AuditCapability { audit_logger }));
    }

    pub fn register_http_client(&self) {
        self.register(Arc::new(HttpCapability::new()));
    }
}

impl CapabilityInvoker for ServerCapabilityRegistry {
    fn invoke(&self, request: &CapabilityRequest) -> Result<CapabilityResponse> {
        let span = tracing::info_span!(
            "capability.invoke",
            capability = %request.capability,
            operation = %request.operation
        );
        let _guard = span.enter();
        self.inner.invoke(request)
    }

    fn describe(&self, capability: &str) -> Option<CapabilityDescriptor> {
        self.inner.describe(capability)
    }
}

pub fn build_server_capability_invoker(
    metric_sink: Arc<PrometheusMetricSink>,
    audit_logger: Option<Arc<AuditLogger>>,
) -> Arc<dyn CapabilityInvoker> {
    let registry = Arc::new(ServerCapabilityRegistry::new());
    registry.register_http_client();
    registry.register_metric_sink(metric_sink);
    if let Some(audit_logger) = audit_logger {
        registry.register_audit_logger(audit_logger);
    }
    registry
}

pub fn build_rule_executor(
    metric_sink: Arc<dyn MetricSink>,
    capability_invoker: Option<Arc<dyn CapabilityInvoker>>,
) -> RuleExecutor {
    let mut executor = RuleExecutor::with_trace_and_metrics(TraceConfig::minimal(), metric_sink);
    if let Some(capability_invoker) = capability_invoker {
        executor.set_capability_invoker(capability_invoker);
    }
    executor
}

#[cfg(test)]
pub fn build_server_executor(
    metric_sink: Arc<PrometheusMetricSink>,
    audit_logger: Option<Arc<AuditLogger>>,
) -> Arc<RuleExecutor> {
    let capability_invoker = build_server_capability_invoker(metric_sink.clone(), audit_logger);
    let metric_sink_trait: Arc<dyn MetricSink> = metric_sink;
    Arc::new(build_rule_executor(
        metric_sink_trait,
        Some(capability_invoker),
    ))
}

pub fn emit_rule_execution_audit(
    capability_invoker: Option<Arc<dyn CapabilityInvoker>>,
    audit_logger: &AuditLogger,
    rule_name: &str,
    duration_us: u64,
    result: &str,
    source_ip: Option<String>,
) {
    if let Some(capability_invoker) = capability_invoker {
        let payload = Value::object({
            let mut m = std::collections::HashMap::new();
            m.insert("rule_name".to_string(), Value::string(rule_name));
            m.insert("duration_us".to_string(), Value::int(duration_us as i64));
            m.insert("result".to_string(), Value::string(result));
            if let Some(source_ip) = &source_ip {
                m.insert("source_ip".to_string(), Value::string(source_ip));
            }
            m
        });
        let request = CapabilityRequest::new("audit.logger", "rule_executed", payload);
        match capability_invoker.invoke(&request) {
            Ok(_) => return,
            Err(OrdoError::CapabilityNotFound { .. }) => {}
            Err(error) => {
                tracing::warn!(rule = %rule_name, error = %error, "Audit capability invocation failed, falling back to direct logger");
            }
        }
    }

    audit_logger.log_execution(rule_name, duration_us, result, source_ip);
}

pub fn invoke_http_json(
    capability_invoker: Option<Arc<dyn CapabilityInvoker>>,
    method: &str,
    url: &str,
    headers: HashMap<String, String>,
    json_body: &serde_json::Value,
    timeout_ms: Option<u64>,
) -> Result<Option<CapabilityResponse>> {
    let Some(capability_invoker) = capability_invoker else {
        return Ok(None);
    };

    let mut payload = std::collections::HashMap::new();
    payload.insert("url".to_string(), Value::string(url));
    payload.insert(
        "headers".to_string(),
        Value::object(
            headers
                .into_iter()
                .map(|(key, value)| (key, Value::string(value)))
                .collect(),
        ),
    );
    let body = serde_json::from_value(json_body.clone()).map_err(|error| {
        OrdoError::capability_invocation("network.http", format!("invalid json body: {}", error))
    })?;
    payload.insert("json_body".to_string(), body);

    let mut request = CapabilityRequest::new(
        "network.http",
        method.to_ascii_uppercase(),
        Value::object(payload),
    );
    if let Some(timeout_ms) = timeout_ms {
        request = request.with_timeout(timeout_ms);
    }

    match capability_invoker.invoke(&request) {
        Ok(response) => Ok(Some(response)),
        Err(OrdoError::CapabilityNotFound { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn http_response_status(response: &CapabilityResponse) -> Option<u16> {
    match response.payload.get_path("status") {
        Some(Value::Int(status)) => (*status).try_into().ok(),
        _ => None,
    }
}

struct PrometheusMetricCapability {
    sink: Arc<PrometheusMetricSink>,
}

impl CapabilityProvider for PrometheusMetricCapability {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor::new("metrics.prometheus", CapabilityCategory::Action)
            .with_description("Bridge capability calls into the Prometheus rule metric sink")
            .with_config(CapabilityConfig::new(CapabilityCategory::Action))
    }

    fn invoke(&self, request: &CapabilityRequest) -> Result<CapabilityResponse> {
        let payload = expect_object(&request.payload, "metrics.prometheus")?;
        let name = required_string(payload, "name", "metrics.prometheus")?;
        let value = required_number(payload, "value", "metrics.prometheus")?;
        let tags = optional_tags(payload, "tags", "metrics.prometheus")?;

        match request.operation.as_str() {
            "counter" => self.sink.record_counter(name, value, &tags),
            "gauge" => self.sink.record_gauge(name, value, &tags),
            other => {
                return Err(OrdoError::capability_invocation(
                    "metrics.prometheus",
                    format!("unsupported operation '{}'", other),
                ));
            }
        }

        Ok(CapabilityResponse::empty()
            .with_metadata("metric", name.to_string())
            .with_metadata("operation", request.operation.clone()))
    }
}

struct AuditCapability {
    audit_logger: Arc<AuditLogger>,
}

impl CapabilityProvider for AuditCapability {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor::new("audit.logger", CapabilityCategory::Action)
            .with_description("Bridge capability calls into the structured audit logger")
            .with_config(CapabilityConfig::new(CapabilityCategory::Action))
    }

    fn invoke(&self, request: &CapabilityRequest) -> Result<CapabilityResponse> {
        let payload = expect_object(&request.payload, "audit.logger")?;
        match request.operation.as_str() {
            "rule_executed" => {
                let rule_name = required_string(payload, "rule_name", "audit.logger")?;
                let duration_us = required_i64(payload, "duration_us", "audit.logger")?;
                let result = required_string(payload, "result", "audit.logger")?;
                let source_ip = optional_string(payload, "source_ip");
                self.audit_logger.log_execution(
                    rule_name,
                    duration_us.max(0) as u64,
                    result,
                    source_ip,
                );
                Ok(CapabilityResponse::empty()
                    .with_metadata("event", "rule_executed")
                    .with_metadata("rule_name", rule_name.to_string()))
            }
            other => Err(OrdoError::capability_invocation(
                "audit.logger",
                format!("unsupported operation '{}'", other),
            )),
        }
    }
}

struct HttpCapability {
    client: reqwest::Client,
}

impl HttpCapability {
    fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }
}

impl CapabilityProvider for HttpCapability {
    fn descriptor(&self) -> CapabilityDescriptor {
        CapabilityDescriptor::new("network.http", CapabilityCategory::Network)
            .with_description("Issue outbound HTTP requests through a capability provider")
            .with_config(CapabilityConfig::new(CapabilityCategory::Network).timeout(10_000))
    }

    fn invoke(&self, request: &CapabilityRequest) -> Result<CapabilityResponse> {
        let payload = expect_object(&request.payload, "network.http")?;
        let url = required_string(payload, "url", "network.http")?;
        let method = request
            .operation
            .parse::<reqwest::Method>()
            .map_err(|error| {
                OrdoError::capability_invocation(
                    "network.http",
                    format!("invalid method '{}': {}", request.operation, error),
                )
            })?;
        let headers = optional_tags(payload, "headers", "network.http")?;

        let json_body = payload
            .get("json_body")
            .map(|body| {
                serde_json::to_value(body).map_err(|error| {
                    OrdoError::capability_invocation(
                        "network.http",
                        format!("failed to serialize request body: {}", error),
                    )
                })
            })
            .transpose()?;
        let (status, body_text) = execute_http_request(
            self.client.clone(),
            method,
            url.to_string(),
            headers,
            json_body,
            request.timeout_ms,
        )?;

        let mut payload = std::collections::HashMap::new();
        payload.insert("status".to_string(), Value::int(status as i64));
        payload.insert("body".to_string(), Value::string(&body_text));
        if let Ok(json_body) = serde_json::from_str::<serde_json::Value>(&body_text) {
            let json_body = serde_json::from_value(json_body).map_err(|error| {
                OrdoError::capability_invocation(
                    "network.http",
                    format!("failed to convert response body: {}", error),
                )
            })?;
            payload.insert("json_body".to_string(), json_body);
        }

        Ok(CapabilityResponse::new(Value::object(payload))
            .with_metadata("status", status.to_string()))
    }
}

fn execute_http_request(
    client: reqwest::Client,
    method: reqwest::Method,
    url: String,
    headers: Vec<(String, String)>,
    json_body: Option<serde_json::Value>,
    timeout_ms: Option<u64>,
) -> Result<(u16, String)> {
    async fn send(
        client: reqwest::Client,
        method: reqwest::Method,
        url: String,
        headers: Vec<(String, String)>,
        json_body: Option<serde_json::Value>,
        timeout_ms: Option<u64>,
    ) -> std::result::Result<(u16, String), reqwest::Error> {
        let mut builder = client.request(method, url);
        if let Some(timeout_ms) = timeout_ms {
            builder = builder.timeout(Duration::from_millis(timeout_ms));
        }
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        if let Some(json_body) = json_body {
            builder = builder.json(&json_body);
        }
        let response = builder.send().await?;
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        Ok((status, body))
    }

    let current = tokio::runtime::Handle::try_current();
    match current {
        Ok(handle) => match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => tokio::task::block_in_place(|| {
                handle.block_on(send(client, method, url, headers, json_body, timeout_ms))
            })
            .map_err(|error| OrdoError::capability_invocation("network.http", error.to_string())),
            tokio::runtime::RuntimeFlavor::CurrentThread | _ => std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| {
                        OrdoError::capability_invocation(
                            "network.http",
                            format!("failed to build runtime: {}", error),
                        )
                    })?;
                runtime
                    .block_on(send(client, method, url, headers, json_body, timeout_ms))
                    .map_err(|error| {
                        OrdoError::capability_invocation("network.http", error.to_string())
                    })
            })
            .join()
            .map_err(|_| {
                OrdoError::capability_invocation("network.http", "http worker thread panicked")
            })?,
        },
        Err(_) => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    OrdoError::capability_invocation(
                        "network.http",
                        format!("failed to build runtime: {}", error),
                    )
                })?;
            runtime
                .block_on(send(client, method, url, headers, json_body, timeout_ms))
                .map_err(|error| {
                    OrdoError::capability_invocation("network.http", error.to_string())
                })
        }
    }
}

fn expect_object<'a>(
    value: &'a Value,
    capability: &str,
) -> Result<&'a hashbrown::HashMap<ordo_core::context::IString, Value>> {
    value.as_object().ok_or_else(|| {
        OrdoError::capability_invocation(capability, "expected object payload for capability")
    })
}

fn required_string<'a>(
    object: &'a hashbrown::HashMap<ordo_core::context::IString, Value>,
    field: &str,
    capability: &str,
) -> Result<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| OrdoError::capability_invocation(capability, format!("missing '{}'", field)))
}

fn optional_string(
    object: &hashbrown::HashMap<ordo_core::context::IString, Value>,
    field: &str,
) -> Option<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn required_number(
    object: &hashbrown::HashMap<ordo_core::context::IString, Value>,
    field: &str,
    capability: &str,
) -> Result<f64> {
    match object.get(field) {
        Some(Value::Int(value)) => Ok(*value as f64),
        Some(Value::Float(value)) => Ok(*value),
        Some(Value::Bool(value)) => Ok(if *value { 1.0 } else { 0.0 }),
        _ => Err(OrdoError::capability_invocation(
            capability,
            format!("field '{}' must be numeric", field),
        )),
    }
}

fn required_i64(
    object: &hashbrown::HashMap<ordo_core::context::IString, Value>,
    field: &str,
    capability: &str,
) -> Result<i64> {
    match object.get(field) {
        Some(Value::Int(value)) => Ok(*value),
        _ => Err(OrdoError::capability_invocation(
            capability,
            format!("field '{}' must be an integer", field),
        )),
    }
}

fn optional_tags(
    object: &hashbrown::HashMap<ordo_core::context::IString, Value>,
    field: &str,
    capability: &str,
) -> Result<Vec<(String, String)>> {
    let Some(Value::Object(tags)) = object.get(field) else {
        return Ok(Vec::new());
    };

    let mut result = Vec::with_capacity(tags.len());
    for (key, value) in tags {
        let value = value.as_str().ok_or_else(|| {
            OrdoError::capability_invocation(capability, format!("tag '{}' must be a string", key))
        })?;
        result.push((key.to_string(), value.to_string()));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Bytes, http::Method, routing::any, Json, Router};
    use serde_json::json;
    use tokio::{net::TcpListener, sync::mpsc, time::Duration};

    #[test]
    fn metric_capability_accepts_gauge_requests() {
        let registry = ServerCapabilityRegistry::new();
        registry.register_metric_sink(Arc::new(PrometheusMetricSink::new()));

        let payload = Value::object({
            let mut m = std::collections::HashMap::new();
            m.insert("name".to_string(), Value::string("score"));
            m.insert("value".to_string(), Value::int(3));
            m
        });

        let response = registry
            .invoke(&CapabilityRequest::new(
                "metrics.prometheus",
                "gauge",
                payload,
            ))
            .unwrap();

        assert_eq!(response.metadata.get("metric"), Some(&"score".to_string()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn http_capability_posts_json() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(error) => panic!("failed to bind test listener: {}", error),
        };
        let addr = listener.local_addr().unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let app = Router::new().route(
            "/hook",
            any(move |method: Method, body: Bytes| {
                let tx = tx.clone();
                async move {
                    let json_body = serde_json::from_slice::<serde_json::Value>(&body)
                        .unwrap_or(serde_json::Value::Null);
                    let _ = tx.send((method.clone(), json_body.clone()));

                    Json(json!({
                        "method": method.as_str(),
                        "received": json_body,
                        "ok": true
                    }))
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let capability_invoker =
            build_server_capability_invoker(Arc::new(PrometheusMetricSink::new()), None);
        let response = invoke_http_json(
            Some(capability_invoker),
            "post",
            &format!("http://{}/hook", addr),
            HashMap::new(),
            &json!({"hello": "world"}),
            Some(1_000),
        )
        .unwrap()
        .unwrap();

        assert_eq!(http_response_status(&response), Some(200));
        let (method, received_body) = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for test server request")
            .expect("test server did not receive request");
        assert_eq!(method, Method::POST);
        assert_eq!(received_body.get("hello"), Some(&json!("world")));
        assert_eq!(
            response.payload.get_path("json_body.received.hello"),
            Some(&Value::string("world"))
        );
        assert_eq!(
            response.payload.get_path("json_body.method"),
            Some(&Value::string("POST"))
        );

        server.abort();
    }
}
