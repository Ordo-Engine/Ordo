//! Optional global in-flight concurrency limit for the HTTP data routes.
//!
//! Backed by a shared [`tokio::sync::Semaphore`]. A permit is held for the
//! duration of the request and released when the response future completes,
//! giving backpressure equivalent to `tower::limit::ConcurrencyLimitLayer`
//! while sharing the same semaphore with the gRPC transport for a truly global
//! cap.
//!
//! This middleware is only layered onto the data routes (never `/healthz*` or
//! `/metrics`) and only when `ORDO_MAX_CONCURRENT_EXECUTIONS > 0`.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use tokio::sync::Semaphore;

/// Acquire a permit before running the inner handler; block (with backpressure)
/// until one is available.
pub async fn concurrency_limit_middleware(
    State(semaphore): State<Arc<Semaphore>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    // The semaphore is never closed, so the only error path is defensive.
    match semaphore.acquire().await {
        Ok(_permit) => next.run(req).await,
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "code": "OVERLOADED",
                "message": "Server at maximum concurrent executions",
            })),
        )
            .into_response(),
    }
}
