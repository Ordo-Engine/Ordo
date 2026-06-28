use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::Role;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgRole {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRoleAssignment {
    pub user_id: String,
    pub org_id: String,
    pub role_id: String,
    pub role_name: Option<String>,
    pub assigned_at: DateTime<Utc>,
    pub assigned_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignRoleRequest {
    pub role_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MemberWithRoles {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub roles: Vec<UserRoleAssignment>,
    pub invited_at: DateTime<Utc>,
    pub invited_by: String,
}
