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

/// Maximum allowed nesting depth for incoming JSON payloads.
///
/// `serde`/`simd-json` build the recursive `Value` enum on the stack, so a
/// hostile `[[[[…` / `{{{…` payload could exhaust the stack before any handler
/// logic runs. We reject anything nested beyond this bound up front. 128 is far
/// deeper than any legitimate rule input while staying well clear of a stack
/// overflow.
pub const MAX_JSON_DEPTH: usize = 128;

/// Cheap structural pre-scan: returns `true` if the raw JSON bytes nest arrays
/// or objects deeper than `max`. Brackets inside string literals are ignored.
///
/// This is O(n) over the body with no allocation and runs before the recursive
/// `simd_json::from_slice`, so a malicious payload is rejected before it can
/// blow the stack.
pub fn exceeds_max_depth(bytes: &[u8], max: usize) -> bool {
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escaped = false;

    for &b in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }

        match b {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > max {
                    return true;
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }

    false
}

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

        // Reject pathologically nested payloads before the recursive parse so a
        // hostile `[[[[…` body returns a clean 400 instead of risking a stack
        // overflow during deserialization.
        if exceeds_max_depth(&bytes, MAX_JSON_DEPTH) {
            return Err(SimdJsonRejection::TooDeeplyNested);
        }

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
    TooDeeplyNested,
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
            Self::TooDeeplyNested => (
                StatusCode::BAD_REQUEST,
                format!("JSON nesting exceeds maximum depth of {}", MAX_JSON_DEPTH),
            ),
        };

        let body = serde_json::json!({
            "code": "BAD_REQUEST",
            "message": message,
        });

        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shallow_payloads_are_accepted() {
        assert!(!exceeds_max_depth(b"{}", MAX_JSON_DEPTH));
        assert!(!exceeds_max_depth(b"[1, 2, 3]", MAX_JSON_DEPTH));
        assert!(!exceeds_max_depth(
            br#"{"a": {"b": [1, {"c": 2}]}}"#,
            MAX_JSON_DEPTH
        ));
    }

    #[test]
    fn deeply_nested_payload_is_rejected() {
        // 200 levels of nested arrays — comfortably over the 128 limit.
        let depth = 200;
        let mut payload = Vec::new();
        payload.extend(std::iter::repeat(b'[').take(depth));
        payload.extend(std::iter::repeat(b']').take(depth));
        assert!(exceeds_max_depth(&payload, MAX_JSON_DEPTH));

        // Mixed objects/arrays nested past the limit are also rejected.
        let mut mixed = Vec::new();
        for _ in 0..150 {
            mixed.extend_from_slice(b"{\"x\":");
        }
        mixed.extend_from_slice(b"1");
        mixed.extend(std::iter::repeat(b'}').take(150));
        assert!(exceeds_max_depth(&mixed, MAX_JSON_DEPTH));
    }

    #[test]
    fn brackets_inside_strings_are_ignored() {
        // A flat object whose string value is full of brackets must not count
        // toward nesting depth.
        let payload = br#"{"note": "[[[[[[[[[[ not real nesting ]]]]]]]]]]"}"#;
        assert!(!exceeds_max_depth(payload, MAX_JSON_DEPTH));

        // Escaped quotes inside strings must not confuse the string scanner.
        let escaped = br#"{"q": "a \" [ b"}"#;
        assert!(!exceeds_max_depth(escaped, MAX_JSON_DEPTH));
    }

    #[test]
    fn depth_exactly_at_limit_is_allowed() {
        let mut payload = Vec::new();
        payload.extend(std::iter::repeat(b'[').take(MAX_JSON_DEPTH));
        payload.extend(std::iter::repeat(b']').take(MAX_JSON_DEPTH));
        assert!(!exceeds_max_depth(&payload, MAX_JSON_DEPTH));
    }
}
