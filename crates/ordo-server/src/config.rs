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
//! | `ORDO_PORT` | HTTP server port (shorthand) | `8080` |
//! | `ORDO_GRPC_PORT` | gRPC server port (shorthand) | `50051` |
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
//! | `ORDO_MULTI_TENANCY_ENABLED` | Enable multi-tenancy | `false` |
//! | `ORDO_DEFAULT_TENANT` | Default tenant id | `default` |
//! | `ORDO_DEFAULT_TENANT_QPS` | Default tenant QPS limit | `-` |
//! | `ORDO_DEFAULT_TENANT_BURST` | Default tenant burst limit | `-` |
//! | `ORDO_DEFAULT_TENANT_TIMEOUT_MS` | Default tenant timeout ms | `100` |
//! | `ORDO_TENANTS_DIR` | Tenant config directory | `-` |
//! | `ORDO_SIGNATURE_ENABLED` | Enable signature verification | `false` |
//! | `ORDO_SIGNATURE_REQUIRE` | Reject unsigned rules | `false` |
//! | `ORDO_SIGNATURE_TRUSTED_KEYS` | Comma-separated public keys | `-` |
//! | `ORDO_SIGNATURE_TRUSTED_KEYS_FILE` | Public key file path | `-` |
//! | `ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL` | Allow unsigned local files | `true` |
//! | `ORDO_SERVICE_NAME` | Service name for OTel traces | `ordo-server` |
//! | `ORDO_OTLP_ENDPOINT` | OTLP HTTP endpoint for traces | - |
//! | `ORDO_SHUTDOWN_TIMEOUT_SECS` | Graceful shutdown timeout | `30` |
//! | `ORDO_MAX_REQUEST_BODY_BYTES` | Max HTTP request body size | `10485760` (10MB) |
//! | `ORDO_REQUEST_TIMEOUT_SECS` | HTTP request timeout | `30` |
//! | `ORDO_MAX_RULES_PER_TENANT` | Max rulesets per tenant | unlimited |
//! | `ORDO_MAX_TOTAL_RULES` | Max rulesets across all tenants | unlimited |
//! | `ORDO_GRPC_TLS_ENABLED` | Enable TLS for gRPC | `false` |
//! | `ORDO_GRPC_TLS_CERT` | Server certificate PEM path | - |
//! | `ORDO_GRPC_TLS_KEY` | Server private key PEM path | - |
//! | `ORDO_GRPC_MTLS_ENABLED` | Enable mutual TLS for gRPC | `false` |
//! | `ORDO_GRPC_TLS_CLIENT_CA` | Client CA certificate PEM path | - |

use clap::Parser;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Instance role in a distributed deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstanceRole {
    /// Single-node mode — full read/write (current default behaviour).
    #[default]
    Standalone,
    /// Accepts mutations and publishes changes to readers.
    Writer,
    /// Read-only — serves GET/execute, rejects mutations.
    Reader,
}

impl fmt::Display for InstanceRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstanceRole::Standalone => write!(f, "standalone"),
            InstanceRole::Writer => write!(f, "writer"),
            InstanceRole::Reader => write!(f, "reader"),
        }
    }
}

impl std::str::FromStr for InstanceRole {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standalone" => Ok(InstanceRole::Standalone),
            "writer" => Ok(InstanceRole::Writer),
            "reader" => Ok(InstanceRole::Reader),
            other => Err(format!(
                "invalid role '{}', expected one of: standalone, writer, reader",
                other
            )),
        }
    }
}

/// Ordo Rule Engine Server
#[derive(Parser, Debug, Clone)]
#[command(name = "ordo-server")]
#[command(author, version, about, long_about = None)]
pub struct ServerConfig {
    /// Instance role: standalone (default), writer, or reader.
    /// Writer accepts mutations; reader is read-only and rejects write requests.
    #[arg(long, default_value = "standalone", env = "ORDO_ROLE")]
    pub role: InstanceRole,

    /// Writer instance address (e.g. http://10.0.0.1:8080).
    /// Reader instances include this in 409 responses so clients can redirect writes.
    #[arg(long, env = "ORDO_WRITER_ADDR")]
    pub writer_addr: Option<String>,

    /// Enable file-system watching for live rule reload (requires --rules-dir).
    #[arg(long, default_value = "false", env = "ORDO_WATCH_RULES")]
    pub watch_rules: bool,

    /// NATS server URL for distributed sync (e.g. nats://localhost:4222).
    /// When set, the writer publishes rule changes to NATS JetStream and
    /// readers subscribe to receive updates. Requires the `nats-sync` feature.
    #[arg(long, env = "ORDO_NATS_URL")]
    pub nats_url: Option<String>,

    /// NATS subject prefix for sync events (default: ordo.rules).
    #[arg(long, default_value = "ordo.rules", env = "ORDO_NATS_SUBJECT_PREFIX")]
    pub nats_subject_prefix: String,

    /// Unique instance identifier for NATS consumer naming and echo suppression.
    /// Defaults to a random ID if not set.
    #[arg(long, env = "ORDO_INSTANCE_ID")]
    pub instance_id: Option<String>,

    /// HTTP server port (shorthand for --http-addr 0.0.0.0:<port>).
    /// If both --port and --http-addr are specified, --http-addr takes precedence.
    #[arg(short = 'p', long, env = "ORDO_PORT")]
    pub port: Option<u16>,

    /// gRPC server port (shorthand for --grpc-addr 0.0.0.0:<port>).
    /// If both --grpc-port and --grpc-addr are specified, --grpc-addr takes precedence.
    #[arg(long, env = "ORDO_GRPC_PORT")]
    pub grpc_port: Option<u16>,

    /// HTTP server address (e.g., 0.0.0.0:8080)
    #[arg(long = "http-addr", env = "ORDO_HTTP_ADDR")]
    http_addr_opt: Option<SocketAddr>,

    /// gRPC server address (e.g., 0.0.0.0:50051)
    #[arg(long = "grpc-addr", env = "ORDO_GRPC_ADDR")]
    grpc_addr_opt: Option<SocketAddr>,

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

    /// Enable multi-tenancy
    #[arg(long, default_value = "false", env = "ORDO_MULTI_TENANCY_ENABLED")]
    pub multi_tenancy_enabled: bool,

    /// Default tenant ID
    #[arg(long, default_value = "default", env = "ORDO_DEFAULT_TENANT")]
    pub default_tenant: String,

    /// Default tenant QPS limit (optional)
    #[arg(long, env = "ORDO_DEFAULT_TENANT_QPS")]
    pub default_tenant_qps: Option<u32>,

    /// Default tenant burst limit (optional)
    #[arg(long, env = "ORDO_DEFAULT_TENANT_BURST")]
    pub default_tenant_burst: Option<u32>,

    /// Default tenant execution timeout in ms
    #[arg(long, default_value = "100", env = "ORDO_DEFAULT_TENANT_TIMEOUT_MS")]
    pub default_tenant_timeout_ms: u64,

    /// Tenant configuration directory (optional)
    #[arg(long, env = "ORDO_TENANTS_DIR")]
    pub tenants_dir: Option<PathBuf>,

    /// Enable signature verification for rule updates and loads
    #[arg(long, default_value = "false", env = "ORDO_SIGNATURE_ENABLED")]
    pub signature_enabled: bool,

    /// Require signatures on rule updates (API)
    #[arg(long, default_value = "false", env = "ORDO_SIGNATURE_REQUIRE")]
    pub signature_require: bool,

    /// Trusted public keys (base64, comma-separated)
    #[arg(long, env = "ORDO_SIGNATURE_TRUSTED_KEYS", value_delimiter = ',')]
    pub signature_trusted_keys: Vec<String>,

    /// Trusted public keys file (one base64 key per line)
    #[arg(long, env = "ORDO_SIGNATURE_TRUSTED_KEYS_FILE")]
    pub signature_trusted_keys_file: Option<PathBuf>,

    /// Allow unsigned local rules when signature verification is enabled
    #[arg(
        long,
        default_value = "true",
        env = "ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL"
    )]
    pub signature_allow_unsigned_local: bool,

    /// Service name reported in OpenTelemetry traces and logs
    #[arg(long, default_value = "ordo-server", env = "ORDO_SERVICE_NAME")]
    pub service_name: String,

    /// OTLP HTTP endpoint for exporting traces (e.g. http://localhost:4318).
    /// If not set, OpenTelemetry is disabled.
    #[arg(long, env = "ORDO_OTLP_ENDPOINT")]
    pub otlp_endpoint: Option<String>,

    /// Seconds to wait for in-flight requests to complete during graceful shutdown.
    #[arg(long, default_value = "30", env = "ORDO_SHUTDOWN_TIMEOUT_SECS")]
    pub shutdown_timeout_secs: u64,

    /// Maximum HTTP request body size in bytes (default 10MB).
    /// gRPC uses tonic's built-in message size limit set to the same value.
    #[arg(long, default_value = "10485760", env = "ORDO_MAX_REQUEST_BODY_BYTES")]
    pub max_request_body_bytes: usize,

    /// HTTP request timeout in seconds.
    /// Requests exceeding this duration are terminated with 408 Request Timeout.
    #[arg(long, default_value = "30", env = "ORDO_REQUEST_TIMEOUT_SECS")]
    pub request_timeout_secs: u64,

    /// Maximum number of rulesets per tenant (optional, unlimited by default).
    /// When this limit is reached, new PUT requests are rejected with 422.
    #[arg(long, env = "ORDO_MAX_RULES_PER_TENANT")]
    pub max_rules_per_tenant: Option<usize>,

    /// Maximum total number of rulesets across all tenants (optional, unlimited by default).
    /// When this limit is reached, new PUT requests are rejected with 422.
    #[arg(long, env = "ORDO_MAX_TOTAL_RULES")]
    pub max_total_rules: Option<usize>,

    // ── gRPC TLS ──────────────────────────────────────────────────────
    /// Enable TLS for the gRPC server.
    /// Requires --grpc-tls-cert and --grpc-tls-key.
    #[arg(long, default_value = "false", env = "ORDO_GRPC_TLS_ENABLED")]
    pub grpc_tls_enabled: bool,

    /// Path to the PEM-encoded server certificate for gRPC TLS.
    #[arg(long, env = "ORDO_GRPC_TLS_CERT")]
    pub grpc_tls_cert: Option<PathBuf>,

    /// Path to the PEM-encoded private key (PKCS8) for gRPC TLS.
    #[arg(long, env = "ORDO_GRPC_TLS_KEY")]
    pub grpc_tls_key: Option<PathBuf>,

    /// Enable mutual TLS (mTLS) for the gRPC server.
    /// Clients must present a certificate signed by the CA specified in --grpc-tls-client-ca.
    /// Implies --grpc-tls-enabled.
    #[arg(long, default_value = "false", env = "ORDO_GRPC_MTLS_ENABLED")]
    pub grpc_mtls_enabled: bool,

    /// Path to the PEM-encoded CA certificate for verifying client certificates (mTLS).
    #[arg(long, env = "ORDO_GRPC_TLS_CLIENT_CA")]
    pub grpc_tls_client_ca: Option<PathBuf>,
}

impl ServerConfig {
    /// Get the HTTP server address.
    /// Priority: --http-addr > --port > default (0.0.0.0:8080)
    pub fn http_addr(&self) -> SocketAddr {
        if let Some(addr) = self.http_addr_opt {
            return addr;
        }
        if let Some(port) = self.port {
            return format!("0.0.0.0:{}", port).parse().unwrap();
        }
        "0.0.0.0:8080".parse().unwrap()
    }

    /// Get the gRPC server address.
    /// Priority: --grpc-addr > --grpc-port > default (0.0.0.0:50051)
    pub fn grpc_addr(&self) -> SocketAddr {
        if let Some(addr) = self.grpc_addr_opt {
            return addr;
        }
        if let Some(port) = self.grpc_port {
            return format!("0.0.0.0:{}", port).parse().unwrap();
        }
        "0.0.0.0:50051".parse().unwrap()
    }

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

    /// Returns true when this instance should reject write operations.
    pub fn is_read_only(&self) -> bool {
        self.role == InstanceRole::Reader
    }

    /// Resolve the instance ID.
    ///
    /// Priority: explicit `--instance-id` > `hostname:http_port` > random hex.
    /// Warns when falling back to a random ID with NATS enabled.
    pub fn resolve_instance_id(&self) -> String {
        if let Some(ref id) = self.instance_id {
            return id.clone();
        }

        // Prefer hostname:port for a stable, human-readable default.
        if let Ok(hostname) = hostname::get() {
            let hostname = hostname.to_string_lossy().to_string();
            if !hostname.is_empty() {
                return format!("{}:{}", hostname, self.http_addr().port());
            }
        }

        // Last resort: random hex — warn if NATS is active because a random ID
        // means a new durable consumer is created on every restart.
        let id = format!("{:016x}", rand::random::<u64>());
        if self.nats_enabled() {
            tracing::warn!(
                "Using random instance ID '{}' — set --instance-id for stable NATS consumers",
                id
            );
        }
        id
    }

    /// Returns true if NATS sync is configured.
    pub fn nats_enabled(&self) -> bool {
        self.nats_url.is_some()
    }

    /// Validate configuration for contradictory or risky settings.
    /// Logs warnings for non-fatal issues, returns Err for fatal ones.
    pub fn validate(&self) -> Result<(), String> {
        // Reader without writer-addr: clients won't know where to redirect writes
        if self.role == InstanceRole::Reader && self.writer_addr.is_none() {
            tracing::warn!(
                "Reader role without --writer-addr: 409 responses will not include a writer address"
            );
        }

        // Writer with writer-addr makes no sense
        if self.role == InstanceRole::Writer && self.writer_addr.is_some() {
            tracing::warn!(
                "--writer-addr is set but this instance is a writer; the value will be ignored"
            );
        }

        // NATS URL configured but feature not compiled in
        #[cfg(not(feature = "nats-sync"))]
        if self.nats_url.is_some() {
            tracing::warn!(
                "--nats-url is set but the binary was built without the `nats-sync` feature — NATS sync is disabled"
            );
        }

        // watch-rules without rules-dir
        if self.watch_rules && self.rules_dir.is_none() {
            tracing::warn!("--watch-rules has no effect without --rules-dir");
        }

        // max-versions without rules-dir
        if self.max_versions != 10 && self.rules_dir.is_none() {
            tracing::warn!("--max-versions has no effect without --rules-dir");
        }

        // All transports disabled
        if self.disable_http && self.disable_grpc && self.uds_path.is_none() {
            return Err(
                "All transports disabled (--disable-http, --disable-grpc, no --uds-path). Nothing to serve."
                    .to_string(),
            );
        }

        // signature-require without signature-enabled
        if self.signature_require && !self.signature_enabled {
            tracing::warn!("--signature-require has no effect without --signature-enabled");
        }

        // gRPC TLS validation
        if (self.grpc_tls_enabled || self.grpc_mtls_enabled)
            && (self.grpc_tls_cert.is_none() || self.grpc_tls_key.is_none())
        {
            return Err("gRPC TLS requires both --grpc-tls-cert and --grpc-tls-key".to_string());
        }
        if self.grpc_mtls_enabled && self.grpc_tls_client_ca.is_none() {
            return Err("gRPC mTLS requires --grpc-tls-client-ca".to_string());
        }

        Ok(())
    }

    /// Returns true when gRPC TLS should be enabled (explicit flag or implied by mTLS).
    pub fn grpc_tls_active(&self) -> bool {
        self.grpc_tls_enabled || self.grpc_mtls_enabled
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            role: InstanceRole::Standalone,
            writer_addr: None,
            watch_rules: false,
            nats_url: None,
            nats_subject_prefix: "ordo.rules".to_string(),
            instance_id: None,
            port: None,
            grpc_port: None,
            http_addr_opt: None,
            grpc_addr_opt: None,
            uds_path: None,
            disable_http: false,
            disable_grpc: false,
            log_level: "info".to_string(),
            rules_dir: None,
            max_versions: 10,
            audit_dir: None,
            audit_sample_rate: 10,
            debug_mode: false,
            multi_tenancy_enabled: false,
            default_tenant: "default".to_string(),
            default_tenant_qps: None,
            default_tenant_burst: None,
            default_tenant_timeout_ms: 100,
            tenants_dir: None,
            signature_enabled: false,
            signature_require: false,
            signature_trusted_keys: Vec::new(),
            signature_trusted_keys_file: None,
            signature_allow_unsigned_local: true,
            service_name: "ordo-server".to_string(),
            otlp_endpoint: None,
            shutdown_timeout_secs: 30,
            max_request_body_bytes: 10 * 1024 * 1024,
            request_timeout_secs: 30,
            max_rules_per_tenant: None,
            max_total_rules: None,
            grpc_tls_enabled: false,
            grpc_tls_cert: None,
            grpc_tls_key: None,
            grpc_mtls_enabled: false,
            grpc_tls_client_ca: None,
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

    #[test]
    fn test_instance_role_default() {
        let config = ServerConfig::default();
        assert_eq!(config.role, InstanceRole::Standalone);
        assert!(!config.is_read_only());
    }

    #[test]
    fn test_reader_is_read_only() {
        let config = ServerConfig {
            role: InstanceRole::Reader,
            ..Default::default()
        };
        assert!(config.is_read_only());
    }

    #[test]
    fn test_writer_is_not_read_only() {
        let config = ServerConfig {
            role: InstanceRole::Writer,
            ..Default::default()
        };
        assert!(!config.is_read_only());
    }

    #[test]
    fn test_resolve_instance_id_explicit() {
        let config = ServerConfig {
            instance_id: Some("my-node".to_string()),
            ..Default::default()
        };
        assert_eq!(config.resolve_instance_id(), "my-node");
    }

    #[test]
    fn test_resolve_instance_id_fallback_hostname() {
        let config = ServerConfig::default();
        let id = config.resolve_instance_id();
        // Should contain hostname:port or be a hex fallback
        assert!(!id.is_empty());
    }

    #[test]
    fn test_validate_all_transports_disabled() {
        let config = ServerConfig {
            disable_http: true,
            disable_grpc: true,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_ok_default() {
        let config = ServerConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_instance_role_parse() {
        assert_eq!(
            "standalone".parse::<InstanceRole>().unwrap(),
            InstanceRole::Standalone
        );
        assert_eq!(
            "writer".parse::<InstanceRole>().unwrap(),
            InstanceRole::Writer
        );
        assert_eq!(
            "reader".parse::<InstanceRole>().unwrap(),
            InstanceRole::Reader
        );
        assert_eq!(
            "Reader".parse::<InstanceRole>().unwrap(),
            InstanceRole::Reader
        );
        assert!("invalid".parse::<InstanceRole>().is_err());
    }

    #[test]
    fn test_grpc_tls_defaults_disabled() {
        let config = ServerConfig::default();
        assert!(!config.grpc_tls_enabled);
        assert!(!config.grpc_mtls_enabled);
        assert!(!config.grpc_tls_active());
    }

    #[test]
    fn test_grpc_tls_active_implied_by_mtls() {
        let config = ServerConfig {
            grpc_mtls_enabled: true,
            grpc_tls_cert: Some(PathBuf::from("/tmp/cert.pem")),
            grpc_tls_key: Some(PathBuf::from("/tmp/key.pem")),
            grpc_tls_client_ca: Some(PathBuf::from("/tmp/ca.pem")),
            ..Default::default()
        };
        assert!(config.grpc_tls_active());
    }

    #[test]
    fn test_grpc_tls_validate_missing_cert() {
        let config = ServerConfig {
            grpc_tls_enabled: true,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_grpc_mtls_validate_missing_ca() {
        let config = ServerConfig {
            grpc_mtls_enabled: true,
            grpc_tls_cert: Some(PathBuf::from("/tmp/cert.pem")),
            grpc_tls_key: Some(PathBuf::from("/tmp/key.pem")),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_grpc_tls_validate_ok() {
        let config = ServerConfig {
            grpc_tls_enabled: true,
            grpc_tls_cert: Some(PathBuf::from("/tmp/cert.pem")),
            grpc_tls_key: Some(PathBuf::from("/tmp/key.pem")),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_grpc_mtls_validate_ok() {
        let config = ServerConfig {
            grpc_mtls_enabled: true,
            grpc_tls_cert: Some(PathBuf::from("/tmp/cert.pem")),
            grpc_tls_key: Some(PathBuf::from("/tmp/key.pem")),
            grpc_tls_client_ca: Some(PathBuf::from("/tmp/ca.pem")),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }
}
