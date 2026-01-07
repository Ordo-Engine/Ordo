//! API error types

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// API error type
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}

/// Error response body
#[derive(Serialize)]
struct ErrorResponse {
    code: String,
    message: String,
}

impl ApiError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "NOT_FOUND".to_string(),
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "BAD_REQUEST".to_string(),
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL_ERROR".to_string(),
            message: message.into(),
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "CONFLICT".to_string(),
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            code: self.code,
            message: self.message,
        });
        (self.status, body).into_response()
    }
}

impl From<ordo_core::error::OrdoError> for ApiError {
    fn from(err: ordo_core::error::OrdoError) -> Self {
        match &err {
            ordo_core::error::OrdoError::RuleSetNotFound { name } => {
                ApiError::not_found(format!("RuleSet '{}' not found", name))
            }
            ordo_core::error::OrdoError::ParseError { message, .. } => {
                ApiError::bad_request(format!("Parse error: {}", message))
            }
            ordo_core::error::OrdoError::EvalError { message, .. } => {
                ApiError::bad_request(format!("Evaluation error: {}", message))
            }
            _ => ApiError::internal(err.to_string()),
        }
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::bad_request(format!("JSON error: {}", err))
    }
}
