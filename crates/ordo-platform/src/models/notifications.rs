use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformNotification {
    pub id: String,
    pub org_id: String,
    pub user_id: String,
    #[serde(rename = "type")]
    pub notif_type: String,
    pub ref_id: Option<String>,
    pub ref_type: Option<String>,
    pub payload: serde_json::Value,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationCount {
    pub unread: i64,
}
