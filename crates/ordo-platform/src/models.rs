//! Platform data models — User, Organization, Member, Project, Role
//!
//! Maps to ordo-book governance framework:
//! - Organization = decision governance unit (Ch13)
//! - Project = decision domain (Ch7-10)
//! - Role = governance roles (Ch13 + Ch15)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

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

// ── Ruleset Change History ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RulesetHistorySource {
    Sync,
    Edit,
    Save,
    Restore,
    Create,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesetHistoryEntry {
    pub id: String,
    pub ruleset_name: String,
    pub action: String,
    pub source: RulesetHistorySource,
    pub created_at: DateTime<Utc>,
    pub author_id: String,
    pub author_email: String,
    pub author_display_name: String,
    pub snapshot: JsonValue,
}

// ── Fact Catalog (ordo-book Ch7 事实五元组) ──────────────────────────────────

/// Data type for facts and concepts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FactDataType {
    String,
    Number,
    Boolean,
    Date,
    Object,
}

/// Null handling policy for a fact field
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NullPolicy {
    /// Treat null as an error
    Error,
    /// Use a default value
    Default,
    /// Skip the rule if null
    Skip,
}

/// A registered fact — raw input field that rules consume (Ch7)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactDefinition {
    /// Dotted field path, e.g. "user.age"
    pub name: String,
    pub data_type: FactDataType,
    /// Where this fact comes from (e.g. "user-service API")
    pub source: String,
    /// Typical fetch latency in ms (acquisition cost)
    pub latency_ms: Option<u32>,
    pub null_policy: NullPolicy,
    pub description: Option<String>,
    /// Responsible team or person
    pub owner: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Concept Registry (ordo-book Ch7 派生字段 DAG) ─────────────────────────────

/// A derived concept — computed from facts/other concepts via an expression (Ch7)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptDefinition {
    /// Concept name, e.g. "risk_score"
    pub name: String,
    pub data_type: FactDataType,
    /// Expression string referencing facts and other concepts
    pub expression: String,
    /// Names of facts/concepts this concept depends on (for DAG + cycle detection)
    pub dependencies: Vec<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Decision Contract (ordo-book Ch13 契约维度) ───────────────────────────────

/// A field in a decision contract (input or output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractField {
    pub name: String,
    pub data_type: FactDataType,
    pub required: bool,
    pub description: Option<String>,
}

/// Formal input/output contract for a ruleset (Ch13)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContract {
    /// Name of the ruleset this contract covers
    pub ruleset_name: String,
    /// Version pattern, e.g. "1.x"
    pub version_pattern: String,
    /// Responsible team
    pub owner: String,
    /// P99 latency SLA in milliseconds
    pub sla_p99_ms: Option<u32>,
    pub input_fields: Vec<ContractField>,
    pub output_fields: Vec<ContractField>,
    pub notes: Option<String>,
    pub updated_at: DateTime<Utc>,
}

// ── Templates (M1.1: rule templates) ─────────────────────────────────────────

/// Metadata for a single template (listed in manifest.json).
///
/// String fields may contain `i18n:<key>` references; the template API layer
/// resolves them against the locale files before returning to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMetadata {
    /// Stable identifier (directory name), e.g. `ecommerce-coupon`
    pub id: String,
    /// Display name (i18n key)
    pub name: String,
    /// One-line description (i18n key)
    pub description: String,
    /// Tag labels (i18n keys) — e.g. e-commerce, decision-table
    #[serde(default)]
    pub tags: Vec<String>,
    /// Lucide icon name (not i18n)
    #[serde(default)]
    pub icon: Option<String>,
    /// "beginner" | "intermediate" | "advanced"
    pub difficulty: String,
    /// Short feature labels (i18n keys) highlighted on the template card
    #[serde(default)]
    pub features: Vec<String>,
}

/// One example input that can be run against the template's ruleset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSample {
    /// Human-readable label (i18n key)
    pub label: String,
    /// Input payload passed to the ruleset's execute endpoint
    pub input: JsonValue,
    /// Expected outcome description (i18n key), rendered in the UI
    #[serde(default)]
    pub expected_result: Option<String>,
}

/// Full detail of a template — returned by `GET /api/v1/templates/:id`
/// and consumed by `POST /api/v1/orgs/:oid/projects/from-template`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDetail {
    #[serde(flatten)]
    pub metadata: TemplateMetadata,
    pub facts: Vec<FactDefinition>,
    pub concepts: Vec<ConceptDefinition>,
    /// Raw RuleSet JSON — handed to the engine unmodified (after tenant_id rewrite).
    pub ruleset: JsonValue,
    pub samples: Vec<TemplateSample>,
    /// Pre-built decision contract for this ruleset (optional — not all templates include one).
    #[serde(default)]
    pub contract: Option<DecisionContract>,
    /// Bundled test cases (optional — applied to the new project on from-template clone).
    #[serde(default)]
    pub tests: Vec<TestCase>,
}

// ── Test Cases (M1.2: rule testing system) ───────────────────────────────────

/// The assertion side of a test case.
/// All fields are optional — only supplied fields are checked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExpectation {
    /// Expected result code (exact match)
    #[serde(default)]
    pub code: Option<String>,
    /// Expected result message (exact match, optional)
    #[serde(default)]
    pub message: Option<String>,
    /// Expected output fields (per-field comparison — only listed keys are checked)
    #[serde(default)]
    pub output: Option<JsonValue>,
}

/// A single test case for a ruleset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Input payload sent to the engine's execute endpoint
    pub input: JsonValue,
    pub expect: TestExpectation,
    /// Free-form tags for filtering (e.g. ["vip", "happy-path"])
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
}

/// Result of running one test case against the engine.
/// Not persisted — held in memory by the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunResult {
    pub test_id: String,
    pub test_name: String,
    pub passed: bool,
    /// Human-readable description of each assertion failure
    #[serde(default)]
    pub failures: Vec<String>,
    pub duration_us: u64,
    #[serde(default)]
    pub actual_code: Option<String>,
    #[serde(default)]
    pub actual_output: Option<JsonValue>,
}

/// Aggregated results for all rulesets in a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTestRunResult {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub rulesets: Vec<RulesetTestSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesetTestSummary {
    pub ruleset_name: String,
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub results: Vec<TestRunResult>,
}

// ── JWT Claims ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub email: String,
    pub exp: usize, // expiry timestamp
    pub iat: usize, // issued at
}
