use super::Role;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub members: Vec<Member>,
    #[serde(default)]
    pub parent_org_id: Option<String>,
    #[serde(default)]
    pub depth: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub invited_at: DateTime<Utc>,
    pub invited_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub org_id: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    #[serde(default)]
    pub server_id: Option<String>,
}
