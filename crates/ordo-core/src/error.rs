//! Ordo error types definition
//!
//! # Performance Optimizations
//!
//! Uses `Cow<'static, str>` for error messages where possible to reduce allocations
//! for static error messages while still supporting dynamic messages.

use std::borrow::Cow;
use thiserror::Error;

/// Ordo core error type
///
/// Uses `Cow<'static, str>` for message fields to optimize for static strings
/// while still supporting dynamic error messages.
#[derive(Error, Debug, Clone)]
pub enum OrdoError {
    /// Rule parsing error
    #[error("Parse error: {message}")]
    ParseError {
        message: Cow<'static, str>,
        location: Option<Cow<'static, str>>,
    },

    /// Expression evaluation error
    #[error("Evaluation error: {message}")]
    EvalError {
        message: Cow<'static, str>,
        expr: Option<String>,
    },

    /// Type mismatch error
    #[error("Type error: expected {expected}, got {actual}")]
    TypeError {
        expected: Cow<'static, str>,
        actual: Cow<'static, str>,
    },

    /// Field not found
    #[error("Field not found: {field}")]
    FieldNotFound { field: String },

    /// Function not found
    #[error("Function not found: {name}")]
    FunctionNotFound { name: Cow<'static, str> },

    /// Function argument error
    #[error("Function {name} argument error: {message}")]
    FunctionArgError {
        name: Cow<'static, str>,
        message: Cow<'static, str>,
    },

    /// RuleSet not found
    #[error("RuleSet not found: {name}")]
    RuleSetNotFound { name: String },

    /// Step not found
    #[error("Step not found: {step_id}")]
    StepNotFound { step_id: String },

    /// Execution timeout
    #[error("Execution timeout: {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// Max depth exceeded
    #[error("Max execution depth exceeded: {max_depth}")]
    MaxDepthExceeded { max_depth: usize },

    /// Configuration error
    #[error("Config error: {message}")]
    ConfigError { message: Cow<'static, str> },

    /// Internal error
    #[error("Internal error: {message}")]
    InternalError { message: Cow<'static, str> },
}

/// Ordo Result type alias
pub type Result<T> = std::result::Result<T, OrdoError>;

impl OrdoError {
    /// Create a parse error from a static string (no allocation)
    #[inline]
    pub fn parse_error_static(message: &'static str) -> Self {
        Self::ParseError {
            message: Cow::Borrowed(message),
            location: None,
        }
    }

    /// Create a parse error (accepts any string-like type)
    pub fn parse_error(message: impl Into<Cow<'static, str>>) -> Self {
        Self::ParseError {
            message: message.into(),
            location: None,
        }
    }

    /// Create a parse error with location
    pub fn parse_error_at(
        message: impl Into<Cow<'static, str>>,
        location: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::ParseError {
            message: message.into(),
            location: Some(location.into()),
        }
    }

    /// Create an evaluation error from a static string (no allocation)
    #[inline]
    pub fn eval_error_static(message: &'static str) -> Self {
        Self::EvalError {
            message: Cow::Borrowed(message),
            expr: None,
        }
    }

    /// Create an evaluation error
    pub fn eval_error(message: impl Into<Cow<'static, str>>) -> Self {
        Self::EvalError {
            message: message.into(),
            expr: None,
        }
    }

    /// Create a type error from static strings (no allocation)
    #[inline]
    pub fn type_error_static(expected: &'static str, actual: &'static str) -> Self {
        Self::TypeError {
            expected: Cow::Borrowed(expected),
            actual: Cow::Borrowed(actual),
        }
    }

    /// Create a type error
    pub fn type_error(
        expected: impl Into<Cow<'static, str>>,
        actual: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::TypeError {
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Create a field not found error
    pub fn field_not_found(field: impl Into<String>) -> Self {
        Self::FieldNotFound {
            field: field.into(),
        }
    }

    /// Create a function not found error
    pub fn function_not_found(name: impl Into<Cow<'static, str>>) -> Self {
        Self::FunctionNotFound { name: name.into() }
    }

    /// Create a config error from a static string (no allocation)
    #[inline]
    pub fn config_error_static(message: &'static str) -> Self {
        Self::ConfigError {
            message: Cow::Borrowed(message),
        }
    }

    /// Create a config error
    pub fn config_error(message: impl Into<Cow<'static, str>>) -> Self {
        Self::ConfigError {
            message: message.into(),
        }
    }

    /// Create an internal error from a static string (no allocation)
    #[inline]
    pub fn internal_error_static(message: &'static str) -> Self {
        Self::InternalError {
            message: Cow::Borrowed(message),
        }
    }

    /// Create an internal error
    pub fn internal_error(message: impl Into<Cow<'static, str>>) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }
}
