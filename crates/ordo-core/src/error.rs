//! Ordo error types definition

use thiserror::Error;

/// Ordo core error type
#[derive(Error, Debug, Clone)]
pub enum OrdoError {
    /// Rule parsing error
    #[error("Parse error: {message}")]
    ParseError {
        message: String,
        location: Option<String>,
    },

    /// Expression evaluation error
    #[error("Evaluation error: {message}")]
    EvalError {
        message: String,
        expr: Option<String>,
    },

    /// Type mismatch error
    #[error("Type error: expected {expected}, got {actual}")]
    TypeError { expected: String, actual: String },

    /// Field not found
    #[error("Field not found: {field}")]
    FieldNotFound { field: String },

    /// Function not found
    #[error("Function not found: {name}")]
    FunctionNotFound { name: String },

    /// Function argument error
    #[error("Function {name} argument error: {message}")]
    FunctionArgError { name: String, message: String },

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
    ConfigError { message: String },

    /// Internal error
    #[error("Internal error: {message}")]
    InternalError { message: String },
}

/// Ordo Result type alias
pub type Result<T> = std::result::Result<T, OrdoError>;

impl OrdoError {
    /// Create a parse error
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
            location: None,
        }
    }

    /// Create a parse error with location
    pub fn parse_error_at(message: impl Into<String>, location: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
            location: Some(location.into()),
        }
    }

    /// Create an evaluation error
    pub fn eval_error(message: impl Into<String>) -> Self {
        Self::EvalError {
            message: message.into(),
            expr: None,
        }
    }

    /// Create a type error
    pub fn type_error(expected: impl Into<String>, actual: impl Into<String>) -> Self {
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
    pub fn function_not_found(name: impl Into<String>) -> Self {
        Self::FunctionNotFound { name: name.into() }
    }
}
