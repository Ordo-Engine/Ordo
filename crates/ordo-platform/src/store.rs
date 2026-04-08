//! JSON file-based persistence for platform data.
//!
//! Storage layout:
//!   {platform_dir}/users/{user_id}.json
//!   {platform_dir}/orgs/{org_id}.json      — org + members
//!   {platform_dir}/orgs/{org_id}/projects/{project_id}.json

use crate::models::{
    ConceptDefinition, DecisionContract, FactDefinition, Member, Organization, Project, Role,
    RulesetHistoryEntry, TestCase, User,
};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Clone)]
pub struct PlatformStore {
    root: PathBuf,
}

impl PlatformStore {
    pub async fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(root.join("users")).await?;
        fs::create_dir_all(root.join("orgs")).await?;
        Ok(Self { root })
    }

    // ── Users ─────────────────────────────────────────────────────────────────

    pub async fn save_user(&self, user: &User) -> Result<()> {
        let path = self.user_path(&user.id);
        write_json(&path, user).await
    }

    pub async fn get_user(&self, id: &str) -> Result<Option<User>> {
        read_json_opt(self.user_path(id)).await
    }

    pub async fn find_user_by_email(&self, email: &str) -> Result<Option<User>> {
        let dir = self.root.join("users");
        let mut entries = fs::read_dir(&dir).await?;
        let email_lower = email.to_lowercase();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(user) = read_json::<User>(&path).await {
                if user.email.to_lowercase() == email_lower {
                    return Ok(Some(user));
                }
            }
        }
        Ok(None)
    }

    pub async fn update_user(&self, user: &User) -> Result<()> {
        self.save_user(user).await
    }

    // ── Organizations ─────────────────────────────────────────────────────────

    pub async fn save_org(&self, org: &Organization) -> Result<()> {
        let path = self.org_path(&org.id);
        fs::create_dir_all(path.parent().unwrap()).await?;
        write_json(&path, org).await
    }

    pub async fn get_org(&self, id: &str) -> Result<Option<Organization>> {
        read_json_opt(self.org_path(id)).await
    }

    /// List all orgs where the given user is a member.
    pub async fn list_user_orgs(&self, user_id: &str) -> Result<Vec<Organization>> {
        let dir = self.root.join("orgs");
        let mut result = Vec::new();
        let mut entries = match fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => return Ok(result),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            // Each org is stored as orgs/{org_id}.json (not in a subdirectory)
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(org) = read_json::<Organization>(&path).await {
                    if org.members.iter().any(|m| m.user_id == user_id) {
                        result.push(org);
                    }
                }
            }
        }
        Ok(result)
    }

    pub async fn delete_org(&self, id: &str) -> Result<bool> {
        let path = self.org_path(id);
        if path.exists() {
            fs::remove_file(&path).await?;
            // Remove projects directory
            let projects_dir = self.org_projects_dir(id);
            if projects_dir.exists() {
                fs::remove_dir_all(&projects_dir).await?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // ── Members (stored inside org JSON) ─────────────────────────────────────

    pub async fn add_member(&self, org_id: &str, member: Member) -> Result<()> {
        let mut org = self
            .get_org(org_id)
            .await?
            .context("organization not found")?;
        // Remove existing entry for this user if any
        org.members.retain(|m| m.user_id != member.user_id);
        org.members.push(member);
        self.save_org(&org).await
    }

    pub async fn update_member_role(
        &self,
        org_id: &str,
        user_id: &str,
        role: Role,
    ) -> Result<bool> {
        let mut org = self
            .get_org(org_id)
            .await?
            .context("organization not found")?;
        if let Some(m) = org.members.iter_mut().find(|m| m.user_id == user_id) {
            m.role = role;
            self.save_org(&org).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn remove_member(&self, org_id: &str, user_id: &str) -> Result<bool> {
        let mut org = self
            .get_org(org_id)
            .await?
            .context("organization not found")?;
        let before = org.members.len();
        org.members.retain(|m| m.user_id != user_id);
        if org.members.len() < before {
            self.save_org(&org).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // ── Projects ──────────────────────────────────────────────────────────────

    pub async fn save_project(&self, project: &Project) -> Result<()> {
        let dir = self.org_projects_dir(&project.org_id);
        fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{}.json", project.id));
        write_json(&path, project).await
    }

    pub async fn get_project(&self, org_id: &str, project_id: &str) -> Result<Option<Project>> {
        let path = self
            .org_projects_dir(org_id)
            .join(format!("{}.json", project_id));
        read_json_opt(path).await
    }

    pub async fn list_projects(&self, org_id: &str) -> Result<Vec<Project>> {
        let dir = self.org_projects_dir(org_id);
        let mut result = Vec::new();
        let mut entries = match fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => return Ok(result),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(project) = read_json::<Project>(&path).await {
                    result.push(project);
                }
            }
        }
        Ok(result)
    }

    pub async fn delete_project(&self, org_id: &str, project_id: &str) -> Result<bool> {
        let path = self
            .org_projects_dir(org_id)
            .join(format!("{}.json", project_id));
        if path.exists() {
            fs::remove_file(&path).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // ── Fact Catalog ──────────────────────────────────────────────────────────

    pub async fn get_facts(
        &self,
        org_id: &str,
        project_id: &str,
    ) -> Result<Vec<FactDefinition>> {
        let path = self.project_asset_path(org_id, project_id, "facts");
        Ok(read_json_opt::<Vec<FactDefinition>>(path)
            .await?
            .unwrap_or_default())
    }

    pub async fn save_facts(
        &self,
        org_id: &str,
        project_id: &str,
        facts: &[FactDefinition],
    ) -> Result<()> {
        let path = self.project_asset_path(org_id, project_id, "facts");
        write_json(&path, facts).await
    }

    // ── Concept Registry ──────────────────────────────────────────────────────

    pub async fn get_concepts(
        &self,
        org_id: &str,
        project_id: &str,
    ) -> Result<Vec<ConceptDefinition>> {
        let path = self.project_asset_path(org_id, project_id, "concepts");
        Ok(read_json_opt::<Vec<ConceptDefinition>>(path)
            .await?
            .unwrap_or_default())
    }

    pub async fn save_concepts(
        &self,
        org_id: &str,
        project_id: &str,
        concepts: &[ConceptDefinition],
    ) -> Result<()> {
        let path = self.project_asset_path(org_id, project_id, "concepts");
        write_json(&path, concepts).await
    }

    // ── Decision Contracts ────────────────────────────────────────────────────

    pub async fn get_contracts(
        &self,
        org_id: &str,
        project_id: &str,
    ) -> Result<Vec<DecisionContract>> {
        let path = self.project_asset_path(org_id, project_id, "contracts");
        Ok(read_json_opt::<Vec<DecisionContract>>(path)
            .await?
            .unwrap_or_default())
    }

    pub async fn save_contracts(
        &self,
        org_id: &str,
        project_id: &str,
        contracts: &[DecisionContract],
    ) -> Result<()> {
        let path = self.project_asset_path(org_id, project_id, "contracts");
        write_json(&path, contracts).await
    }

    // ── Ruleset Change History ───────────────────────────────────────────────

    pub async fn get_ruleset_history(
        &self,
        org_id: &str,
        project_id: &str,
        ruleset_name: &str,
    ) -> Result<Vec<RulesetHistoryEntry>> {
        let path = self.ruleset_history_path(org_id, project_id, ruleset_name);
        Ok(read_json_opt::<Vec<RulesetHistoryEntry>>(path)
            .await?
            .unwrap_or_default())
    }

    pub async fn append_ruleset_history(
        &self,
        org_id: &str,
        project_id: &str,
        ruleset_name: &str,
        entries: &[RulesetHistoryEntry],
    ) -> Result<Vec<RulesetHistoryEntry>> {
        let mut history = self
            .get_ruleset_history(org_id, project_id, ruleset_name)
            .await?;

        for entry in entries {
            if history.iter().any(|existing| existing.id == entry.id) {
                continue;
            }
            history.push(entry.clone());
        }

        const MAX_RULESET_HISTORY_ENTRIES: usize = 500;
        if history.len() > MAX_RULESET_HISTORY_ENTRIES {
            let start = history.len() - MAX_RULESET_HISTORY_ENTRIES;
            history = history.split_off(start);
        }

        let path = self.ruleset_history_path(org_id, project_id, ruleset_name);
        write_json(&path, &history).await?;
        Ok(history)
    }

    // ── Test Cases ────────────────────────────────────────────────────────────

    pub async fn get_tests(
        &self,
        org_id: &str,
        project_id: &str,
        ruleset_name: &str,
    ) -> Result<Vec<TestCase>> {
        let path = self.test_cases_path(org_id, project_id, ruleset_name);
        Ok(read_json_opt::<Vec<TestCase>>(path)
            .await?
            .unwrap_or_default())
    }

    pub async fn save_tests(
        &self,
        org_id: &str,
        project_id: &str,
        ruleset_name: &str,
        tests: &[TestCase],
    ) -> Result<()> {
        let path = self.test_cases_path(org_id, project_id, ruleset_name);
        write_json(&path, tests).await
    }

    fn test_cases_path(&self, org_id: &str, project_id: &str, ruleset_name: &str) -> PathBuf {
        let safe_name: String = ruleset_name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        self.project_asset_path(org_id, project_id, &format!("tests_{}", safe_name))
    }

    // ── Paths ─────────────────────────────────────────────────────────────────

    fn user_path(&self, user_id: &str) -> PathBuf {
        self.root.join("users").join(format!("{}.json", user_id))
    }

    fn org_path(&self, org_id: &str) -> PathBuf {
        self.root.join("orgs").join(format!("{}.json", org_id))
    }

    fn org_projects_dir(&self, org_id: &str) -> PathBuf {
        self.root.join("orgs").join(org_id).join("projects")
    }

    /// Public accessor for the projects directory (used by testing.rs for discovery).
    pub fn org_projects_dir_pub(&self, org_id: &str) -> PathBuf {
        self.org_projects_dir(org_id)
    }

    /// Path for a project-scoped asset file: `projects/{pid}_{asset}.json`
    fn project_asset_path(&self, org_id: &str, project_id: &str, asset: &str) -> PathBuf {
        self.org_projects_dir(org_id)
            .join(format!("{}_{}.json", project_id, asset))
    }

    fn ruleset_history_path(&self, org_id: &str, project_id: &str, ruleset_name: &str) -> PathBuf {
        let safe_ruleset_name: String = ruleset_name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();

        self.project_asset_path(
            org_id,
            project_id,
            &format!("ruleset_history_{}", safe_ruleset_name),
        )
    }
}

// ── JSON helpers ─────────────────────────────────────────────────────────────

async fn write_json<T: serde::Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    // Atomic write: write to temp file, then rename
    let tmp = path.with_extension("json.tmp");
    if let Some(parent) = tmp.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(&tmp, json.as_bytes()).await?;
    fs::rename(&tmp, path).await?;
    Ok(())
}

async fn read_json<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    let data = fs::read(path.as_ref()).await?;
    Ok(serde_json::from_slice(&data)?)
}

async fn read_json_opt<T: serde::de::DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<Option<T>> {
    match fs::read(path.as_ref()).await {
        Ok(data) => Ok(Some(serde_json::from_slice(&data)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
