//! Telemetry initialization: structured logging + optional OpenTelemetry tracing.
//!
//! If `ORDO_OTLP_ENDPOINT` is set, spans are exported via OTLP/HTTP.
//! Otherwise only stdout logging is active (zero runtime overhead).

use opentelemetry_sdk::trace::TracerProvider;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the global tracing subscriber.
///
/// Returns a `TracerProvider` if OTLP was configured — caller must call
/// [`shutdown`] on it before the process exits to flush pending spans.
pub fn init(
    service_name: &str,
    log_level: &str,
    otlp_endpoint: Option<&str>,
) -> Option<TracerProvider> {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    let fmt_layer = tracing_subscriber::fmt::layer();

    if let Some(endpoint) = otlp_endpoint {
        match build_otel_provider(service_name, endpoint) {
            Ok(provider) => {
                // Use the concrete SDK tracer (not BoxedTracer) — required by PreSampledTracer bound.
                use opentelemetry::trace::TracerProvider as _;
                let tracer = provider.tracer(service_name.to_owned());
                let otel_layer = tracing_opentelemetry::OpenTelemetryLayer::new(tracer);

                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(fmt_layer)
                    .with(otel_layer)
                    .init();

                tracing::info!(
                    endpoint = %endpoint,
                    service = %service_name,
                    "OpenTelemetry OTLP exporter initialized"
                );

                Some(provider)
            }
            Err(e) => {
                // Fall back to fmt-only — don't crash the server over telemetry
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(fmt_layer)
                    .init();
                tracing::warn!(
                    error = %e,
                    "Failed to initialize OTLP exporter, falling back to stdout logging"
                );
                None
            }
        }
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
        None
    }
}

/// Flush and shut down the OTLP exporter. Call this before process exit.
pub fn shutdown(provider: TracerProvider) {
    opentelemetry::global::shutdown_tracer_provider();
    for result in provider.force_flush() {
        if let Err(e) = result {
            eprintln!("OTel flush error on shutdown: {e}");
        }
    }
}

fn build_otel_provider(service_name: &str, endpoint: &str) -> anyhow::Result<TracerProvider> {
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{runtime::Tokio, trace as sdktrace, Resource};

    let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_endpoint(endpoint)
        .build_span_exporter()
        .map_err(|e| anyhow::anyhow!("OTLP span exporter: {}", e))?;

    // In opentelemetry_sdk 0.22, resource is set via Config, not builder directly.
    let config = sdktrace::Config::default().with_resource(Resource::new(vec![KeyValue::new(
        "service.name",
        service_name.to_string(),
    )]));

    let provider = sdktrace::TracerProvider::builder()
        .with_config(config)
        .with_batch_exporter(exporter, Tokio)
        .build();

    // Set global provider so context propagation works for downstream code.
    opentelemetry::global::set_tracer_provider(provider.clone());

    Ok(provider)
}
