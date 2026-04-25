use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Online,
    Offline,
    Degraded,
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerStatus::Online => write!(f, "online"),
            ServerStatus::Offline => write!(f, "offline"),
            ServerStatus::Degraded => write!(f, "degraded"),
        }
    }
}

impl std::str::FromStr for ServerStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "online" => Ok(ServerStatus::Online),
            "offline" => Ok(ServerStatus::Offline),
            "degraded" => Ok(ServerStatus::Degraded),
            other => Err(format!("invalid server status: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerNode {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(skip_serializing)]
    pub token: String,
    pub org_id: Option<String>,
    pub labels: JsonValue,
    pub version: Option<String>,
    pub status: ServerStatus,
    pub last_seen: Option<DateTime<Utc>>,
    pub registered_at: DateTime<Utc>,
    pub capabilities: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub url: String,
    pub org_id: Option<String>,
    pub labels: JsonValue,
    pub version: Option<String>,
    pub status: ServerStatus,
    pub last_seen: Option<DateTime<Utc>>,
    pub registered_at: DateTime<Utc>,
    pub capabilities: JsonValue,
}

impl From<ServerNode> for ServerInfo {
    fn from(s: ServerNode) -> Self {
        Self {
            id: s.id,
            name: s.name,
            url: s.url,
            org_id: s.org_id,
            labels: s.labels,
            version: s.version,
            status: s.status,
            last_seen: s.last_seen,
            registered_at: s.registered_at,
            capabilities: s.capabilities,
        }
    }
}

pub fn normalize_server_url(server_url: &str) -> anyhow::Result<String> {
    let parsed = Url::parse(server_url.trim())
        .map_err(|e| anyhow::anyhow!("invalid server url '{}': {}", server_url, e))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("server url '{}' is missing a host", server_url))?
        .to_ascii_lowercase();
    let port = parsed.port_or_known_default().ok_or_else(|| {
        anyhow::anyhow!(
            "server url '{}' is missing an explicit or default port",
            server_url
        )
    })?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    let authority = if host.contains(':') {
        format!("[{}]:{}", host, port)
    } else {
        format!("{}:{}", host, port)
    };
    Ok(format!("{}://{}", scheme, authority))
}

pub fn derive_server_id(server_url: &str) -> anyhow::Result<String> {
    let normalized = normalize_server_url(server_url)?;
    let digest = Sha256::digest(normalized.as_bytes());
    Ok(format!("srv_{}", &hex::encode(digest)[..32]))
}
