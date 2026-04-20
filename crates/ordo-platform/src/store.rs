//! PostgreSQL-backed persistence for platform data.

use crate::models::{
    ConceptDefinition, ContractField, CreateEnvironmentRequest, CreateReleasePolicyRequest,
    CreateReleaseRequest, CreateRoleRequest, DecisionContract, DeploymentStatus, FactDataType,
    FactDefinition, Member, NullPolicy, OrgRole, Organization, Project, ProjectEnvironment,
    ProjectRuleset, ProjectRulesetMeta, ReleaseApprovalDecision, ReleaseApprovalRecord,
    ReleaseContentDiffSummary, ReleaseExecution, ReleaseExecutionInstance, ReleaseExecutionStatus,
    ReleaseExecutionSummary, ReleaseInstanceStatus, ReleasePolicy, ReleasePolicyScope,
    ReleasePolicyTargetType, ReleaseRequest, ReleaseRequestSnapshot, ReleaseRequestStatus,
    ReleaseVersionDiff, Role, RollbackPolicy, RolloutStrategy, RulesetDeployment,
    RulesetHistoryEntry, RulesetHistorySource, ServerNode, ServerStatus, TestCase, TestExpectation,
    UpdateEnvironmentRequest, UpdateReleasePolicyRequest, UpdateRoleRequest, User,
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

    pub async fn get_latest_ruleset_history_snapshot(
        &self,
        project_id: &str,
        ruleset_name: &str,
    ) -> Result<Option<(JsonValue, chrono::DateTime<chrono::Utc>, Option<String>)>> {
        let row = sqlx::query(
            "SELECT snapshot, created_at, author_id
             FROM ruleset_history
             WHERE project_id = $1 AND ruleset_name = $2
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(project_id)
        .bind(ruleset_name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| {
            let snapshot: sqlx::types::Json<JsonValue> = r.get("snapshot");
            (snapshot.0, r.get("created_at"), r.get("author_id"))
        }))
    }

    pub async fn backfill_project_rulesets_from_history(&self) -> Result<u64> {
        let result = sqlx::query(
            "WITH latest_history AS (
                SELECT DISTINCT ON (project_id, ruleset_name)
                    project_id,
                    ruleset_name,
                    snapshot,
                    created_at,
                    author_id
                FROM ruleset_history
                ORDER BY project_id, ruleset_name, created_at DESC
            ),
            latest_publish AS (
                SELECT DISTINCT ON (project_id, ruleset_name)
                    project_id,
                    ruleset_name,
                    created_at,
                    snapshot #>> '{config,version}' AS published_version
                FROM ruleset_history
                WHERE source = 'publish'
                ORDER BY project_id, ruleset_name, created_at DESC
            )
            INSERT INTO project_rulesets (
                id, project_id, name, draft, draft_seq, draft_updated_at, draft_updated_by,
                published_version, published_at, created_at
            )
            SELECT
                gen_random_uuid()::text,
                lh.project_id,
                lh.ruleset_name,
                lh.snapshot,
                1,
                lh.created_at,
                lh.author_id,
                lp.published_version,
                lp.created_at,
                lh.created_at
            FROM latest_history lh
            LEFT JOIN latest_publish lp
              ON lp.project_id = lh.project_id AND lp.ruleset_name = lh.ruleset_name
            WHERE NOT EXISTS (
                SELECT 1 FROM project_rulesets pr
                WHERE pr.project_id = lh.project_id AND pr.name = lh.ruleset_name
            )",
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
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

    #[allow(clippy::too_many_arguments)]
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

    /// Mark online servers degraded if last_seen is older than the given threshold.
    pub async fn mark_stale_servers_degraded(
        &self,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE servers SET status = 'degraded'
             WHERE status = 'online' AND (last_seen IS NULL OR last_seen < $1)",
        )
        .bind(older_than)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Mark stale servers offline if last_seen is older than the given threshold.
    pub async fn mark_stale_servers_offline(
        &self,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE servers SET status = 'offline'
             WHERE status IN ('online', 'degraded') AND (last_seen IS NULL OR last_seen < $1)",
        )
        .bind(older_than)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Delete offline servers whose last heartbeat is older than the given threshold.
    pub async fn delete_stale_offline_servers(
        &self,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM servers
             WHERE status = 'offline' AND (last_seen IS NULL OR last_seen < $1)",
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
        row.as_ref().map(row_to_deployment).transpose()
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

    pub async fn clear_user_role_assignments(&self, org_id: &str, user_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM user_org_roles WHERE user_id = $1 AND org_id = $2")
            .bind(user_id)
            .bind(org_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Keep `user_org_roles` in sync with the member's org-level role.
    /// This is the single source of truth for permission checks.
    pub async fn sync_member_system_role(
        &self,
        org_id: &str,
        user_id: &str,
        member_role: &Role,
        assigned_by: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Clear existing system-role assignments for this user in this org.
        sqlx::query(
            "DELETE FROM user_org_roles
             WHERE org_id = $1 AND user_id = $2
               AND role_id IN (
                 SELECT id FROM org_roles WHERE org_id = $1 AND is_system = true
               )",
        )
        .bind(org_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        // Assign the current member role's system role.
        let role_name = member_role.to_string();
        let role_row = sqlx::query(
            "SELECT id FROM org_roles WHERE org_id = $1 AND name = $2 AND is_system = true",
        )
        .bind(org_id)
        .bind(&role_name)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = role_row {
            let role_id: String = row.get("id");
            sqlx::query(
                "INSERT INTO user_org_roles (user_id, org_id, role_id, assigned_at, assigned_by)
                 VALUES ($1, $2, $3, NOW(), $4)
                 ON CONFLICT (user_id, org_id, role_id) DO NOTHING",
            )
            .bind(user_id)
            .bind(org_id)
            .bind(&role_id)
            .bind(assigned_by)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
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

    // ── Release Center ──────────────────────────────────────────────────────

    pub async fn list_release_policies(
        &self,
        org_id: &str,
        project_id: Option<&str>,
    ) -> Result<Vec<ReleasePolicy>> {
        let rows = sqlx::query(
            "SELECT id, org_id, project_id, name, scope, target_type, target_id, description,
                    min_approvals, allow_self_approval, approver_ids, rollout_strategy,
                    rollback_policy, created_at, updated_at
             FROM release_policies
             WHERE org_id = $1 AND ($2::text IS NULL OR project_id = $2 OR project_id IS NULL)
             ORDER BY project_id NULLS FIRST, name",
        )
        .bind(org_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_release_policy).collect()
    }

    pub async fn get_release_policy(
        &self,
        org_id: &str,
        project_id: &str,
        policy_id: &str,
    ) -> Result<Option<ReleasePolicy>> {
        let row = sqlx::query(
            "SELECT id, org_id, project_id, name, scope, target_type, target_id, description,
                    min_approvals, allow_self_approval, approver_ids, rollout_strategy,
                    rollback_policy, created_at, updated_at
             FROM release_policies
             WHERE id = $1 AND org_id = $2 AND (project_id = $3 OR project_id IS NULL)",
        )
        .bind(policy_id)
        .bind(org_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_release_policy).transpose()
    }

    pub async fn create_release_policy(
        &self,
        id: &str,
        org_id: &str,
        project_id: Option<&str>,
        req: &CreateReleasePolicyRequest,
    ) -> Result<ReleasePolicy> {
        sqlx::query(
            "INSERT INTO release_policies
             (id, org_id, project_id, name, scope, target_type, target_id, description,
              min_approvals, allow_self_approval, approver_ids, rollout_strategy,
              rollback_policy, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW(), NOW())",
        )
        .bind(id)
        .bind(org_id)
        .bind(project_id)
        .bind(&req.name)
        .bind(req.scope.to_string())
        .bind(req.target_type.to_string())
        .bind(&req.target_id)
        .bind(&req.description)
        .bind(req.min_approvals)
        .bind(req.allow_self_approval)
        .bind(&req.approver_ids)
        .bind(sqlx::types::Json(req.rollout_strategy.clone()))
        .bind(sqlx::types::Json(req.rollback_policy.clone()))
        .execute(&self.pool)
        .await?;
        Ok(self
            .get_release_policy(org_id, project_id.unwrap_or_default(), id)
            .await?
            .expect("just inserted"))
    }

    pub async fn update_release_policy(
        &self,
        org_id: &str,
        project_id: &str,
        policy_id: &str,
        req: &UpdateReleasePolicyRequest,
    ) -> Result<bool> {
        let Some(current) = self
            .get_release_policy(org_id, project_id, policy_id)
            .await?
        else {
            return Ok(false);
        };
        let result = sqlx::query(
            "UPDATE release_policies SET
               name = $1,
               description = $2,
               min_approvals = $3,
               allow_self_approval = $4,
               approver_ids = $5,
               rollout_strategy = $6,
               rollback_policy = $7,
               updated_at = NOW()
             WHERE id = $8 AND org_id = $9 AND (project_id = $10 OR project_id IS NULL)",
        )
        .bind(req.name.as_deref().unwrap_or(&current.name))
        .bind(req.description.as_ref().or(current.description.as_ref()))
        .bind(req.min_approvals.unwrap_or(current.min_approvals))
        .bind(
            req.allow_self_approval
                .unwrap_or(current.allow_self_approval),
        )
        .bind(req.approver_ids.as_ref().unwrap_or(&current.approver_ids))
        .bind(sqlx::types::Json(
            req.rollout_strategy
                .clone()
                .unwrap_or_else(|| current.rollout_strategy.clone()),
        ))
        .bind(sqlx::types::Json(
            req.rollback_policy
                .clone()
                .unwrap_or_else(|| current.rollback_policy.clone()),
        ))
        .bind(policy_id)
        .bind(org_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_release_policy(
        &self,
        org_id: &str,
        project_id: &str,
        policy_id: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM release_policies
             WHERE id = $1 AND org_id = $2 AND (project_id = $3 OR project_id IS NULL)",
        )
        .bind(policy_id)
        .bind(org_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn find_matching_release_policy(
        &self,
        org_id: &str,
        project_id: &str,
        environment_id: &str,
    ) -> Result<Option<ReleasePolicy>> {
        let row = sqlx::query(
            "SELECT id, org_id, project_id, name, scope, target_type, target_id, description,
                    min_approvals, allow_self_approval, approver_ids, rollout_strategy,
                    rollback_policy, created_at, updated_at
             FROM release_policies
             WHERE org_id = $1
               AND (project_id = $2 OR project_id IS NULL)
               AND (
                 (target_type = 'environment' AND target_id = $3)
                 OR (target_type = 'project' AND target_id = $2)
               )
             ORDER BY
               CASE WHEN target_type = 'environment' THEN 0 ELSE 1 END,
               CASE WHEN project_id IS NULL THEN 1 ELSE 0 END,
               updated_at DESC
             LIMIT 1",
        )
        .bind(org_id)
        .bind(project_id)
        .bind(environment_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_release_policy).transpose()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_release_request(
        &self,
        id: &str,
        org_id: &str,
        project_id: &str,
        created_by: &str,
        created_by_name: Option<&str>,
        created_by_email: Option<&str>,
        req: &CreateReleaseRequest,
        current_version: Option<&str>,
        version_diff: &ReleaseVersionDiff,
        content_diff: &ReleaseContentDiffSummary,
        request_snapshot: &ReleaseRequestSnapshot,
    ) -> Result<ReleaseRequest> {
        sqlx::query(
            "INSERT INTO release_requests
             (id, org_id, project_id, ruleset_name, version, environment_id, policy_id, status,
              title, change_summary, release_note, affected_instance_count, rollback_version,
              created_by, created_by_name, created_by_email, current_version, version_diff, content_diff,
              request_snapshot, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                     $18, $19, $20, NOW(), NOW())",
        )
        .bind(id)
        .bind(org_id)
        .bind(project_id)
        .bind(&req.ruleset_name)
        .bind(&req.version)
        .bind(&req.environment_id)
        .bind(&req.policy_id)
        .bind(ReleaseRequestStatus::PendingApproval.to_string())
        .bind(&req.title)
        .bind(&req.change_summary)
        .bind(&req.release_note)
        .bind(req.affected_instance_count.unwrap_or_default())
        .bind(&req.rollback_version)
        .bind(created_by)
        .bind(created_by_name)
        .bind(created_by_email)
        .bind(current_version)
        .bind(sqlx::types::Json(version_diff))
        .bind(sqlx::types::Json(content_diff))
        .bind(sqlx::types::Json(request_snapshot))
        .execute(&self.pool)
        .await?;
        Ok(self
            .get_release_request(org_id, project_id, id)
            .await?
            .expect("just inserted"))
    }

    pub async fn list_release_requests(
        &self,
        org_id: &str,
        project_id: &str,
    ) -> Result<Vec<ReleaseRequest>> {
        let rows = sqlx::query(
            "SELECT rr.id, rr.org_id, rr.project_id, rr.ruleset_name, rr.version, rr.environment_id,
                    env.name AS environment_name, rr.policy_id, rr.status, rr.title,
                    rr.change_summary, rr.release_note, rr.affected_instance_count,
                    rp.rollout_strategy,
                    rr.rollback_version, rr.created_by, COALESCE(rr.created_by_name, u.display_name) AS created_by_name,
                    rr.created_by_email, rr.version_diff, rr.content_diff, rr.request_snapshot,
                    rr.created_at, rr.updated_at
             FROM release_requests rr
             LEFT JOIN project_environments env ON env.id = rr.environment_id
             LEFT JOIN release_policies rp ON rp.id = rr.policy_id
             LEFT JOIN users u ON u.id = rr.created_by
             WHERE rr.org_id = $1 AND rr.project_id = $2
             ORDER BY rr.created_at DESC",
        )
        .bind(org_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let mut item = row_to_release_request(&row)?;
            item.approvals = self.list_release_approvals(&item.id).await?;
            items.push(item);
        }
        Ok(items)
    }

    pub async fn get_release_request(
        &self,
        org_id: &str,
        project_id: &str,
        release_id: &str,
    ) -> Result<Option<ReleaseRequest>> {
        let row = sqlx::query(
            "SELECT rr.id, rr.org_id, rr.project_id, rr.ruleset_name, rr.version, rr.environment_id,
                    env.name AS environment_name, rr.policy_id, rr.status, rr.title,
                    rr.change_summary, rr.release_note, rr.affected_instance_count,
                    rp.rollout_strategy,
                    rr.rollback_version, rr.created_by, COALESCE(rr.created_by_name, u.display_name) AS created_by_name,
                    rr.created_by_email, rr.version_diff, rr.content_diff, rr.request_snapshot,
                    rr.created_at, rr.updated_at
             FROM release_requests rr
             LEFT JOIN project_environments env ON env.id = rr.environment_id
             LEFT JOIN release_policies rp ON rp.id = rr.policy_id
             LEFT JOIN users u ON u.id = rr.created_by
             WHERE rr.id = $1 AND rr.org_id = $2 AND rr.project_id = $3",
        )
        .bind(release_id)
        .bind(org_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let mut item = row_to_release_request(&row)?;
        item.approvals = self.list_release_approvals(&item.id).await?;
        Ok(Some(item))
    }

    pub async fn create_release_approval(
        &self,
        id: &str,
        release_request_id: &str,
        stage: i32,
        reviewer_id: &str,
        reviewer_name: Option<&str>,
        reviewer_email: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO release_approvals
             (id, release_request_id, stage, reviewer_id, reviewer_name, reviewer_email, decision, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())",
        )
        .bind(id)
        .bind(release_request_id)
        .bind(stage)
        .bind(reviewer_id)
        .bind(reviewer_name)
        .bind(reviewer_email)
        .bind(ReleaseApprovalDecision::Pending.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_release_approvals(
        &self,
        release_request_id: &str,
    ) -> Result<Vec<ReleaseApprovalRecord>> {
        let rows = sqlx::query(
            "SELECT ra.id, ra.release_request_id, ra.stage, ra.reviewer_id,
                    COALESCE(ra.reviewer_name, u.display_name) AS reviewer_name,
                    COALESCE(ra.reviewer_email, u.email) AS reviewer_email,
                    ra.decision, ra.comment, ra.decided_at, ra.created_at
             FROM release_approvals ra
             LEFT JOIN users u ON u.id = ra.reviewer_id
             WHERE ra.release_request_id = $1
             ORDER BY ra.stage, ra.created_at",
        )
        .bind(release_request_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_release_approval).collect()
    }

    pub async fn review_release_request(
        &self,
        release_request_id: &str,
        reviewer_id: &str,
        decision: ReleaseApprovalDecision,
        comment: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE release_approvals
             SET decision = $1, comment = $2, decided_at = NOW()
             WHERE release_request_id = $3 AND reviewer_id = $4 AND decision = 'pending'",
        )
        .bind(decision.to_string())
        .bind(comment)
        .bind(release_request_id)
        .bind(reviewer_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_release_request_status(
        &self,
        release_request_id: &str,
        status: ReleaseRequestStatus,
    ) -> Result<()> {
        sqlx::query("UPDATE release_requests SET status = $1, updated_at = NOW() WHERE id = $2")
            .bind(status.to_string())
            .bind(release_request_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_release_execution(
        &self,
        id: &str,
        release_request_id: &str,
        status: ReleaseExecutionStatus,
        current_batch: i32,
        total_batches: i32,
        strategy: &RolloutStrategy,
        triggered_by: Option<&str>,
    ) -> Result<ReleaseExecution> {
        sqlx::query(
            "INSERT INTO release_executions
             (id, release_request_id, status, current_batch, total_batches, strategy_snapshot, started_at, triggered_by)
             VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7)",
        )
        .bind(id)
        .bind(release_request_id)
        .bind(status.to_string())
        .bind(current_batch)
        .bind(total_batches)
        .bind(sqlx::types::Json(strategy.clone()))
        .bind(triggered_by)
        .execute(&self.pool)
        .await?;
        Ok(self
            .get_release_execution(id)
            .await?
            .expect("just inserted"))
    }

    pub async fn update_release_execution_status(
        &self,
        execution_id: &str,
        status: ReleaseExecutionStatus,
        current_batch: Option<i32>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE release_executions
             SET status = $1,
                 current_batch = COALESCE($2, current_batch),
                 finished_at = CASE
                   WHEN $1 IN ('completed', 'failed') THEN NOW()
                   WHEN $1 IN ('preparing', 'waiting_start', 'rolling_out', 'paused', 'verifying', 'rollback_in_progress') THEN NULL
                   ELSE finished_at
                 END
             WHERE id = $3",
        )
        .bind(status.to_string())
        .bind(current_batch)
        .bind(execution_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn create_release_execution_event(
        &self,
        id: &str,
        execution_id: &str,
        instance_id: Option<&str>,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO release_execution_events
             (id, release_execution_id, instance_id, event_type, payload, created_at)
             VALUES ($1, $2, $3, $4, $5, NOW())",
        )
        .bind(id)
        .bind(execution_id)
        .bind(instance_id)
        .bind(event_type)
        .bind(payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn create_release_execution_instance(
        &self,
        instance: &ReleaseExecutionInstance,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO release_execution_instances
             (id, release_execution_id, instance_id, instance_name, zone, current_version, target_version,
              status, message, metric_summary, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, COALESCE($10::jsonb, '{}'::jsonb), $11)",
        )
        .bind(&instance.id)
        .bind(&instance.release_execution_id)
        .bind(&instance.instance_id)
        .bind(&instance.instance_name)
        .bind(&instance.zone)
        .bind(&instance.current_version)
        .bind(&instance.target_version)
        .bind(instance.status.to_string())
        .bind(&instance.message)
        .bind(instance.metric_summary.as_ref().map(|value| serde_json::json!({ "summary": value })))
        .bind(instance.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_release_execution_instance(
        &self,
        instance_id: &str,
        status: ReleaseInstanceStatus,
        message: Option<&str>,
        metric_summary: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE release_execution_instances
             SET status = $1,
                 message = $2,
                 metric_summary = COALESCE($3::jsonb, metric_summary),
                 updated_at = NOW()
             WHERE id = $4",
        )
        .bind(status.to_string())
        .bind(message)
        .bind(metric_summary.map(|value| serde_json::json!({ "summary": value })))
        .bind(instance_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_release_execution_instances(
        &self,
        execution_id: &str,
    ) -> Result<Vec<ReleaseExecutionInstance>> {
        let rows = sqlx::query(
            "SELECT id, release_execution_id, instance_id, instance_name, zone, current_version,
                    target_version, status, message, metric_summary, updated_at
             FROM release_execution_instances
             WHERE release_execution_id = $1
             ORDER BY updated_at DESC, instance_name",
        )
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_release_execution_instance).collect()
    }

    pub async fn get_release_execution(
        &self,
        execution_id: &str,
    ) -> Result<Option<ReleaseExecution>> {
        let row = sqlx::query(
            "SELECT id, release_request_id, status, current_batch, total_batches, strategy_snapshot, started_at
             FROM release_executions
             WHERE id = $1",
        )
        .bind(execution_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut item = row_to_release_execution(&row)?;
        item.instances = self.list_release_execution_instances(&item.id).await?;
        item.summary = summarize_release_execution_instances(&item.instances);
        Ok(Some(item))
    }

    pub async fn find_release_execution_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<ReleaseExecution>> {
        let row = sqlx::query(
            "SELECT id, release_request_id, status, current_batch, total_batches, strategy_snapshot, started_at
             FROM release_executions
             WHERE release_request_id = $1
             ORDER BY started_at DESC
             LIMIT 1",
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut item = row_to_release_execution(&row)?;
        item.instances = self.list_release_execution_instances(&item.id).await?;
        item.summary = summarize_release_execution_instances(&item.instances);
        Ok(Some(item))
    }

    pub async fn find_latest_project_release_execution(
        &self,
        org_id: &str,
        project_id: &str,
    ) -> Result<Option<ReleaseExecution>> {
        let row = sqlx::query(
            "SELECT re.id, re.release_request_id, re.status, re.current_batch, re.total_batches,
                    re.strategy_snapshot, re.started_at
             FROM release_executions re
             INNER JOIN release_requests rr ON rr.id = re.release_request_id
             WHERE rr.org_id = $1 AND rr.project_id = $2
             ORDER BY
               CASE WHEN re.status IN ('preparing', 'waiting_start', 'rolling_out', 'paused', 'verifying', 'rollback_in_progress') THEN 0 ELSE 1 END,
               re.started_at DESC
             LIMIT 1",
        )
        .bind(org_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut item = row_to_release_execution(&row)?;
        item.instances = self.list_release_execution_instances(&item.id).await?;
        item.summary = summarize_release_execution_instances(&item.instances);
        Ok(Some(item))
    }

    /// Seed built-in system roles for an org if they don't already exist.
    pub async fn seed_system_roles(&self, org_id: &str) -> Result<()> {
        let existing = self.list_org_roles(org_id).await?;
        let find_existing = |name: &str| existing.iter().find(|r| r.name == name && r.is_system);

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
                    "release:policy.manage",
                    "release:request.create",
                    "release:request.view",
                    "release:request.approve",
                    "release:request.reject",
                    "release:execute",
                    "release:pause",
                    "release:resume",
                    "release:rollback",
                    "release:instance.view",
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
                    "release:policy.manage",
                    "release:request.create",
                    "release:request.view",
                    "release:request.approve",
                    "release:request.reject",
                    "release:execute",
                    "release:pause",
                    "release:resume",
                    "release:rollback",
                    "release:instance.view",
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
                    "release:request.create",
                    "release:request.view",
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
                    "release:request.view",
                ],
            ),
        ];

        for (name, desc, perms) in system_roles {
            let desired_perms: Vec<String> = perms.iter().map(|s| s.to_string()).collect();
            if let Some(role) = find_existing(name) {
                let needs_update =
                    role.description.as_deref() != Some(*desc) || role.permissions != desired_perms;
                if needs_update {
                    sqlx::query(
                        "UPDATE org_roles
                         SET description = $1, permissions = $2
                         WHERE id = $3 AND org_id = $4 AND is_system = true",
                    )
                    .bind(*desc)
                    .bind(&desired_perms)
                    .bind(&role.id)
                    .bind(org_id)
                    .execute(&self.pool)
                    .await?;
                }
                continue;
            }
            let id = uuid::Uuid::new_v4().to_string();
            self.create_org_role(
                &id,
                org_id,
                &CreateRoleRequest {
                    name: name.to_string(),
                    description: Some(desc.to_string()),
                    permissions: desired_perms,
                },
                true,
            )
            .await?;
        }
        Ok(())
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

fn row_to_release_policy(r: &sqlx::postgres::PgRow) -> Result<ReleasePolicy> {
    use std::str::FromStr;
    let rollout_strategy: sqlx::types::Json<RolloutStrategy> = r.get("rollout_strategy");
    let rollback_policy: sqlx::types::Json<RollbackPolicy> = r.get("rollback_policy");
    let scope: String = r.get("scope");
    let target_type: String = r.get("target_type");
    Ok(ReleasePolicy {
        id: r.get("id"),
        org_id: r.get("org_id"),
        project_id: r.get("project_id"),
        name: r.get("name"),
        scope: ReleasePolicyScope::from_str(&scope).map_err(|e| anyhow::anyhow!(e))?,
        target_type: ReleasePolicyTargetType::from_str(&target_type)
            .map_err(|e| anyhow::anyhow!(e))?,
        target_id: r.get("target_id"),
        description: r.get("description"),
        min_approvals: r.get("min_approvals"),
        allow_self_approval: r.get("allow_self_approval"),
        approver_ids: r.get("approver_ids"),
        rollout_strategy: rollout_strategy.0,
        rollback_policy: rollback_policy.0,
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    })
}

fn row_to_release_request(r: &sqlx::postgres::PgRow) -> Result<ReleaseRequest> {
    use std::str::FromStr;
    let status: String = r.get("status");
    let rollout_strategy: Option<sqlx::types::Json<RolloutStrategy>> =
        r.try_get("rollout_strategy").ok();
    let version_diff: Option<sqlx::types::Json<ReleaseVersionDiff>> =
        r.try_get("version_diff").ok();
    let content_diff: Option<sqlx::types::Json<ReleaseContentDiffSummary>> =
        r.try_get("content_diff").ok();
    let request_snapshot: Option<sqlx::types::Json<ReleaseRequestSnapshot>> =
        r.try_get("request_snapshot").ok();
    Ok(ReleaseRequest {
        id: r.get("id"),
        org_id: r.get("org_id"),
        project_id: r.get("project_id"),
        ruleset_name: r.get("ruleset_name"),
        version: r.get("version"),
        environment_id: r.get("environment_id"),
        environment_name: r.try_get("environment_name").ok(),
        policy_id: r.get("policy_id"),
        status: ReleaseRequestStatus::from_str(&status).map_err(|e| anyhow::anyhow!(e))?,
        title: r.get("title"),
        change_summary: r.get("change_summary"),
        release_note: r.get("release_note"),
        affected_instance_count: r.get("affected_instance_count"),
        rollout_strategy: rollout_strategy.map(|v| v.0).unwrap_or_default(),
        rollback_version: r.get("rollback_version"),
        created_by: r.get("created_by"),
        created_by_name: r.try_get("created_by_name").ok(),
        created_by_email: r.try_get("created_by_email").ok(),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        version_diff: version_diff.map(|v| v.0).unwrap_or_default(),
        content_diff: content_diff.map(|v| v.0).unwrap_or_default(),
        request_snapshot: request_snapshot.map(|v| v.0).unwrap_or_default(),
        approvals: Vec::new(),
    })
}

fn row_to_release_approval(r: &sqlx::postgres::PgRow) -> Result<ReleaseApprovalRecord> {
    use std::str::FromStr;
    let decision: String = r.get("decision");
    Ok(ReleaseApprovalRecord {
        id: r.get("id"),
        release_request_id: r.get("release_request_id"),
        stage: r.get("stage"),
        reviewer_id: r.get("reviewer_id"),
        reviewer_name: r.try_get("reviewer_name").ok(),
        reviewer_email: r.try_get("reviewer_email").ok(),
        decision: ReleaseApprovalDecision::from_str(&decision).map_err(|e| anyhow::anyhow!(e))?,
        comment: r.get("comment"),
        decided_at: r.get("decided_at"),
        created_at: r.get("created_at"),
    })
}

fn row_to_release_execution(r: &sqlx::postgres::PgRow) -> Result<ReleaseExecution> {
    use std::str::FromStr;
    let status: String = r.get("status");
    let strategy: sqlx::types::Json<RolloutStrategy> = r.get("strategy_snapshot");
    Ok(ReleaseExecution {
        id: r.get("id"),
        request_id: r.get("release_request_id"),
        status: ReleaseExecutionStatus::from_str(&status).map_err(|e| anyhow::anyhow!(e))?,
        started_at: r.get("started_at"),
        current_batch: r.get("current_batch"),
        total_batches: r.get("total_batches"),
        strategy: strategy.0,
        summary: ReleaseExecutionSummary::default(),
        instances: Vec::new(),
    })
}

fn row_to_release_execution_instance(
    r: &sqlx::postgres::PgRow,
) -> Result<ReleaseExecutionInstance> {
    use std::str::FromStr;
    let status: String = r.get("status");
    let metric_summary: Option<sqlx::types::Json<JsonValue>> = r.try_get("metric_summary").ok();
    Ok(ReleaseExecutionInstance {
        id: r.get("id"),
        release_execution_id: r.get("release_execution_id"),
        instance_id: r.get("instance_id"),
        instance_name: r.get("instance_name"),
        zone: r.try_get("zone").ok(),
        current_version: r.get("current_version"),
        target_version: r.get("target_version"),
        status: ReleaseInstanceStatus::from_str(&status).map_err(|e| anyhow::anyhow!(e))?,
        updated_at: r.get("updated_at"),
        message: r.try_get("message").ok(),
        metric_summary: metric_summary.and_then(|value| {
            value
                .0
                .get("summary")
                .and_then(|item| item.as_str())
                .map(str::to_string)
        }),
    })
}

fn summarize_release_execution_instances(
    instances: &[ReleaseExecutionInstance],
) -> ReleaseExecutionSummary {
    let total_instances = instances.len() as i32;
    let succeeded_instances = instances
        .iter()
        .filter(|item| {
            item.status == ReleaseInstanceStatus::Success
                || item.status == ReleaseInstanceStatus::RolledBack
        })
        .count() as i32;
    let failed_instances = instances
        .iter()
        .filter(|item| item.status == ReleaseInstanceStatus::Failed)
        .count() as i32;
    let pending_instances = total_instances - succeeded_instances - failed_instances;

    ReleaseExecutionSummary {
        total_instances,
        succeeded_instances,
        failed_instances,
        pending_instances,
    }
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
