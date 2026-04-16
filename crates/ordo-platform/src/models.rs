//! Platform data models — User, Organization, Member, Project, Role
//!
//! Maps to ordo-book governance framework:
//! - Organization = decision governance unit (Ch13)
//! - Project = decision domain (Ch7-10)
//! - Role = governance roles (Ch13 + Ch15)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Role ─────────────────────────────────────────────────────────────────────

/// Four-level role hierarchy mapping ordo-book governance roles:
/// - Owner   → Decision Owner (Ch15)
/// - Admin   → Rule Reviewer + Data Steward (Ch13)
/// - Editor  → Rule Author (Ch13)
/// - Viewer  → Governor / auditor (Ch13)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer = 0,
    Editor = 1,
    Admin = 2,
    Owner = 3,
}

impl Role {
    pub fn can_edit_rules(&self) -> bool {
        *self >= Role::Editor
    }
    pub fn can_manage_members(&self) -> bool {
        *self >= Role::Admin
    }
    pub fn can_manage_org(&self) -> bool {
        *self >= Role::Admin
    }
    pub fn can_approve_changes(&self) -> bool {
        *self >= Role::Admin
    }
    pub fn can_publish(&self) -> bool {
        *self >= Role::Admin
    }
    pub fn can_rollback(&self) -> bool {
        *self >= Role::Admin
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Owner => write!(f, "owner"),
            Role::Admin => write!(f, "admin"),
            Role::Editor => write!(f, "editor"),
            Role::Viewer => write!(f, "viewer"),
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "owner" => Ok(Role::Owner),
            "admin" => Ok(Role::Admin),
            "editor" => Ok(Role::Editor),
            "viewer" => Ok(Role::Viewer),
            other => Err(format!(
                "invalid role '{}', expected: owner, admin, editor, viewer",
                other
            )),
        }
    }
}

// ── User ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
}

/// Public user info (no password hash)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
}

impl From<&User> for UserInfo {
    fn from(u: &User) -> Self {
        Self {
            id: u.id.clone(),
            email: u.email.clone(),
            display_name: u.display_name.clone(),
            created_at: u.created_at,
            last_login: u.last_login,
        }
    }
}

// ── Organization ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub members: Vec<Member>,
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

// ── Project (= Decision Domain, Ch7-10) ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Project ID — also used as tenant_id in ordo-server
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub org_id: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
}

// ── JWT Claims ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub email: String,
    pub exp: usize, // expiry timestamp
    pub iat: usize, // issued at
}
