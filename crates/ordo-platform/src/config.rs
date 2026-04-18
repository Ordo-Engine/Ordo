//! Platform server configuration

use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Ordo Platform Server
///
/// Manages organizations, projects, members, and provides an authenticated proxy
/// to ordo-server for rule engine operations.
///
/// # Environment Variables
///
/// | Variable | Description | Default |
/// |----------|-------------|---------|
/// | `ORDO_PLATFORM_ADDR` | Listen address | `0.0.0.0:3000` |
/// | `ORDO_DATABASE_URL` | PostgreSQL connection URL | required |
/// | `ORDO_ENGINE_URL` | ordo-server base URL | `http://localhost:8080` |
/// | `ORDO_NATS_URL` | NATS server URL for platform → server sync | - |
/// | `ORDO_NATS_SUBJECT_PREFIX` | NATS subject prefix | `ordo.rules` |
/// | `ORDO_INSTANCE_ID` | Stable instance ID for NATS publishing | auto |
/// | `ORDO_JWT_SECRET` | JWT signing secret | required |
/// | `ORDO_JWT_EXPIRY_HOURS` | JWT expiry in hours | `24` |
/// | `ORDO_PLATFORM_CORS_ORIGINS` | Allowed CORS origins (comma-separated) | `*` |
/// | `ORDO_LOG_LEVEL` | Log level | `info` |
#[derive(Parser, Debug, Clone)]
#[command(name = "ordo-platform")]
#[command(author, version, about, long_about = None)]
pub struct PlatformConfig {
    /// Listen address for the platform HTTP server
    #[arg(
        long = "addr",
        default_value = "0.0.0.0:3000",
        env = "ORDO_PLATFORM_ADDR"
    )]
    pub listen_addr: SocketAddr,

    /// PostgreSQL connection URL (e.g. postgresql://user:pass@host/db)
    #[arg(long = "database-url", env = "ORDO_DATABASE_URL")]
    pub database_url: String,

    /// ordo-server base URL for engine API proxy
    #[arg(
        long = "engine-url",
        default_value = "http://localhost:8080",
        env = "ORDO_ENGINE_URL"
    )]
    pub engine_url: String,

    /// NATS server URL used to publish tenant and ruleset sync events.
    #[arg(long = "nats-url", env = "ORDO_NATS_URL")]
    pub nats_url: Option<String>,

    /// NATS subject prefix for sync events.
    #[arg(
        long = "nats-subject-prefix",
        default_value = "ordo.rules",
        env = "ORDO_NATS_SUBJECT_PREFIX"
    )]
    pub nats_subject_prefix: String,

    /// Unique instance identifier used in sync message envelopes.
    #[arg(long = "instance-id", env = "ORDO_INSTANCE_ID")]
    pub instance_id: Option<String>,

    /// JWT signing secret (use a strong random value in production)
    #[arg(long = "jwt-secret", env = "ORDO_JWT_SECRET")]
    pub jwt_secret: String,

    /// JWT token expiry in hours
    #[arg(
        long = "jwt-expiry-hours",
        default_value = "24",
        env = "ORDO_JWT_EXPIRY_HOURS"
    )]
    pub jwt_expiry_hours: u64,

    /// Allowed CORS origins (comma-separated). Use `*` to allow all.
    #[arg(
        long = "cors-origins",
        default_value = "*",
        env = "ORDO_PLATFORM_CORS_ORIGINS",
        value_delimiter = ','
    )]
    pub cors_allowed_origins: Vec<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "ORDO_LOG_LEVEL")]
    pub log_level: String,

    /// Directory containing rule templates (manifest.json + per-template dirs).
    /// If the path does not exist, the template system is disabled gracefully.
    #[arg(
        long = "templates-dir",
        default_value = "./templates",
        env = "ORDO_PLATFORM_TEMPLATES_DIR"
    )]
    pub templates_dir: PathBuf,

    // ── GitHub OAuth ─────────────────────────────────────────────────────────

    /// GitHub OAuth App client ID (register at github.com/settings/developers)
    #[arg(long = "github-client-id", env = "ORDO_GITHUB_CLIENT_ID")]
    pub github_client_id: Option<String>,

    /// GitHub OAuth App client secret (keep this out of logs and version control)
    #[arg(long = "github-client-secret", env = "ORDO_GITHUB_CLIENT_SECRET")]
    pub github_client_secret: Option<String>,

    /// Full callback URL registered in your GitHub OAuth App.
    /// Must match exactly: e.g. https://platform.example.com/api/v1/github/callback
    #[arg(
        long = "github-callback-url",
        default_value = "http://localhost:3000/api/v1/github/callback",
        env = "ORDO_GITHUB_CALLBACK_URL"
    )]
    pub github_callback_url: String,
}

impl PlatformConfig {
    pub fn nats_enabled(&self) -> bool {
        self.nats_url.is_some()
    }

    pub fn resolve_instance_id(&self) -> String {
        if let Some(ref id) = self.instance_id {
            return id.clone();
        }

        if let Ok(hostname) = hostname::get() {
            let hostname = hostname.to_string_lossy().to_string();
            if !hostname.is_empty() {
                return format!("{}:{}", hostname, self.listen_addr.port());
            }
        }

        format!("platform-{:016x}", rand::random::<u64>())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.jwt_secret.len() < 32 {
            return Err("ORDO_JWT_SECRET must be at least 32 characters".to_string());
        }
        if self.database_url.is_empty() {
            return Err("ORDO_DATABASE_URL must not be empty".to_string());
        }
        Ok(())
    }
}
