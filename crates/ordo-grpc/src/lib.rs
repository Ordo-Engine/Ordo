//! Ordo Protocol Buffers
//!
//! This crate contains the generated protobuf code for the Ordo gRPC service.

#![allow(missing_docs)]
#![allow(clippy::all)]

/// Generated protobuf code
pub mod ordo {
    pub mod v1 {
        include!("generated/ordo.v1.rs");
    }
}

// Re-export for convenience
pub use ordo::v1::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_request() {
        let req = ExecuteRequest {
            ruleset_name: "test_rule".to_string(),
            input_json: r#"{"value": 100}"#.to_string(),
            include_trace: true,
        };
        assert_eq!(req.ruleset_name, "test_rule");
        assert!(req.include_trace);
    }

    #[test]
    fn test_health_response() {
        let resp = HealthResponse {
            status: health_response::Status::Serving as i32,
            version: "0.1.0".to_string(),
            ruleset_count: 5,
            uptime_seconds: 3600,
        };
        assert_eq!(resp.status, health_response::Status::Serving as i32);
    }
}
