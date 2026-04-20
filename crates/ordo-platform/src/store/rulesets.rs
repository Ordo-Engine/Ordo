use super::*;

fn extract_ruleset_version(draft: &serde_json::Value) -> Option<String> {
    draft
        .get("config")
        .and_then(|config| config.get("version"))
        .and_then(|version| version.as_str())
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(ToOwned::to_owned)
}

impl PlatformStore {
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

    pub async fn list_draft_rulesets(&self, project_id: &str) -> Result<Vec<ProjectRulesetMeta>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, draft_seq, draft_updated_at, draft_updated_by,
                    draft #>> '{config,version}' AS draft_version,
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
                    draft #>> '{config,version}' AS draft_version,
                    published_version, published_at, created_at
             FROM project_rulesets WHERE project_id = $1 AND name = $2",
        )
        .bind(project_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(row_to_ruleset))
    }

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
        let incoming_version = extract_ruleset_version(draft);

        if let Some(ref existing) = existing {
            if existing.meta.draft_seq != expected_seq {
                return Err(anyhow::anyhow!("conflict"));
            }
            if existing.meta.published_version.is_some()
                && incoming_version == existing.meta.published_version
            {
                return Err(anyhow::anyhow!(
                    "Published ruleset changes require a new version number"
                ));
            }
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

    /// On startup, mark any deployment that was stuck in 'queued' as 'failed'.
    /// A queued deployment means the platform crashed after persisting the record
    /// but before publishing to NATS.
    pub async fn fail_stuck_queued_deployments(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE ruleset_deployments SET status = 'failed' WHERE status = 'queued'",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
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
}
