//! Ordo Rule Engine Server
//!
//! Provides HTTP, gRPC, and Unix Domain Socket APIs for rule execution.
//!
//! # Usage
//!
//! ```bash
//! # Start with default settings (HTTP on 8080, gRPC on 50051)
//! ordo-server
//!
//! # Start with UDS support
//! ordo-server --uds-path /tmp/ordo.sock
//!
//! # Disable HTTP, only gRPC
//! ordo-server --disable-http
//!
//! # Custom ports
//! ordo-server --http-addr 0.0.0.0:9090 --grpc-addr 0.0.0.0:9091
//!
//! # Enable audit logging with 10% sampling
//! ordo-server --audit-dir ./audit --audit-sample-rate 10
//!
//! # Enable debug mode (for development/testing only!)
//! ordo-server --debug-mode
//!
//! # Using environment variables
//! ORDO_HTTP_ADDR=0.0.0.0:9090 ORDO_DEBUG_MODE=true ordo-server
//! ```

use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use ordo_proto::ordo_service_server::OrdoServiceServer;
use tokio::sync::{watch, RwLock};
use tonic::transport::Server as TonicServer;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

mod api;
mod audit;
mod config;
pub mod debug;
mod error;
mod grpc;
mod metrics;
mod middleware;
mod rate_limiter;
mod store;
mod sync;
mod telemetry;
mod tenant;
#[cfg(unix)]
mod uds;

use audit::AuditLogger;
use config::ServerConfig;
use grpc::OrdoGrpcService;
use metrics::PrometheusMetricSink;
use ordo_core::prelude::{RuleExecutor, TraceConfig};
use ordo_core::signature::ed25519::decode_public_key;
use ordo_core::signature::RuleVerifier;
use rate_limiter::RateLimiter;
use std::fs;
use store::RuleStore;
use sync::file_watcher::RecentWrites;
use tenant::{default_tenant_store_path, TenantDefaults, TenantManager, TenantStore};

/// Application state shared between HTTP handlers
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<RwLock<RuleStore>>,
    pub audit_logger: Arc<AuditLogger>,
    pub metric_sink: Arc<PrometheusMetricSink>,
    /// Shared executor for rule execution (avoids holding lock during execution)
    pub executor: Arc<RuleExecutor>,
    /// Server configuration
    pub config: Arc<ServerConfig>,
    /// Signature verifier (if enabled)
    pub signature_verifier: Option<RuleVerifier>,
    /// Debug session manager (only active in debug mode)
    pub debug_sessions: Arc<debug::DebugSessionManager>,
    /// Tenant manager
    pub tenant_manager: Arc<TenantManager>,
    /// Tenant rate limiter
    pub rate_limiter: Arc<RateLimiter>,
}

fn build_signature_verifier(config: &ServerConfig) -> anyhow::Result<Option<RuleVerifier>> {
    if !config.signature_enabled {
        return Ok(None);
    }

    let mut keys = Vec::new();
    for encoded in &config.signature_trusted_keys {
        keys.push(decode_public_key(encoded).map_err(|e| anyhow::anyhow!(e))?);
    }

    if let Some(path) = &config.signature_trusted_keys_file {
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read trusted keys file {:?}: {}", path, e))?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            keys.push(decode_public_key(trimmed).map_err(|e| anyhow::anyhow!(e))?);
        }
    }

    Ok(Some(RuleVerifier::new(keys, config.signature_require)))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install panic hook first — before any other setup — so panics are always logged.
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("Box<dyn Any>");
        // Use both tracing (may not be initialized yet) and stderr as fallback.
        tracing::error!(location = %location, "PANIC: {}", payload);
        eprintln!("PANIC at {location}: {payload}");
    }));

    // Parse command line arguments (also reads from environment variables)
    let config = Arc::new(ServerConfig::parse());

    // Initialize structured logging (+ optional OTLP tracing)
    let otel_provider = telemetry::init(
        &config.service_name,
        &config.log_level,
        config.otlp_endpoint.as_deref(),
    );

    // Validate configuration
    if let Err(e) = config.validate() {
        return Err(anyhow::anyhow!("Configuration error: {}", e));
    }

    // Log instance role
    info!("Instance role: {}", config.role);
    if config.is_read_only() {
        info!(
            "Read-only mode — write requests will be rejected (writer: {})",
            config.writer_addr.as_deref().unwrap_or("not configured")
        );
    }

    // Log debug mode warning
    if config.debug_enabled() {
        warn!("╔════════════════════════════════════════════════════════════╗");
        warn!("║  DEBUG MODE ENABLED - DO NOT USE IN PRODUCTION!            ║");
        warn!("║  Debug API endpoints are exposed and may impact performance║");
        warn!("╚════════════════════════════════════════════════════════════╝");
    }

    // Initialize Prometheus metric sink for custom rule metrics
    let metric_sink = Arc::new(PrometheusMetricSink::new());
    info!("Initialized Prometheus metric sink for custom rule metrics");

    // Initialize shared executor (moved out of RuleStore for lock-free execution)
    let executor = Arc::new(RuleExecutor::with_trace_and_metrics(
        TraceConfig::minimal(),
        metric_sink.clone(),
    ));

    let signature_verifier = build_signature_verifier(&config)?;

    // Initialize shared store (with or without persistence)
    let store = if let Some(ref rules_dir) = config.rules_dir {
        let store_dir = if config.multi_tenancy_enabled {
            rules_dir.join("tenants")
        } else {
            rules_dir.clone()
        };
        info!(
            "Initializing store with persistence at {:?} (max {} versions)",
            store_dir, config.max_versions
        );
        let mut store = RuleStore::new_with_persistence_and_metrics(
            store_dir,
            config.max_versions,
            metric_sink.clone(),
        );
        if let Some(verifier) = signature_verifier.clone() {
            store.set_signature_verifier(verifier, config.signature_allow_unsigned_local);
        }
        if config.multi_tenancy_enabled {
            store.enable_multi_tenancy(config.default_tenant.clone());
        }
        store.set_resource_limits(config.max_rules_per_tenant, config.max_total_rules);

        // Load existing rules from directory
        match store.load_from_dir() {
            Ok(count) => {
                if count > 0 {
                    info!("Loaded {} rules from {:?}", count, rules_dir);
                } else {
                    info!(
                        "No rules found in {:?}, starting with empty store",
                        rules_dir
                    );
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to load rules from {:?}: {}",
                    rules_dir,
                    e
                ));
            }
        }

        Arc::new(RwLock::new(store))
    } else {
        info!("Initializing in-memory store (no persistence)");
        let mut store = RuleStore::new_with_metrics(metric_sink.clone());
        if let Some(verifier) = signature_verifier.clone() {
            store.set_signature_verifier(verifier, config.signature_allow_unsigned_local);
        }
        if config.multi_tenancy_enabled {
            store.enable_multi_tenancy(config.default_tenant.clone());
        }
        store.set_resource_limits(config.max_rules_per_tenant, config.max_total_rules);
        Arc::new(RwLock::new(store))
    };

    // Initialize metrics
    metrics::init();
    {
        let store_guard = store.read().await;
        metrics::set_rules_count(store_guard.len() as i64);
        metrics::set_tenant_rules_count(
            &config.default_tenant,
            store_guard.list_for_tenant(&config.default_tenant).len() as i64,
        );
    }

    // Initialize audit logger
    let audit_logger = Arc::new(AuditLogger::new(
        config.audit_dir.clone(),
        config.audit_sample_rate,
    ));

    // Log audit configuration
    if config.audit_dir.is_some() {
        info!(
            "Audit logging enabled: dir={:?}, sample_rate={}%",
            config.audit_dir, config.audit_sample_rate
        );
    } else {
        info!(
            "Audit logging to stdout only, sample_rate={}%",
            config.audit_sample_rate
        );
    }

    // Initialize debug session manager
    let debug_sessions = Arc::new(debug::DebugSessionManager::new());
    // Initialize tenant manager
    let tenant_defaults = TenantDefaults {
        default_qps_limit: config.default_tenant_qps,
        default_burst_limit: config.default_tenant_burst,
        default_timeout_ms: config.default_tenant_timeout_ms,
    };
    let tenant_store = config.tenants_dir.clone().or_else(|| {
        config
            .rules_dir
            .as_ref()
            .map(|dir| default_tenant_store_path(dir))
    });
    let tenant_store = tenant_store.map(TenantStore::new);
    let tenant_manager = Arc::new(TenantManager::new(tenant_store, tenant_defaults).await?);
    tenant_manager
        .ensure_default(&config.default_tenant)
        .await?;

    let rate_limiter = Arc::new(RateLimiter::new());
    let recent_writes = Arc::new(RecentWrites::new());

    // Shutdown broadcast channel — signal handlers and servers share this.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Log server started event
    {
        let store_guard = store.read().await;
        audit_logger.log_server_started(ordo_core::VERSION, store_guard.len());
    }

    // Create tasks for each enabled protocol
    let mut tasks = Vec::new();

    // HTTP Server
    if config.http_enabled() {
        let http_store = store.clone();
        let http_audit_logger = audit_logger.clone();
        let http_metric_sink = metric_sink.clone();
        let http_executor = executor.clone();
        let http_config = config.clone();
        let http_signature_verifier = signature_verifier.clone();
        let http_debug_sessions = debug_sessions.clone();
        let http_tenant_manager = tenant_manager.clone();
        let http_rate_limiter = rate_limiter.clone();
        let http_addr = config.http_addr();
        let http_shutdown_rx = shutdown_rx.clone();
        tasks.push(tokio::spawn(async move {
            start_http_server(
                http_addr,
                http_store,
                http_audit_logger,
                http_metric_sink,
                http_executor,
                http_config,
                http_signature_verifier,
                http_debug_sessions,
                http_tenant_manager,
                http_rate_limiter,
                http_shutdown_rx,
            )
            .await
        }));
    }

    // gRPC Server
    if config.grpc_enabled() {
        let grpc_store = store.clone();
        let grpc_executor = executor.clone();
        let grpc_addr = config.grpc_addr();
        let default_tenant = config.default_tenant.clone();
        let grpc_tenant_manager = tenant_manager.clone();
        let grpc_rate_limiter = rate_limiter.clone();
        let grpc_multi_tenancy_enabled = config.multi_tenancy_enabled;
        let grpc_max_body = config.max_request_body_bytes;
        let grpc_role = config.role;
        let grpc_writer_addr = config.writer_addr.clone();
        let grpc_shutdown_rx = shutdown_rx.clone();
        tasks.push(tokio::spawn(async move {
            start_grpc_server(
                grpc_addr,
                grpc_store,
                grpc_executor,
                default_tenant,
                grpc_tenant_manager,
                grpc_rate_limiter,
                grpc_multi_tenancy_enabled,
                grpc_max_body,
                grpc_role,
                grpc_writer_addr,
                grpc_shutdown_rx,
            )
            .await
        }));
    }

    // UDS Server (Unix only)
    #[cfg(unix)]
    if config.uds_enabled() {
        let uds_store = store.clone();
        let uds_executor = executor.clone();
        let uds_path = config.uds_path.clone().unwrap();
        let default_tenant = config.default_tenant.clone();
        let uds_tenant_manager = tenant_manager.clone();
        let uds_rate_limiter = rate_limiter.clone();
        let uds_multi_tenancy_enabled = config.multi_tenancy_enabled;
        let uds_max_body = config.max_request_body_bytes;
        let uds_shutdown_rx = shutdown_rx.clone();
        tasks.push(tokio::spawn(async move {
            uds::start_uds_server(
                &uds_path,
                uds_store,
                uds_executor,
                default_tenant,
                uds_tenant_manager,
                uds_rate_limiter,
                uds_multi_tenancy_enabled,
                uds_max_body,
                uds_shutdown_rx,
            )
            .await
            .map_err(|e| anyhow::anyhow!("UDS server error: {}", e))
        }));
    }

    // File watcher (if --watch-rules is set and --rules-dir is configured)
    let mut watcher_handle: Option<tokio::task::JoinHandle<()>> = None;
    if config.watch_rules {
        if let Some(ref rules_dir) = config.rules_dir {
            let watch_dir = if config.multi_tenancy_enabled {
                rules_dir.join("tenants")
            } else {
                rules_dir.clone()
            };
            let handle = sync::file_watcher::start_file_watcher(
                watch_dir,
                store.clone(),
                tenant_manager.clone(),
                recent_writes.clone(),
                shutdown_rx.clone(),
            )
            .await;
            watcher_handle = Some(handle);
            info!("File watcher enabled for {:?}", rules_dir);
        } else {
            warn!("--watch-rules requires --rules-dir; file watching disabled");
        }
    }

    // NATS sync (if configured and `nats-sync` feature is enabled)
    #[cfg(feature = "nats-sync")]
    let mut nats_subscriber_handle: Option<tokio::task::JoinHandle<()>> = None;
    #[cfg(feature = "nats-sync")]
    if let Some(ref nats_url) = config.nats_url {
        let instance_id = config.resolve_instance_id();
        info!(
            "Initializing NATS sync (url={}, instance={}, prefix={})",
            nats_url, instance_id, config.nats_subject_prefix
        );

        match sync::nats_sync::connect(nats_url).await {
            Ok(jetstream) => {
                if let Err(e) =
                    sync::nats_sync::ensure_stream(&jetstream, &config.nats_subject_prefix).await
                {
                    warn!("Failed to ensure NATS stream: {} — NATS sync disabled", e);
                } else {
                    // Writer: set up publisher → channel → store/tenant_manager
                    if !config.is_read_only() {
                        let publisher = sync::nats_sync::NatsPublisher::new(
                            jetstream.clone(),
                            config.nats_subject_prefix.clone(),
                            instance_id.clone(),
                        );
                        let sync_tx = publisher.start(shutdown_rx.clone());
                        {
                            let mut store_guard = store.write().await;
                            store_guard.set_sync_tx(sync_tx.clone());
                        }
                        tenant_manager.set_sync_tx(sync_tx).await;
                        info!("NATS publisher started (writer mode)");
                    }

                    // Reader (and writer for echo-suppressed fallback): set up subscriber
                    if config.is_read_only() {
                        match sync::nats_sync::create_consumer(&jetstream, &instance_id).await {
                            Ok(consumer) => {
                                let subscriber = sync::nats_sync::NatsSubscriber::new(
                                    consumer,
                                    instance_id.clone(),
                                    store.clone(),
                                    tenant_manager.clone(),
                                );
                                nats_subscriber_handle =
                                    Some(subscriber.start(shutdown_rx.clone()));
                                info!("NATS subscriber started (reader mode)");
                            }
                            Err(e) => {
                                warn!("Failed to create NATS consumer: {} — reader will rely on file watcher only", e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to connect to NATS at {}: {} — NATS sync disabled",
                    nats_url, e
                );
            }
        }
    }

    // Wait for shutdown signal or unexpected server exit, then drain gracefully.
    let shutdown_timeout = Duration::from_secs(config.shutdown_timeout_secs);

    if !tasks.is_empty() {
        // Build the shutdown signal future.
        let shutdown_signal = async {
            let ctrl_c = tokio::signal::ctrl_c();
            #[cfg(unix)]
            {
                let mut sigterm =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .unwrap();
                tokio::select! {
                    _ = ctrl_c => {},
                    _ = sigterm.recv() => {},
                }
            }
            #[cfg(not(unix))]
            ctrl_c.await.ok();
        };

        let all_tasks = futures::future::join_all(tasks);
        tokio::pin!(all_tasks);

        // Phase 1: Run until a signal arrives OR all servers exit on their own.
        tokio::select! {
            _ = shutdown_signal => {
                let uptime = metrics::START_TIME.elapsed().as_secs();
                audit_logger.log_server_stopped(uptime);
                info!(
                    uptime_secs = uptime,
                    timeout_secs = shutdown_timeout.as_secs(),
                    "Shutdown signal received — draining connections"
                );

                // Notify all servers to begin graceful shutdown.
                shutdown_tx.send(true).ok();

                // Phase 2: Wait for servers to finish, with a timeout.
                match tokio::time::timeout(shutdown_timeout, &mut all_tasks).await {
                    Ok(results) => {
                        for r in results {
                            r??;
                        }
                    }
                    Err(_) => {
                        warn!(
                            timeout_secs = shutdown_timeout.as_secs(),
                            "Graceful shutdown timed out — forcing exit"
                        );
                    }
                }
            }
            results = &mut all_tasks => {
                // Servers exited on their own (crash or error).
                for r in results {
                    r??;
                }
            }
        }
    } else {
        info!("No servers enabled. Exiting.");
    }

    // Stop the file watcher (it listens on shutdown_rx too, but abort for certainty).
    if let Some(handle) = watcher_handle {
        handle.abort();
        let _ = handle.await;
    }

    // Stop the NATS subscriber.
    #[cfg(feature = "nats-sync")]
    if let Some(handle) = nats_subscriber_handle {
        handle.abort();
        let _ = handle.await;
    }

    // Flush and shut down OpenTelemetry spans before process exit.
    if let Some(provider) = otel_provider {
        telemetry::shutdown(provider);
    }

    Ok(())
}

/// Start the HTTP server
#[allow(clippy::too_many_arguments)]
async fn start_http_server(
    addr: std::net::SocketAddr,
    store: Arc<RwLock<RuleStore>>,
    audit_logger: Arc<AuditLogger>,
    metric_sink: Arc<PrometheusMetricSink>,
    executor: Arc<RuleExecutor>,
    config: Arc<ServerConfig>,
    signature_verifier: Option<RuleVerifier>,
    debug_sessions: Arc<debug::DebugSessionManager>,
    tenant_manager: Arc<TenantManager>,
    rate_limiter: Arc<RateLimiter>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let debug_enabled = config.debug_enabled();

    let state = AppState {
        store,
        audit_logger,
        metric_sink,
        executor,
        config,
        signature_verifier,
        debug_sessions,
        tenant_manager,
        rate_limiter,
    };

    // Build base router
    let mut app = Router::new()
        // Health check (backward-compatible + Kubernetes-style probes)
        .route("/health", get(readiness_check))
        .route("/healthz/live", get(liveness_check))
        .route("/healthz/ready", get(readiness_check))
        // Rule management (Admin API)
        .route(
            "/api/v1/rulesets",
            get(api::list_rulesets).post(api::create_ruleset),
        )
        .route(
            "/api/v1/rulesets/:name",
            get(api::get_ruleset).delete(api::delete_ruleset),
        )
        // Version management
        .route(
            "/api/v1/rulesets/:name/versions",
            get(api::list_versions),
        )
        .route(
            "/api/v1/rulesets/:name/rollback",
            post(api::rollback_ruleset),
        )
        // Rule execution
        .route("/api/v1/execute/:name", post(api::execute_ruleset))
        // Batch execution
        .route(
            "/api/v1/execute/:name/batch",
            post(api::execute_ruleset_batch),
        )
        // Expression evaluation (debug)
        .route("/api/v1/eval", post(api::eval_expression))
        // Audit configuration
        .route(
            "/api/v1/config/audit-sample-rate",
            get(api::get_audit_sample_rate).put(api::set_audit_sample_rate),
        )
        // Metrics
        .route("/metrics", get(prometheus_metrics))
        // Tenant management
        .route(
            "/api/v1/tenants",
            get(api::list_tenants).post(api::create_tenant),
        )
        .route(
            "/api/v1/tenants/:id",
            get(api::get_tenant)
                .put(api::update_tenant)
                .delete(api::delete_tenant),
        );

    // Register debug routes only in debug mode
    if debug_enabled {
        info!("Registering debug API endpoints");
        app = app
            // Debug execution with full trace (existing ruleset by name)
            .route(
                "/api/v1/debug/execute/:name",
                post(debug::api::debug_execute_ruleset),
            )
            // Debug execution with inline ruleset (no upload required)
            .route(
                "/api/v1/debug/execute-inline",
                post(debug::api::debug_execute_inline),
            )
            // Debug expression evaluation
            .route("/api/v1/debug/eval", post(debug::api::debug_eval_expression))
            // Debug session management
            .route(
                "/api/v1/debug/sessions",
                get(debug::api::list_debug_sessions).post(debug::api::create_debug_session),
            )
            .route(
                "/api/v1/debug/sessions/:session_id",
                get(debug::api::get_debug_session).delete(debug::api::delete_debug_session),
            )
            // SSE stream for debug events
            .route(
                "/api/v1/debug/stream/:session_id",
                get(debug::api::debug_stream),
            )
            // Debug control commands
            .route(
                "/api/v1/debug/control/:session_id",
                post(debug::api::debug_control),
            );
    }

    // CORS configuration - permissive for debug mode, restrictive otherwise
    let cors = if debug_enabled {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
            ])
            .allow_headers([axum::http::header::CONTENT_TYPE])
    };

    let request_timeout = Duration::from_secs(state.config.request_timeout_secs);
    let max_body = state.config.max_request_body_bytes;

    let app = app
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
        .with_state(state);

    info!("HTTP server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_rx.changed().await.ok();
            info!("HTTP server: draining in-flight requests");
        })
        .await?;

    info!("HTTP server stopped");
    Ok(())
}

/// Start the gRPC server
#[allow(clippy::too_many_arguments)]
async fn start_grpc_server(
    addr: std::net::SocketAddr,
    store: Arc<RwLock<RuleStore>>,
    executor: Arc<RuleExecutor>,
    default_tenant: String,
    tenant_manager: Arc<TenantManager>,
    rate_limiter: Arc<RateLimiter>,
    multi_tenancy_enabled: bool,
    max_request_body_bytes: usize,
    role: config::InstanceRole,
    writer_addr: Option<String>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let grpc_service = OrdoGrpcService::new(
        store,
        executor,
        default_tenant,
        tenant_manager,
        rate_limiter,
        multi_tenancy_enabled,
    )
    .with_role(role, writer_addr);

    info!("gRPC server listening on {}", addr);

    TonicServer::builder()
        .tcp_keepalive(Some(Duration::from_secs(60)))
        .http2_keepalive_interval(Some(Duration::from_secs(30)))
        .http2_keepalive_timeout(Some(Duration::from_secs(20)))
        .add_service(
            OrdoServiceServer::new(grpc_service).max_decoding_message_size(max_request_body_bytes),
        )
        .serve_with_shutdown(addr, async move {
            shutdown_rx.changed().await.ok();
            info!("gRPC server: draining in-flight requests");
        })
        .await?;

    info!("gRPC server stopped");
    Ok(())
}

/// Liveness probe — confirms the process is running.
/// Always returns 200; use for Kubernetes liveness probes.
async fn liveness_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "alive",
        "version": ordo_core::VERSION,
        "uptime_seconds": metrics::START_TIME.elapsed().as_secs(),
    }))
}

/// Readiness probe — confirms the service can handle requests.
/// Checks store lock acquisition (with timeout) and disk writability.
/// Returns 503 if any check fails. Use for Kubernetes readiness probes
/// and load balancer health checks.
async fn readiness_check(State(state): State<AppState>) -> impl IntoResponse {
    let store_result = tokio::time::timeout(Duration::from_secs(2), state.store.read()).await;

    let (store_lock_ok, rules_count, storage_mode) = match store_result {
        Ok(guard) => {
            metrics::set_rules_count(guard.len() as i64);
            let mode = if guard.persistence_enabled() {
                "persistent"
            } else {
                "memory"
            };
            (true, guard.len(), mode)
        }
        Err(_) => (false, 0, "unknown"),
    };

    let disk_ok = if let Some(ref rules_dir) = state.config.rules_dir {
        let probe = rules_dir.join(".health_probe");
        match tokio::fs::write(&probe, b"ok").await {
            Ok(_) => {
                let _ = tokio::fs::remove_file(&probe).await;
                true
            }
            Err(_) => false,
        }
    } else {
        true
    };

    let is_ready = store_lock_ok && disk_ok;
    let status_code = if is_ready {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(serde_json::json!({
            "status": if is_ready { "ready" } else { "not_ready" },
            "version": ordo_core::VERSION,
            "role": state.config.role.to_string(),
            "uptime_seconds": metrics::START_TIME.elapsed().as_secs(),
            "checks": {
                "store_lock": store_lock_ok,
                "disk_writable": disk_ok,
            },
            "storage": {
                "mode": storage_mode,
                "rules_count": rules_count,
            },
            "sync": {
                "nats_configured": state.config.nats_enabled(),
                "watch_rules": state.config.watch_rules,
            },
            "debug_mode": state.config.debug_enabled()
        })),
    )
}

/// Prometheus metrics endpoint
async fn prometheus_metrics(State(state): State<AppState>) -> impl IntoResponse {
    // Update rules count before encoding
    let store = state.store.read().await;
    metrics::set_rules_count(store.len() as i64);
    drop(store);

    // Combine standard metrics with custom rule metrics
    let standard_metrics = metrics::encode_metrics();
    let custom_metrics = state.metric_sink.encode_custom_metrics();

    // Return Prometheus text format
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        format!("{}\n{}", standard_metrics, custom_metrics),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert!(config.http_enabled());
        assert!(config.grpc_enabled());
        assert!(!config.uds_enabled());
        assert!(!config.debug_enabled());
    }
}

#[cfg(test)]
mod api_tests;
