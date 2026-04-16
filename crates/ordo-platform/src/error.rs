//! Platform API error types

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl PlatformError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(anyhow::anyhow!(msg.into()))
    }
}

impl IntoResponse for PlatformError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            PlatformError::NotFound(m) => (StatusCode::NOT_FOUND, m.clone()),
            PlatformError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.clone()),
            PlatformError::Forbidden(m) => (StatusCode::FORBIDDEN, m.clone()),
            PlatformError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            PlatformError::Conflict(m) => (StatusCode::CONFLICT, m.clone()),
            PlatformError::Internal(e) => {
                tracing::error!("Internal platform error: {:#}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub type ApiResult<T> = std::result::Result<T, PlatformError>;
