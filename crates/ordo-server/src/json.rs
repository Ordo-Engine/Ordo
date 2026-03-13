//! Fast JSON extraction using simd-json for deserialization.
//!
//! Drop-in replacement for `axum::Json` that uses simd-json for parsing
//! request bodies (~2-4x faster than serde_json on modern CPUs).
//! Response serialization still uses serde_json.

use axum::{
    async_trait,
    body::Bytes,
    extract::{FromRequest, Request},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{de::DeserializeOwned, Serialize};

/// Fast JSON extractor that uses simd-json for deserialization.
///
/// Usage is identical to `axum::Json<T>`:
/// ```ignore
/// async fn handler(SimdJson(payload): SimdJson<MyRequest>) -> ... { }
/// ```
pub struct SimdJson<T>(pub T);

#[async_trait]
impl<S, T> FromRequest<S> for SimdJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = SimdJsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // Validate content type — match axum::Json behaviour:
        // accept "application/json" or "application/json; charset=utf-8" etc.
        if let Some(ct) = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
        {
            let mime = ct.split(';').next().unwrap_or("").trim();
            if mime != "application/json" {
                return Err(SimdJsonRejection::InvalidContentType);
            }
        }

        // Extract body bytes
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| SimdJsonRejection::BodyReadError(e.to_string()))?;

        // simd-json needs a mutable slice (it modifies in-place for speed).
        // For small payloads (<512 bytes), the copy overhead may outweigh
        // simd-json gains, but for typical batch/execute payloads this is a net win.
        let mut buf = bytes.to_vec();

        let value = simd_json::from_slice::<T>(&mut buf)
            .map_err(|e| SimdJsonRejection::DeserializeError(e.to_string()))?;

        Ok(SimdJson(value))
    }
}

/// Implement IntoResponse so SimdJson<T> can be used as a response type too.
/// For responses, we use standard serde_json serialization.
impl<T: Serialize> IntoResponse for SimdJson<T> {
    fn into_response(self) -> Response {
        Json(self.0).into_response()
    }
}

/// Rejection type for SimdJson extractor
pub enum SimdJsonRejection {
    InvalidContentType,
    BodyReadError(String),
    DeserializeError(String),
}

impl IntoResponse for SimdJsonRejection {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::InvalidContentType => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Expected content-type: application/json".to_string(),
            ),
            Self::BodyReadError(e) => (
                StatusCode::BAD_REQUEST,
                format!("Failed to read request body: {}", e),
            ),
            Self::DeserializeError(e) => {
                (StatusCode::BAD_REQUEST, format!("JSON parse error: {}", e))
            }
        };

        let body = serde_json::json!({
            "code": "BAD_REQUEST",
            "message": message,
        });

        (status, Json(body)).into_response()
    }
}
