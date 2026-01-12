//! Server configuration

use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Ordo Rule Engine Server
#[derive(Parser, Debug, Clone)]
#[command(name = "ordo-server")]
#[command(author, version, about, long_about = None)]
pub struct ServerConfig {
    /// HTTP server address (e.g., 0.0.0.0:8080)
    #[arg(long, default_value = "0.0.0.0:8080")]
    pub http_addr: SocketAddr,

    /// gRPC server address (e.g., 0.0.0.0:50051)
    #[arg(long, default_value = "0.0.0.0:50051")]
    pub grpc_addr: SocketAddr,

    /// Unix Domain Socket path (optional)
    #[arg(long)]
    pub uds_path: Option<PathBuf>,

    /// Disable HTTP server
    #[arg(long, default_value = "false")]
    pub disable_http: bool,

    /// Disable gRPC server
    #[arg(long, default_value = "false")]
    pub disable_grpc: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Rules directory for persistence (optional).
    ///
    /// When specified, rules are:
    /// - Loaded from this directory on startup (.json, .yaml, .yml files)
    /// - Saved to this directory when created/updated via API
    /// - Deleted from this directory when removed via API
    ///
    /// Without this flag, rules are stored in memory only.
    #[arg(long)]
    pub rules_dir: Option<PathBuf>,

    /// Maximum number of historical versions to keep per rule.
    /// When a rule is updated, the previous version is saved.
    /// Older versions beyond this limit are automatically deleted.
    /// Only applies when --rules-dir is specified.
    #[arg(long, default_value = "10")]
    pub max_versions: usize,

    /// Audit log directory (optional).
    /// When specified, audit events are written to JSON Lines files
    /// in this directory, with daily rotation (audit-YYYY-MM-DD.jsonl).
    /// Events are also logged to stdout regardless of this setting.
    #[arg(long)]
    pub audit_dir: Option<PathBuf>,

    /// Execution log sampling rate (0-100, default 10).
    /// Controls the percentage of rule executions that are logged.
    /// 0 = no execution logging, 100 = log all executions.
    /// This can be changed at runtime via the API.
    #[arg(long, default_value = "10")]
    pub audit_sample_rate: u8,
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
}
