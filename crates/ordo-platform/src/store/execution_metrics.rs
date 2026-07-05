use super::*;
use crate::sync::RulesetExecStat;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

/// One stored snapshot row (cumulative counters for a (server, ruleset) at a
/// point in time).
#[derive(Debug, Clone)]
pub struct ExecutionSnapshot {
    pub server_id: String,
    pub ruleset: String,
    pub captured_at: DateTime<Utc>,
    pub exec_success: f64,
    pub exec_error: f64,
    pub terminal: BTreeMap<String, f64>,
    pub duration_count: f64,
    pub duration_sum_seconds: f64,
}

impl PlatformStore {
    /// The org a registered server belongs to (None if unknown/unassigned).
    pub async fn get_server_org(&self, server_id: &str) -> Result<Option<String>> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT org_id FROM servers WHERE id = $1")
                .bind(server_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|r| r.0))
    }

    /// Append one snapshot row per reported ruleset.
    pub async fn insert_execution_snapshots(
        &self,
        org_id: &str,
        server_id: &str,
        captured_at: DateTime<Utc>,
        rulesets: &[RulesetExecStat],
    ) -> Result<()> {
        for r in rulesets {
            sqlx::query(
                "INSERT INTO execution_metric_snapshots
                   (org_id, server_id, ruleset, captured_at, exec_success, exec_error,
                    terminal_counts, duration_count, duration_sum_seconds)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(org_id)
            .bind(server_id)
            .bind(&r.ruleset)
            .bind(captured_at)
            .bind(r.exec_success)
            .bind(r.exec_error)
            .bind(sqlx::types::Json(&r.terminal))
            .bind(r.duration_count)
            .bind(r.duration_sum_seconds)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Fetch snapshots for an org, optionally filtered to a set of ruleset names,
    /// within `[from, to]`, ordered so consecutive snapshots of the same
    /// (server, ruleset) are adjacent and time-ascending (ready for diffing).
    pub async fn fetch_execution_snapshots(
        &self,
        org_id: &str,
        ruleset_names: Option<&[String]>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<ExecutionSnapshot>> {
        // An empty allow-list means "no rulesets in this project" → no rows.
        if matches!(ruleset_names, Some(v) if v.is_empty()) {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            "SELECT server_id, ruleset, captured_at, exec_success, exec_error,
                    terminal_counts, duration_count, duration_sum_seconds
             FROM execution_metric_snapshots
             WHERE org_id = $1
               AND captured_at >= $2 AND captured_at <= $3
               AND ($4::text[] IS NULL OR ruleset = ANY($4))
             ORDER BY server_id, ruleset, captured_at",
        )
        .bind(org_id)
        .bind(from)
        .bind(to)
        .bind(ruleset_names)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let terminal: sqlx::types::Json<BTreeMap<String, f64>> = row.get("terminal_counts");
                ExecutionSnapshot {
                    server_id: row.get("server_id"),
                    ruleset: row.get("ruleset"),
                    captured_at: row.get("captured_at"),
                    exec_success: row.get("exec_success"),
                    exec_error: row.get("exec_error"),
                    terminal: terminal.0,
                    duration_count: row.get("duration_count"),
                    duration_sum_seconds: row.get("duration_sum_seconds"),
                }
            })
            .collect())
    }

    /// The ruleset names belonging to a project (for scoping analytics).
    pub async fn list_project_ruleset_names(&self, project_id: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM project_rulesets WHERE project_id = $1")
                .bind(project_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Delete snapshots older than `before` (retention). Returns rows removed.
    pub async fn prune_execution_snapshots(&self, before: DateTime<Utc>) -> Result<u64> {
        let res = sqlx::query("DELETE FROM execution_metric_snapshots WHERE captured_at < $1")
            .bind(before)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }
}
