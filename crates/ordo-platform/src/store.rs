//! PostgreSQL-backed persistence for platform data.

use crate::models::{
    ConceptDefinition, ContractField, CreateEnvironmentRequest, CreateRoleRequest,
    DecisionContract, DeploymentStatus, FactDataType, FactDefinition, Member, NullPolicy, OrgRole,
    Organization, Project, ProjectEnvironment, ProjectRuleset, ProjectRulesetMeta, Role,
    RulesetDeployment, RulesetHistoryEntry, RulesetHistorySource, ServerNode, ServerStatus,
    TestCase, TestExpectation, UpdateEnvironmentRequest, UpdateRoleRequest, User,
    UserRoleAssignment,
};
use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};

#[derive(Clone)]
pub struct PlatformStore {
    pool: PgPool,
}

impl PlatformStore {
    pub async fn new(pool: PgPool) -> Result<Self> {
        Ok(Self { pool })
    }

    // ── Users ─────────────────────────────────────────────────────────────────

    pub async fn save_user(&self, user: &User) -> Result<()> {
        sqlx::query(
            "INSERT INTO users (id, email, password_hash, display_name, created_at, last_login)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (id) DO UPDATE SET
               email = EXCLUDED.email,
               password_hash = EXCLUDED.password_hash,
               display_name = EXCLUDED.display_name,
               last_login = EXCLUDED.last_login",
        )
        .bind(&user.id)
        .bind(&user.email)
        .bind(&user.password_hash)
        .bind(&user.display_name)
        .bind(user.created_at)
        .bind(user.last_login)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_user(&self, id: &str) -> Result<Option<User>> {
        let row = sqlx::query(
            "SELECT id, email, password_hash, display_name, created_at, last_login
             FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| row_to_user(&r)))
    }

    pub async fn find_user_by_email(&self, email: &str) -> Result<Option<User>> {
        let row = sqlx::query(
            "SELECT id, email, password_hash, display_name, created_at, last_login
             FROM users WHERE LOWER(email) = LOWER($1)",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| row_to_user(&r)))
    }

    pub async fn update_user(&self, user: &User) -> Result<()> {
        self.save_user(user).await
    }

    // ── Organizations ─────────────────────────────────────────────────────────

    pub async fn save_org(&self, org: &Organization) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO organizations (id, name, description, created_at, created_by)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name,
               description = EXCLUDED.description",
        )
        .bind(&org.id)
        .bind(&org.name)
        .bind(&org.description)
        .bind(org.created_at)
        .bind(&org.created_by)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM members WHERE org_id = $1")
            .bind(&org.id)
            .execute(&mut *tx)
            .await?;

        for member in &org.members {
            sqlx::query(
                "INSERT INTO members (org_id, user_id, email, display_name, role, invited_at, invited_by)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&org.id)
            .bind(&member.user_id)
            .bind(&member.email)
            .bind(&member.display_name)
            .bind(role_to_str(&member.role))
            .bind(member.invited_at)
            .bind(&member.invited_by)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_org(&self, id: &str) -> Result<Option<Organization>> {
        let row = sqlx::query(
            "SELECT id, name, description, created_at, created_by
             FROM organizations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let members = self.fetch_members(id).await?;
        Ok(Some(Organization {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            created_at: row.get("created_at"),
            created_by: row.get("created_by"),
            members,
        }))
    }

    pub async fn list_user_orgs(&self, user_id: &str) -> Result<Vec<Organization>> {
        let rows = sqlx::query(
            "SELECT o.id, o.name, o.description, o.created_at, o.created_by
             FROM organizations o
             JOIN members m ON o.id = m.org_id
             WHERE m.user_id = $1
             ORDER BY o.created_at",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let mut orgs = Vec::new();
        for row in rows {
            let org_id: String = row.get("id");
            let members = self.fetch_members(&org_id).await?;
            orgs.push(Organization {
                id: org_id,
                name: row.get("name"),
                description: row.get("description"),
                created_at: row.get("created_at"),
                created_by: row.get("created_by"),
                members,
            });
        }
        Ok(orgs)
    }

    pub async fn delete_org(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM organizations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn fetch_members(&self, org_id: &str) -> Result<Vec<Member>> {
        let rows = sqlx::query(
            "SELECT user_id, email, display_name, role, invited_at, invited_by
             FROM members WHERE org_id = $1 ORDER BY invited_at",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let role_str: String = row.get("role");
                Ok(Member {
                    user_id: row.get("user_id"),
                    email: row.get("email"),
                    display_name: row.get("display_name"),
                    role: str_to_role(&role_str)?,
                    invited_at: row.get("invited_at"),
                    invited_by: row.get("invited_by"),
                })
            })
            .collect()
    }

    // ── Members ────────────────────────────────────────────────────────────────

    pub async fn add_member(&self, org_id: &str, member: Member) -> Result<()> {
        sqlx::query(
            "INSERT INTO members (org_id, user_id, email, display_name, role, invited_at, invited_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (org_id, user_id) DO UPDATE SET
               email = EXCLUDED.email,
               display_name = EXCLUDED.display_name,
               role = EXCLUDED.role,
               invited_at = EXCLUDED.invited_at,
               invited_by = EXCLUDED.invited_by",
        )
        .bind(org_id)
        .bind(&member.user_id)
        .bind(&member.email)
        .bind(&member.display_name)
        .bind(role_to_str(&member.role))
        .bind(member.invited_at)
        .bind(&member.invited_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_member_role(
        &self,
        org_id: &str,
        user_id: &str,
        role: Role,
    ) -> Result<bool> {
        let result = sqlx::query("UPDATE members SET role = $1 WHERE org_id = $2 AND user_id = $3")
            .bind(role_to_str(&role))
            .bind(org_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn remove_member(&self, org_id: &str, user_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM members WHERE org_id = $1 AND user_id = $2")
            .bind(org_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── Projects ──────────────────────────────────────────────────────────────

    pub async fn save_project(&self, project: &Project) -> Result<()> {
        sqlx::query(
            "INSERT INTO projects (id, name, description, org_id, created_at, created_by, server_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name,
               description = EXCLUDED.description,
               server_id = EXCLUDED.server_id",
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.description)
        .bind(&project.org_id)
        .bind(project.created_at)
        .bind(&project.created_by)
        .bind(&project.server_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_project(&self, org_id: &str, project_id: &str) -> Result<Option<Project>> {
        let row = sqlx::query(
            "SELECT id, name, description, org_id, created_at, created_by, server_id
             FROM projects WHERE id = $1 AND org_id = $2",
        )
        .bind(project_id)
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| row_to_project(&r)))
    }

    pub async fn list_projects(&self, org_id: &str) -> Result<Vec<Project>> {
        let rows = sqlx::query(
            "SELECT id, name, description, org_id, created_at, created_by, server_id
             FROM projects WHERE org_id = $1 ORDER BY created_at",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_project).collect())
    }

    pub async fn delete_project(&self, org_id: &str, project_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM projects WHERE id = $1 AND org_id = $2")
            .bind(project_id)
            .bind(org_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn bind_project_server(
        &self,
        org_id: &str,
        project_id: &str,
        server_id: Option<&str>,
    ) -> Result<bool> {
        let result =
            sqlx::query("UPDATE projects SET server_id = $1 WHERE id = $2 AND org_id = $3")
                .bind(server_id)
                .bind(project_id)
                .bind(org_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── Fact Catalog ──────────────────────────────────────────────────────────

    pub async fn get_facts(&self, _org_id: &str, project_id: &str) -> Result<Vec<FactDefinition>> {
        let rows = sqlx::query(
            "SELECT name, data_type, source, latency_ms, null_policy, description, owner, created_at, updated_at
             FROM facts WHERE project_id = $1 ORDER BY name",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                let dt_str: String = r.get("data_type");
                let np_str: String = r.get("null_policy");
                let latency_ms: Option<i32> = r.get("latency_ms");
                Ok(FactDefinition {
                    name: r.get("name"),
                    data_type: str_to_fact_data_type(&dt_str)?,
                    source: r.get("source"),
                    latency_ms: latency_ms.map(|v| v as u32),
                    null_policy: str_to_null_policy(&np_str)?,
                    description: r.get("description"),
                    owner: r.get("owner"),
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                })
            })
            .collect()
    }

    pub async fn save_facts(
        &self,
        _org_id: &str,
        project_id: &str,
        facts: &[FactDefinition],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM facts WHERE project_id = $1")
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        for fact in facts {
            sqlx::query(
                "INSERT INTO facts (project_id, name, data_type, source, latency_ms, null_policy, description, owner, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(project_id)
            .bind(&fact.name)
            .bind(fact_data_type_to_str(&fact.data_type))
            .bind(&fact.source)
            .bind(fact.latency_ms.map(|v| v as i32))
            .bind(null_policy_to_str(&fact.null_policy))
            .bind(&fact.description)
            .bind(&fact.owner)
            .bind(fact.created_at)
            .bind(fact.updated_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // ── Concept Registry ──────────────────────────────────────────────────────

    pub async fn get_concepts(
        &self,
        _org_id: &str,
        project_id: &str,
    ) -> Result<Vec<ConceptDefinition>> {
        let rows = sqlx::query(
            "SELECT name, data_type, expression, dependencies, description, created_at, updated_at
             FROM concepts WHERE project_id = $1 ORDER BY name",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                let dt_str: String = r.get("data_type");
                let deps: sqlx::types::Json<Vec<String>> = r.get("dependencies");
                Ok(ConceptDefinition {
                    name: r.get("name"),
                    data_type: str_to_fact_data_type(&dt_str)?,
                    expression: r.get("expression"),
                    dependencies: deps.0,
                    description: r.get("description"),
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                })
            })
            .collect()
    }

    pub async fn save_concepts(
        &self,
        _org_id: &str,
        project_id: &str,
        concepts: &[ConceptDefinition],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM concepts WHERE project_id = $1")
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        for concept in concepts {
            sqlx::query(
                "INSERT INTO concepts (project_id, name, data_type, expression, dependencies, description, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(project_id)
            .bind(&concept.name)
            .bind(fact_data_type_to_str(&concept.data_type))
            .bind(&concept.expression)
            .bind(sqlx::types::Json(&concept.dependencies))
            .bind(&concept.description)
            .bind(concept.created_at)
            .bind(concept.updated_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // ── Decision Contracts ────────────────────────────────────────────────────

    pub async fn get_contracts(
        &self,
        _org_id: &str,
        project_id: &str,
    ) -> Result<Vec<DecisionContract>> {
        let rows = sqlx::query(
            "SELECT ruleset_name, version_pattern, owner, sla_p99_ms, input_fields, output_fields, notes, updated_at
             FROM contracts WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                let sla: Option<i32> = r.get("sla_p99_ms");
                let input_fields: sqlx::types::Json<Vec<ContractField>> = r.get("input_fields");
                let output_fields: sqlx::types::Json<Vec<ContractField>> = r.get("output_fields");
                Ok(DecisionContract {
                    ruleset_name: r.get("ruleset_name"),
                    version_pattern: r.get("version_pattern"),
                    owner: r.get("owner"),
                    sla_p99_ms: sla.map(|v| v as u32),
                    input_fields: input_fields.0,
                    output_fields: output_fields.0,
                    notes: r.get("notes"),
                    updated_at: r.get("updated_at"),
                })
            })
            .collect()
    }

    pub async fn save_contracts(
        &self,
        _org_id: &str,
        project_id: &str,
        contracts: &[DecisionContract],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM contracts WHERE project_id = $1")
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        for contract in contracts {
            sqlx::query(
                "INSERT INTO contracts (project_id, ruleset_name, version_pattern, owner, sla_p99_ms, input_fields, output_fields, notes, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(project_id)
            .bind(&contract.ruleset_name)
            .bind(&contract.version_pattern)
            .bind(&contract.owner)
            .bind(contract.sla_p99_ms.map(|v| v as i32))
            .bind(sqlx::types::Json(&contract.input_fields))
            .bind(sqlx::types::Json(&contract.output_fields))
            .bind(&contract.notes)
            .bind(contract.updated_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // ── Ruleset Change History ───────────────────────────────────────────────

    pub async fn get_ruleset_history(
        &self,
        _org_id: &str,
        project_id: &str,
        ruleset_name: &str,
    ) -> Result<Vec<RulesetHistoryEntry>> {
        let rows = sqlx::query(
            "SELECT id, ruleset_name, action, source, created_at, author_id, author_email, author_display_name, snapshot
             FROM ruleset_history
             WHERE project_id = $1 AND ruleset_name = $2
             ORDER BY created_at DESC",
        )
        .bind(project_id)
        .bind(ruleset_name)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                let source_str: String = r.get("source");
                let snapshot: sqlx::types::Json<JsonValue> = r.get("snapshot");
                Ok(RulesetHistoryEntry {
                    id: r.get("id"),
                    ruleset_name: r.get("ruleset_name"),
                    action: r.get("action"),
                    source: str_to_history_source(&source_str)?,
                    created_at: r.get("created_at"),
                    author_id: r.get("author_id"),
                    author_email: r.get("author_email"),
                    author_display_name: r.get("author_display_name"),
                    snapshot: snapshot.0,
                })
            })
            .collect()
    }

    pub async fn append_ruleset_history(
        &self,
        org_id: &str,
        project_id: &str,
        ruleset_name: &str,
        entries: &[RulesetHistoryEntry],
    ) -> Result<Vec<RulesetHistoryEntry>> {
        for entry in entries {
            sqlx::query(
                "INSERT INTO ruleset_history
                 (id, project_id, ruleset_name, action, source, created_at, author_id, author_email, author_display_name, snapshot)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(&entry.id)
            .bind(project_id)
            .bind(ruleset_name)
            .bind(&entry.action)
            .bind(history_source_to_str(&entry.source))
            .bind(entry.created_at)
            .bind(&entry.author_id)
            .bind(&entry.author_email)
            .bind(&entry.author_display_name)
            .bind(sqlx::types::Json(&entry.snapshot))
            .execute(&self.pool)
            .await?;
        }
        self.get_ruleset_history(org_id, project_id, ruleset_name)
            .await
    }

    // ── Test Cases ────────────────────────────────────────────────────────────

    pub async fn get_tests(
        &self,
        _org_id: &str,
        project_id: &str,
        ruleset_name: &str,
    ) -> Result<Vec<TestCase>> {
        let rows = sqlx::query(
            "SELECT id, name, description, input, expect, tags, created_at, updated_at, created_by
             FROM test_cases
             WHERE project_id = $1 AND ruleset_name = $2
             ORDER BY created_at",
        )
        .bind(project_id)
        .bind(ruleset_name)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                let input: sqlx::types::Json<JsonValue> = r.get("input");
                let expect: sqlx::types::Json<TestExpectation> = r.get("expect");
                let tags: sqlx::types::Json<Vec<String>> = r.get("tags");
                Ok(TestCase {
                    id: r.get("id"),
                    name: r.get("name"),
                    description: r.get("description"),
                    input: input.0,
                    expect: expect.0,
                    tags: tags.0,
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                    created_by: r.get("created_by"),
                })
            })
            .collect()
    }

    pub async fn save_tests(
        &self,
        _org_id: &str,
        project_id: &str,
        ruleset_name: &str,
        tests: &[TestCase],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM test_cases WHERE project_id = $1 AND ruleset_name = $2")
            .bind(project_id)
            .bind(ruleset_name)
            .execute(&mut *tx)
            .await?;
        for test in tests {
            sqlx::query(
                "INSERT INTO test_cases
                 (id, project_id, ruleset_name, name, description, input, expect, tags, created_at, updated_at, created_by)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            )
            .bind(&test.id)
            .bind(project_id)
            .bind(ruleset_name)
            .bind(&test.name)
            .bind(&test.description)
            .bind(sqlx::types::Json(&test.input))
            .bind(sqlx::types::Json(&test.expect))
            .bind(sqlx::types::Json(&test.tags))
            .bind(test.created_at)
            .bind(test.updated_at)
            .bind(&test.created_by)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// List all ruleset names that have test cases for this project.
    pub async fn list_test_rulesets(&self, project_id: &str) -> Vec<String> {
        sqlx::query(
            "SELECT DISTINCT ruleset_name FROM test_cases WHERE project_id = $1 ORDER BY ruleset_name",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.get("ruleset_name"))
        .collect()
    }

    // ── GitHub connections ────────────────────────────────────────────────────

    pub async fn save_github_connection(
        &self,
        user_id: &str,
        github_user_id: i64,
        login: &str,
        name: Option<&str>,
        avatar_url: Option<&str>,
        access_token: &str,
        scope: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO github_connections
                 (user_id, github_user_id, login, name, avatar_url, access_token, scope, connected_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
             ON CONFLICT (user_id) DO UPDATE SET
                 github_user_id = EXCLUDED.github_user_id,
                 login          = EXCLUDED.login,
                 name           = EXCLUDED.name,
                 avatar_url     = EXCLUDED.avatar_url,
                 access_token   = EXCLUDED.access_token,
                 scope          = EXCLUDED.scope,
                 connected_at   = NOW()",
        )
        .bind(user_id)
        .bind(github_user_id)
        .bind(login)
        .bind(name)
        .bind(avatar_url)
        .bind(access_token)
        .bind(scope)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_github_connection(
        &self,
        user_id: &str,
    ) -> Result<Option<crate::github::GitHubConnectionRow>> {
        let row = sqlx::query(
            "SELECT user_id, github_user_id, login, name, avatar_url, connected_at
             FROM github_connections WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| crate::github::GitHubConnectionRow {
            user_id: r.get("user_id"),
            github_user_id: r.get("github_user_id"),
            login: r.get("login"),
            name: r.get("name"),
            avatar_url: r.get("avatar_url"),
            connected_at: r.get("connected_at"),
        }))
    }

    pub async fn get_github_token(&self, user_id: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT access_token FROM github_connections WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get("access_token")))
    }

    pub async fn delete_github_connection(&self, user_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM github_connections WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Servers ───────────────────────────────────────────────────────────────

    pub async fn upsert_server(&self, server: &ServerNode) -> Result<()> {
        sqlx::query(
            "INSERT INTO servers (id, name, url, token, org_id, labels, version, status, last_seen, registered_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (token) DO UPDATE SET
               name = EXCLUDED.name,
               url = EXCLUDED.url,
               org_id = EXCLUDED.org_id,
               version = EXCLUDED.version,
               status = 'online',
               last_seen = NOW()",
        )
        .bind(&server.id)
        .bind(&server.name)
        .bind(&server.url)
        .bind(&server.token)
        .bind(&server.org_id)
        .bind(sqlx::types::Json(&server.labels))
        .bind(&server.version)
        .bind(server.status.to_string())
        .bind(server.last_seen)
        .bind(server.registered_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_server(&self, id: &str) -> Result<Option<ServerNode>> {
        let row = sqlx::query(
            "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at
             FROM servers WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| row_to_server(&r)))
    }

    pub async fn find_server_by_token(&self, token: &str) -> Result<Option<ServerNode>> {
        let row = sqlx::query(
            "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at
             FROM servers WHERE token = $1",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| row_to_server(&r)))
    }

    pub async fn list_servers(&self, org_id: Option<&str>) -> Result<Vec<ServerNode>> {
        let rows = if let Some(oid) = org_id {
            sqlx::query(
                "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at
                 FROM servers WHERE org_id = $1 ORDER BY registered_at DESC",
            )
            .bind(oid)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, name, url, token, org_id, labels, version, status, last_seen, registered_at
                 FROM servers ORDER BY registered_at DESC",
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.iter().map(row_to_server).collect())
    }

    /// Update last_seen to now and set status=online. Returns the server id if found.
    pub async fn update_server_heartbeat(&self, token: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "UPDATE servers SET last_seen = NOW(), status = 'online'
             WHERE token = $1 RETURNING id",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.get("id")))
    }

    /// Mark servers offline if last_seen is older than the given threshold.
    pub async fn mark_stale_servers_offline(
        &self,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE servers SET status = 'offline'
             WHERE status = 'online' AND (last_seen IS NULL OR last_seen < $1)",
        )
        .bind(older_than)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_server(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM servers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── Project Environments ─────────────────────────────────────────────────

    pub async fn list_environments(&self, project_id: &str) -> Result<Vec<ProjectEnvironment>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, server_id, nats_subject_prefix, is_default,
                    canary_target_env_id, canary_percentage, created_at
             FROM project_environments WHERE project_id = $1 ORDER BY created_at",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_environment).collect())
    }

    pub async fn get_environment(
        &self,
        project_id: &str,
        env_id: &str,
    ) -> Result<Option<ProjectEnvironment>> {
        let row = sqlx::query(
            "SELECT id, project_id, name, server_id, nats_subject_prefix, is_default,
                    canary_target_env_id, canary_percentage, created_at
             FROM project_environments WHERE id = $1 AND project_id = $2",
        )
        .bind(env_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(row_to_environment))
    }

    pub async fn get_default_environment(
        &self,
        project_id: &str,
    ) -> Result<Option<ProjectEnvironment>> {
        let row = sqlx::query(
            "SELECT id, project_id, name, server_id, nats_subject_prefix, is_default,
                    canary_target_env_id, canary_percentage, created_at
             FROM project_environments WHERE project_id = $1 AND is_default = true LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(row_to_environment))
    }

    pub async fn create_environment(
        &self,
        id: &str,
        project_id: &str,
        req: &CreateEnvironmentRequest,
        is_default: bool,
    ) -> Result<ProjectEnvironment> {
        sqlx::query(
            "INSERT INTO project_environments
             (id, project_id, name, server_id, nats_subject_prefix, is_default,
              canary_target_env_id, canary_percentage, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, NULL, 0, NOW())",
        )
        .bind(id)
        .bind(project_id)
        .bind(&req.name)
        .bind(&req.server_id)
        .bind(&req.nats_subject_prefix)
        .bind(is_default)
        .execute(&self.pool)
        .await?;
        Ok(self
            .get_environment(project_id, id)
            .await?
            .expect("just inserted"))
    }

    pub async fn update_environment(
        &self,
        project_id: &str,
        env_id: &str,
        req: &UpdateEnvironmentRequest,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE project_environments SET
               name = COALESCE($1, name),
               server_id = CASE WHEN $2::boolean THEN $3 ELSE server_id END,
               nats_subject_prefix = CASE WHEN $4::boolean THEN $5 ELSE nats_subject_prefix END
             WHERE id = $6 AND project_id = $7",
        )
        .bind(&req.name)
        .bind(req.server_id.is_some())
        .bind(&req.server_id)
        .bind(req.nats_subject_prefix.is_some())
        .bind(&req.nats_subject_prefix)
        .bind(env_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_canary(
        &self,
        project_id: &str,
        env_id: &str,
        canary_target_env_id: Option<&str>,
        canary_percentage: i32,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE project_environments
             SET canary_target_env_id = $1, canary_percentage = $2
             WHERE id = $3 AND project_id = $4",
        )
        .bind(canary_target_env_id)
        .bind(canary_percentage)
        .bind(env_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_environment(&self, project_id: &str, env_id: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM project_environments WHERE id = $1 AND project_id = $2 AND is_default = false",
        )
        .bind(env_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── Draft Rulesets ────────────────────────────────────────────────────────

    pub async fn list_draft_rulesets(&self, project_id: &str) -> Result<Vec<ProjectRulesetMeta>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, draft_seq, draft_updated_at, draft_updated_by,
                    published_version, published_at, created_at
             FROM project_rulesets WHERE project_id = $1 ORDER BY name",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_ruleset_meta).collect())
    }

    pub async fn get_draft_ruleset(
        &self,
        project_id: &str,
        name: &str,
    ) -> Result<Option<ProjectRuleset>> {
        let row = sqlx::query(
            "SELECT id, project_id, name, draft, draft_seq, draft_updated_at, draft_updated_by,
                    published_version, published_at, created_at
             FROM project_rulesets WHERE project_id = $1 AND name = $2",
        )
        .bind(project_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(row_to_ruleset))
    }

    /// Save a draft with optimistic-locking. Returns Err with message "conflict" on seq mismatch.
    pub async fn save_draft_ruleset(
        &self,
        id: &str,
        project_id: &str,
        name: &str,
        draft: &serde_json::Value,
        expected_seq: i64,
        user_id: &str,
    ) -> Result<ProjectRuleset> {
        let existing = self.get_draft_ruleset(project_id, name).await?;

        if let Some(ref existing) = existing {
            if existing.meta.draft_seq != expected_seq {
                return Err(anyhow::anyhow!("conflict"));
            }
            // Update
            sqlx::query(
                "UPDATE project_rulesets SET
                   draft = $1, draft_seq = draft_seq + 1,
                   draft_updated_at = NOW(), draft_updated_by = $2
                 WHERE project_id = $3 AND name = $4",
            )
            .bind(sqlx::types::Json(draft))
            .bind(user_id)
            .bind(project_id)
            .bind(name)
            .execute(&self.pool)
            .await?;
        } else {
            // Insert
            sqlx::query(
                "INSERT INTO project_rulesets
                 (id, project_id, name, draft, draft_seq, draft_updated_at, draft_updated_by, created_at)
                 VALUES ($1, $2, $3, $4, 1, NOW(), $5, NOW())",
            )
            .bind(id)
            .bind(project_id)
            .bind(name)
            .bind(sqlx::types::Json(draft))
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        }

        Ok(self
            .get_draft_ruleset(project_id, name)
            .await?
            .expect("just upserted"))
    }

    pub async fn delete_draft_ruleset(&self, project_id: &str, name: &str) -> Result<bool> {
        let result =
            sqlx::query("DELETE FROM project_rulesets WHERE project_id = $1 AND name = $2")
                .bind(project_id)
                .bind(name)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_ruleset_published(
        &self,
        project_id: &str,
        name: &str,
        version: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE project_rulesets SET published_version = $1, published_at = NOW()
             WHERE project_id = $2 AND name = $3",
        )
        .bind(version)
        .bind(project_id)
        .bind(name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Deployments ───────────────────────────────────────────────────────────

    pub async fn create_deployment(&self, dep: &RulesetDeployment) -> Result<()> {
        sqlx::query(
            "INSERT INTO ruleset_deployments
             (id, project_id, environment_id, ruleset_name, version, release_note, snapshot,
              deployed_at, deployed_by, status)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(&dep.id)
        .bind(&dep.project_id)
        .bind(&dep.environment_id)
        .bind(&dep.ruleset_name)
        .bind(&dep.version)
        .bind(&dep.release_note)
        .bind(sqlx::types::Json(&dep.snapshot))
        .bind(dep.deployed_at)
        .bind(&dep.deployed_by)
        .bind(dep.status.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_deployment_status(
        &self,
        deployment_id: &str,
        status: DeploymentStatus,
    ) -> Result<()> {
        sqlx::query("UPDATE ruleset_deployments SET status = $1 WHERE id = $2")
            .bind(status.to_string())
            .bind(deployment_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_deployments(
        &self,
        project_id: &str,
        ruleset_name: Option<&str>,
        limit: i64,
    ) -> Result<Vec<RulesetDeployment>> {
        let rows = if let Some(name) = ruleset_name {
            sqlx::query(
                "SELECT d.id, d.project_id, d.environment_id, e.name AS environment_name,
                        d.ruleset_name, d.version, d.release_note, d.snapshot,
                        d.deployed_at, d.deployed_by, d.status
                 FROM ruleset_deployments d
                 LEFT JOIN project_environments e ON e.id = d.environment_id
                 WHERE d.project_id = $1 AND d.ruleset_name = $2
                 ORDER BY d.deployed_at DESC LIMIT $3",
            )
            .bind(project_id)
            .bind(name)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT d.id, d.project_id, d.environment_id, e.name AS environment_name,
                        d.ruleset_name, d.version, d.release_note, d.snapshot,
                        d.deployed_at, d.deployed_by, d.status
                 FROM ruleset_deployments d
                 LEFT JOIN project_environments e ON e.id = d.environment_id
                 WHERE d.project_id = $1
                 ORDER BY d.deployed_at DESC LIMIT $2",
            )
            .bind(project_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        rows.into_iter().map(|r| row_to_deployment(&r)).collect()
    }

    pub async fn get_deployment(
        &self,
        project_id: &str,
        deployment_id: &str,
    ) -> Result<Option<RulesetDeployment>> {
        let row = sqlx::query(
            "SELECT d.id, d.project_id, d.environment_id, e.name AS environment_name,
                    d.ruleset_name, d.version, d.release_note, d.snapshot,
                    d.deployed_at, d.deployed_by, d.status
             FROM ruleset_deployments d
             LEFT JOIN project_environments e ON e.id = d.environment_id
             WHERE d.id = $1 AND d.project_id = $2",
        )
        .bind(deployment_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(|r| row_to_deployment(r)).transpose()
    }

    // ── RBAC: Org Roles ──────────────────────────────────────────────────────

    pub async fn list_org_roles(&self, org_id: &str) -> Result<Vec<OrgRole>> {
        let rows = sqlx::query(
            "SELECT id, org_id, name, description, permissions, is_system, created_at
             FROM org_roles WHERE org_id = $1 ORDER BY is_system DESC, name",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_org_role).collect())
    }

    pub async fn get_org_role(&self, org_id: &str, role_id: &str) -> Result<Option<OrgRole>> {
        let row = sqlx::query(
            "SELECT id, org_id, name, description, permissions, is_system, created_at
             FROM org_roles WHERE id = $1 AND org_id = $2",
        )
        .bind(role_id)
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(row_to_org_role))
    }

    pub async fn create_org_role(
        &self,
        id: &str,
        org_id: &str,
        req: &CreateRoleRequest,
        is_system: bool,
    ) -> Result<OrgRole> {
        sqlx::query(
            "INSERT INTO org_roles (id, org_id, name, description, permissions, is_system, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, NOW())",
        )
        .bind(id)
        .bind(org_id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.permissions)
        .bind(is_system)
        .execute(&self.pool)
        .await?;
        Ok(self.get_org_role(org_id, id).await?.expect("just inserted"))
    }

    pub async fn update_org_role(
        &self,
        org_id: &str,
        role_id: &str,
        req: &UpdateRoleRequest,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE org_roles SET
               name = COALESCE($1, name),
               description = CASE WHEN $2::boolean THEN $3 ELSE description END,
               permissions = COALESCE($4, permissions)
             WHERE id = $5 AND org_id = $6 AND is_system = false",
        )
        .bind(&req.name)
        .bind(req.description.is_some())
        .bind(&req.description)
        .bind(req.permissions.as_deref())
        .bind(role_id)
        .bind(org_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_org_role(&self, org_id: &str, role_id: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM org_roles WHERE id = $1 AND org_id = $2 AND is_system = false",
        )
        .bind(role_id)
        .bind(org_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── RBAC: User Role Assignments ──────────────────────────────────────────

    pub async fn list_user_roles(
        &self,
        org_id: &str,
        user_id: &str,
    ) -> Result<Vec<UserRoleAssignment>> {
        let rows = sqlx::query(
            "SELECT uur.user_id, uur.org_id, uur.role_id, r.name AS role_name,
                    uur.assigned_at, uur.assigned_by
             FROM user_org_roles uur
             JOIN org_roles r ON r.id = uur.role_id
             WHERE uur.org_id = $1 AND uur.user_id = $2
             ORDER BY uur.assigned_at",
        )
        .bind(org_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_user_role).collect())
    }

    pub async fn assign_role(
        &self,
        org_id: &str,
        user_id: &str,
        role_id: &str,
        assigned_by: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO user_org_roles (user_id, org_id, role_id, assigned_at, assigned_by)
             VALUES ($1, $2, $3, NOW(), $4)
             ON CONFLICT (user_id, org_id, role_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(org_id)
        .bind(role_id)
        .bind(assigned_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn revoke_role(&self, org_id: &str, user_id: &str, role_id: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM user_org_roles WHERE user_id = $1 AND org_id = $2 AND role_id = $3",
        )
        .bind(user_id)
        .bind(org_id)
        .bind(role_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get the union of all permissions for a user in an org.
    pub async fn get_user_permissions(
        &self,
        org_id: &str,
        user_id: &str,
    ) -> Result<std::collections::HashSet<String>> {
        let rows = sqlx::query(
            "SELECT DISTINCT unnest(r.permissions) AS perm
             FROM user_org_roles uur
             JOIN org_roles r ON r.id = uur.role_id
             WHERE uur.org_id = $1 AND uur.user_id = $2",
        )
        .bind(org_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.get("perm")).collect())
    }

    /// Seed built-in system roles for an org if they don't already exist.
    pub async fn seed_system_roles(&self, org_id: &str) -> Result<()> {
        let existing = self.list_org_roles(org_id).await?;
        let has = |name: &str| existing.iter().any(|r| r.name == name && r.is_system);

        let system_roles: &[(&str, &str, &[&str])] = &[
            (
                "owner",
                "Organization owner — full access",
                &[
                    "org:view",
                    "org:manage",
                    "member:view",
                    "member:invite",
                    "member:remove",
                    "role:view",
                    "role:manage",
                    "project:view",
                    "project:create",
                    "project:manage",
                    "project:delete",
                    "ruleset:view",
                    "ruleset:edit",
                    "ruleset:publish",
                    "environment:view",
                    "environment:manage",
                    "server:view",
                    "server:manage",
                    "test:run",
                    "deployment:view",
                    "deployment:redeploy",
                    "canary:manage",
                ],
            ),
            (
                "admin",
                "Administrator — manages members and deployments",
                &[
                    "org:view",
                    "org:manage",
                    "member:view",
                    "member:invite",
                    "member:remove",
                    "role:view",
                    "role:manage",
                    "project:view",
                    "project:create",
                    "project:manage",
                    "ruleset:view",
                    "ruleset:edit",
                    "ruleset:publish",
                    "environment:view",
                    "environment:manage",
                    "server:view",
                    "server:manage",
                    "test:run",
                    "deployment:view",
                    "deployment:redeploy",
                    "canary:manage",
                ],
            ),
            (
                "editor",
                "Editor — edits and tests rulesets",
                &[
                    "org:view",
                    "member:view",
                    "role:view",
                    "project:view",
                    "ruleset:view",
                    "ruleset:edit",
                    "environment:view",
                    "server:view",
                    "test:run",
                    "deployment:view",
                ],
            ),
            (
                "viewer",
                "Viewer — read-only access",
                &[
                    "org:view",
                    "member:view",
                    "role:view",
                    "project:view",
                    "ruleset:view",
                    "environment:view",
                    "server:view",
                    "deployment:view",
                ],
            ),
        ];

        for (name, desc, perms) in system_roles {
            if has(name) {
                continue;
            }
            let id = uuid::Uuid::new_v4().to_string();
            self.create_org_role(
                &id,
                org_id,
                &CreateRoleRequest {
                    name: name.to_string(),
                    description: Some(desc.to_string()),
                    permissions: perms.iter().map(|s| s.to_string()).collect(),
                },
                true,
            )
            .await?;
        }
        Ok(())
    }

    /// Migrate a legacy Role to a system role assignment for a user.
    pub async fn migrate_legacy_role(
        &self,
        org_id: &str,
        user_id: &str,
        legacy_role: &Role,
    ) -> Result<()> {
        let role_name = legacy_role.to_string();
        let row = sqlx::query(
            "SELECT id FROM org_roles WHERE org_id = $1 AND name = $2 AND is_system = true",
        )
        .bind(org_id)
        .bind(&role_name)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else { return Ok(()) };
        let role_id: String = row.get("id");
        self.assign_role(org_id, user_id, &role_id, "system-migration")
            .await
    }

    // ── Startup migration helpers ─────────────────────────────────────────────

    pub async fn list_all_orgs(&self) -> Result<Vec<Organization>> {
        let rows = sqlx::query(
            "SELECT id, name, description, created_at, created_by FROM organizations ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut orgs = Vec::new();
        for row in rows {
            let org_id: String = row.get("id");
            let members = self.fetch_members(&org_id).await?;
            orgs.push(Organization {
                id: org_id,
                name: row.get("name"),
                description: row.get("description"),
                created_at: row.get("created_at"),
                created_by: row.get("created_by"),
                members,
            });
        }
        Ok(orgs)
    }

    pub async fn list_all_projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query("SELECT * FROM projects ORDER BY created_at")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(row_to_project).collect())
    }

    /// Migrate a project's legacy `server_id` column to a `project_environments` row (idempotent).
    pub async fn migrate_project_server_to_environment(
        &self,
        project_id: &str,
        server_id: Option<&str>,
    ) -> Result<()> {
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM project_environments WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;

        if existing > 0 {
            return Ok(());
        }

        let env_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO project_environments
               (id, project_id, name, server_id, nats_subject_prefix, is_default, canary_percentage)
               VALUES ($1, $2, 'production', $3, NULL, true, 0)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(&env_id)
        .bind(project_id)
        .bind(server_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

// ── Row helpers ──────────────────────────────────────────────────────────────

fn row_to_user(r: &sqlx::postgres::PgRow) -> User {
    User {
        id: r.get("id"),
        email: r.get("email"),
        password_hash: r.get("password_hash"),
        display_name: r.get("display_name"),
        created_at: r.get("created_at"),
        last_login: r.get("last_login"),
    }
}

fn row_to_project(r: &sqlx::postgres::PgRow) -> Project {
    Project {
        id: r.get("id"),
        name: r.get("name"),
        description: r.get("description"),
        org_id: r.get("org_id"),
        created_at: r.get("created_at"),
        created_by: r.get("created_by"),
        server_id: r.get("server_id"),
    }
}

fn row_to_server(r: &sqlx::postgres::PgRow) -> ServerNode {
    use std::str::FromStr;
    let labels: sqlx::types::Json<serde_json::Value> = r.get("labels");
    let status_str: String = r.get("status");
    ServerNode {
        id: r.get("id"),
        name: r.get("name"),
        url: r.get("url"),
        token: r.get("token"),
        org_id: r.get("org_id"),
        labels: labels.0,
        version: r.get("version"),
        status: ServerStatus::from_str(&status_str).unwrap_or(ServerStatus::Offline),
        last_seen: r.get("last_seen"),
        registered_at: r.get("registered_at"),
    }
}

// ── Enum serialization helpers ────────────────────────────────────────────────

fn role_to_str(r: &Role) -> &'static str {
    match r {
        Role::Owner => "owner",
        Role::Admin => "admin",
        Role::Editor => "editor",
        Role::Viewer => "viewer",
    }
}

fn str_to_role(s: &str) -> Result<Role> {
    s.parse().map_err(|e: String| anyhow::anyhow!(e))
}

fn fact_data_type_to_str(t: &FactDataType) -> &'static str {
    match t {
        FactDataType::String => "string",
        FactDataType::Number => "number",
        FactDataType::Boolean => "boolean",
        FactDataType::Date => "date",
        FactDataType::Object => "object",
    }
}

fn str_to_fact_data_type(s: &str) -> Result<FactDataType> {
    match s {
        "string" => Ok(FactDataType::String),
        "number" => Ok(FactDataType::Number),
        "boolean" => Ok(FactDataType::Boolean),
        "date" => Ok(FactDataType::Date),
        "object" => Ok(FactDataType::Object),
        other => Err(anyhow::anyhow!("invalid data type: {}", other)),
    }
}

fn null_policy_to_str(p: &NullPolicy) -> &'static str {
    match p {
        NullPolicy::Error => "error",
        NullPolicy::Default => "default",
        NullPolicy::Skip => "skip",
    }
}

fn str_to_null_policy(s: &str) -> Result<NullPolicy> {
    match s {
        "error" => Ok(NullPolicy::Error),
        "default" => Ok(NullPolicy::Default),
        "skip" => Ok(NullPolicy::Skip),
        other => Err(anyhow::anyhow!("invalid null policy: {}", other)),
    }
}

fn history_source_to_str(s: &RulesetHistorySource) -> &'static str {
    match s {
        RulesetHistorySource::Sync => "sync",
        RulesetHistorySource::Edit => "edit",
        RulesetHistorySource::Save => "save",
        RulesetHistorySource::Restore => "restore",
        RulesetHistorySource::Create => "create",
        RulesetHistorySource::Publish => "publish",
    }
}

fn str_to_history_source(s: &str) -> Result<RulesetHistorySource> {
    match s {
        "sync" => Ok(RulesetHistorySource::Sync),
        "edit" => Ok(RulesetHistorySource::Edit),
        "save" => Ok(RulesetHistorySource::Save),
        "restore" => Ok(RulesetHistorySource::Restore),
        "create" => Ok(RulesetHistorySource::Create),
        "publish" => Ok(RulesetHistorySource::Publish),
        other => Err(anyhow::anyhow!("invalid history source: {}", other)),
    }
}

fn row_to_environment(r: &sqlx::postgres::PgRow) -> ProjectEnvironment {
    ProjectEnvironment {
        id: r.get("id"),
        project_id: r.get("project_id"),
        name: r.get("name"),
        server_id: r.get("server_id"),
        nats_subject_prefix: r.get("nats_subject_prefix"),
        is_default: r.get("is_default"),
        canary_target_env_id: r.get("canary_target_env_id"),
        canary_percentage: r.get("canary_percentage"),
        created_at: r.get("created_at"),
    }
}

fn row_to_ruleset_meta(r: &sqlx::postgres::PgRow) -> ProjectRulesetMeta {
    ProjectRulesetMeta {
        id: r.get("id"),
        project_id: r.get("project_id"),
        name: r.get("name"),
        draft_seq: r.get("draft_seq"),
        draft_updated_at: r.get("draft_updated_at"),
        draft_updated_by: r.get("draft_updated_by"),
        published_version: r.get("published_version"),
        published_at: r.get("published_at"),
        created_at: r.get("created_at"),
    }
}

fn row_to_ruleset(r: &sqlx::postgres::PgRow) -> ProjectRuleset {
    let draft: sqlx::types::Json<serde_json::Value> = r.get("draft");
    ProjectRuleset {
        meta: row_to_ruleset_meta(r),
        draft: draft.0,
    }
}

fn row_to_deployment(r: &sqlx::postgres::PgRow) -> Result<RulesetDeployment> {
    use std::str::FromStr;
    let snapshot: sqlx::types::Json<serde_json::Value> = r.get("snapshot");
    let status_str: String = r.get("status");
    Ok(RulesetDeployment {
        id: r.get("id"),
        project_id: r.get("project_id"),
        environment_id: r.get("environment_id"),
        environment_name: r.try_get("environment_name").ok(),
        ruleset_name: r.get("ruleset_name"),
        version: r.get("version"),
        release_note: r.get("release_note"),
        snapshot: snapshot.0,
        deployed_at: r.get("deployed_at"),
        deployed_by: r.get("deployed_by"),
        status: DeploymentStatus::from_str(&status_str).unwrap_or(DeploymentStatus::Failed),
    })
}

fn row_to_org_role(r: &sqlx::postgres::PgRow) -> OrgRole {
    let permissions: Vec<String> = r.get("permissions");
    OrgRole {
        id: r.get("id"),
        org_id: r.get("org_id"),
        name: r.get("name"),
        description: r.get("description"),
        permissions,
        is_system: r.get("is_system"),
        created_at: r.get("created_at"),
    }
}

fn row_to_user_role(r: &sqlx::postgres::PgRow) -> UserRoleAssignment {
    UserRoleAssignment {
        user_id: r.get("user_id"),
        org_id: r.get("org_id"),
        role_id: r.get("role_id"),
        role_name: r.try_get("role_name").ok(),
        assigned_at: r.get("assigned_at"),
        assigned_by: r.get("assigned_by"),
    }
}
