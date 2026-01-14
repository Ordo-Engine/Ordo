//! Server configuration
//!
//! Configuration can be provided via command-line arguments or environment variables.
//! Environment variables take precedence over default values but are overridden by
//! explicit command-line arguments.
//!
//! # Environment Variables
//!
//! | Variable | Description | Default |
//! |----------|-------------|---------|
//! | `ORDO_HTTP_ADDR` | HTTP server address | `0.0.0.0:8080` |
//! | `ORDO_GRPC_ADDR` | gRPC server address | `0.0.0.0:50051` |
//! | `ORDO_UDS_PATH` | Unix Domain Socket path | - |
//! | `ORDO_DISABLE_HTTP` | Disable HTTP server | `false` |
//! | `ORDO_DISABLE_GRPC` | Disable gRPC server | `false` |
//! | `ORDO_LOG_LEVEL` | Log level | `info` |
//! | `ORDO_RULES_DIR` | Rules persistence directory | - |
//! | `ORDO_MAX_VERSIONS` | Max rule versions to keep | `10` |
//! | `ORDO_AUDIT_DIR` | Audit log directory | - |
//! | `ORDO_AUDIT_SAMPLE_RATE` | Audit sampling rate (0-100) | `10` |
//! | `ORDO_DEBUG_MODE` | Enable debug mode | `false` |

use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Ordo Rule Engine Server
#[derive(Parser, Debug, Clone)]
#[command(name = "ordo-server")]
#[command(author, version, about, long_about = None)]
pub struct ServerConfig {
    /// HTTP server address (e.g., 0.0.0.0:8080)
    #[arg(long, default_value = "0.0.0.0:8080", env = "ORDO_HTTP_ADDR")]
    pub http_addr: SocketAddr,

    /// gRPC server address (e.g., 0.0.0.0:50051)
    #[arg(long, default_value = "0.0.0.0:50051", env = "ORDO_GRPC_ADDR")]
    pub grpc_addr: SocketAddr,

    /// Unix Domain Socket path (optional)
    #[arg(long, env = "ORDO_UDS_PATH")]
    pub uds_path: Option<PathBuf>,

    /// Disable HTTP server
    #[arg(long, default_value = "false", env = "ORDO_DISABLE_HTTP")]
    pub disable_http: bool,

    /// Disable gRPC server
    #[arg(long, default_value = "false", env = "ORDO_DISABLE_GRPC")]
    pub disable_grpc: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "ORDO_LOG_LEVEL")]
    pub log_level: String,

    /// Rules directory for persistence (optional).
    ///
    /// When specified, rules are:
    /// - Loaded from this directory on startup (.json, .yaml, .yml files)
    /// - Saved to this directory when created/updated via API
    /// - Deleted from this directory when removed via API
    ///
    /// Without this flag, rules are stored in memory only.
    #[arg(long, env = "ORDO_RULES_DIR")]
    pub rules_dir: Option<PathBuf>,

    /// Maximum number of historical versions to keep per rule.
    /// When a rule is updated, the previous version is saved.
    /// Older versions beyond this limit are automatically deleted.
    /// Only applies when --rules-dir is specified.
    #[arg(long, default_value = "10", env = "ORDO_MAX_VERSIONS")]
    pub max_versions: usize,

    /// Audit log directory (optional).
    /// When specified, audit events are written to JSON Lines files
    /// in this directory, with daily rotation (audit-YYYY-MM-DD.jsonl).
    /// Events are also logged to stdout regardless of this setting.
    #[arg(long, env = "ORDO_AUDIT_DIR")]
    pub audit_dir: Option<PathBuf>,

    /// Execution log sampling rate (0-100, default 10).
    /// Controls the percentage of rule executions that are logged.
    /// 0 = no execution logging, 100 = log all executions.
    /// This can be changed at runtime via the API.
    #[arg(long, default_value = "10", env = "ORDO_AUDIT_SAMPLE_RATE")]
    pub audit_sample_rate: u8,

    /// Enable debug/test mode.
    ///
    /// When enabled, additional debug API endpoints are available:
    /// - `/api/v1/debug/execute/:name` - Execute with detailed VM trace
    /// - `/api/v1/debug/eval` - Evaluate expression with AST and bytecode info
    /// - `/api/v1/debug/stream/:session_id` - SSE stream for step debugging
    /// - `/api/v1/debug/control/:session_id` - Control debug session
    ///
    /// **WARNING**: Do NOT enable in production environments!
    /// Debug mode exposes internal execution details and may impact performance.
    #[arg(long, default_value = "false", env = "ORDO_DEBUG_MODE")]
    pub debug_mode: bool,
}

impl ServerConfig {
    /// Check if HTTP server is enabled
    pub fn http_enabled(&self) -> bool {
        !self.disable_http
    }

    /// Check if gRPC server is enabled
    pub fn grpc_enabled(&self) -> bool {
        !self.disable_grpc
    }

    /// Check if UDS server is enabled
    pub fn uds_enabled(&self) -> bool {
        self.uds_path.is_some()
    }

    /// Check if debug mode is enabled
    pub fn debug_enabled(&self) -> bool {
        self.debug_mode
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            http_addr: "0.0.0.0:8080".parse().unwrap(),
            grpc_addr: "0.0.0.0:50051".parse().unwrap(),
            uds_path: None,
            disable_http: false,
            disable_grpc: false,
            log_level: "info".to_string(),
            rules_dir: None,
            max_versions: 10,
            audit_dir: None,
            audit_sample_rate: 10,
            debug_mode: false,
        }
    }
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

    #[test]
    fn test_disabled_servers() {
        let config = ServerConfig {
            disable_http: true,
            disable_grpc: true,
            ..Default::default()
        };
        assert!(!config.http_enabled());
        assert!(!config.grpc_enabled());
    }

    #[test]
    fn test_uds_enabled() {
        let config = ServerConfig {
            uds_path: Some(PathBuf::from("/tmp/ordo.sock")),
            ..Default::default()
        };
        assert!(config.uds_enabled());
    }

    #[test]
    fn test_debug_mode() {
        let config = ServerConfig {
            debug_mode: true,
            ..Default::default()
        };
        assert!(config.debug_enabled());
    }
}
