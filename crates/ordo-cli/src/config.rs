//! Global CLI config — auth token + API URL, stored at `~/.ordo/config.toml`
//! (chmod 600). Env vars `ORDO_TOKEN` / `ORDO_API_URL` override the file, so CI
//! never needs a config file.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_API_URL: &str = "https://api.ordoengine.com/api/v1";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,
}

fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    Ok(home.join(".ordo").join("config.toml"))
}

impl Config {
    pub fn load() -> Result<Config> {
        let path = config_path()?;
        if !path.is_file() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("invalid {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        restrict_permissions(&path);
        Ok(())
    }
}

/// Auth token: `ORDO_TOKEN` env overrides the config file.
pub fn resolve_token(cfg: &Config) -> Option<String> {
    std::env::var("ORDO_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| cfg.token.clone())
}

/// API base URL: `ORDO_API_URL` env > project `ordo.yaml` > config file > default.
pub fn resolve_api_url(cfg: &Config, project_api_url: Option<&str>) -> String {
    std::env::var("ORDO_API_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| project_api_url.map(String::from))
        .or_else(|| cfg.api_url.clone())
        .unwrap_or_else(|| DEFAULT_API_URL.to_string())
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) {}
