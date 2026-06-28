use super::*;
use std::str::FromStr;

impl PlatformStore {
    pub async fn list_project_sub_rules(
        &self,
        org_id: &str,
        project_id: &str,
    ) -> Result<Vec<SubRuleAssetMeta>> {
        let rows = sqlx::query(
            "SELECT id, org_id, project_id, scope, name, display_name, description,
                    draft_seq, draft_updated_at, draft_updated_by,
                    created_at, created_by
             FROM sub_rule_assets
             WHERE org_id = $1 AND (scope = 'org' OR project_id = $2)
             ORDER BY scope, name",
        )
        .bind(org_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_sub_rule_meta).collect()
    }

    pub async fn list_org_sub_rules(&self, org_id: &str) -> Result<Vec<SubRuleAssetMeta>> {
        let rows = sqlx::query(
            "SELECT id, org_id, project_id, scope, name, display_name, description,
                    draft_seq, draft_updated_at, draft_updated_by,
                    created_at, created_by
             FROM sub_rule_assets
             WHERE org_id = $1 AND scope = 'org'
             ORDER BY name",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_sub_rule_meta).collect()
    }

    pub async fn get_sub_rule_asset(
        &self,
        org_id: &str,
        scope: SubRuleScope,
        project_id: Option<&str>,
        name: &str,
    ) -> Result<Option<SubRuleAsset>> {
        let row = match scope {
            SubRuleScope::Org => {
                sqlx::query(
                    "SELECT id, org_id, project_id, scope, name, display_name, description,
                            draft, input_schema, output_schema,
                            draft_seq, draft_updated_at, draft_updated_by,
                            created_at, created_by
                     FROM sub_rule_assets
                     WHERE org_id = $1 AND scope = 'org' AND name = $2",
                )
                .bind(org_id)
                .bind(name)
                .fetch_optional(&self.pool)
                .await?
            }
            SubRuleScope::Project => {
                let project_id = project_id.ok_or_else(|| {
                    anyhow::anyhow!("project_id is required for project-scoped sub-rule")
                })?;
                sqlx::query(
                    "SELECT id, org_id, project_id, scope, name, display_name, description,
                            draft, input_schema, output_schema,
                            draft_seq, draft_updated_at, draft_updated_by,
                            created_at, created_by
                     FROM sub_rule_assets
                     WHERE org_id = $1 AND scope = 'project' AND project_id = $2 AND name = $3",
                )
                .bind(org_id)
                .bind(project_id)
                .bind(name)
                .fetch_optional(&self.pool)
                .await?
            }
        };

        row.map(row_to_sub_rule_asset).transpose()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_sub_rule_asset(
        &self,
        id: &str,
        org_id: &str,
        project_id: Option<&str>,
        scope: SubRuleScope,
        name: &str,
        display_name: Option<&str>,
        description: Option<&str>,
        draft: &JsonValue,
        input_schema: &JsonValue,
        output_schema: &JsonValue,
        expected_seq: Option<i64>,
        user_id: &str,
    ) -> Result<SubRuleAsset> {
        let existing = self
            .get_sub_rule_asset(org_id, scope.clone(), project_id, name)
            .await?;

        if let Some(existing) = existing {
            if let Some(expected_seq) = expected_seq {
                if existing.meta.draft_seq != expected_seq {
                    return Err(anyhow::anyhow!("conflict"));
                }
            }

            sqlx::query(
                "UPDATE sub_rule_assets SET
                    display_name = $1,
                    description = $2,
                    draft = $3,
                    input_schema = $4,
                    output_schema = $5,
                    draft_seq = draft_seq + 1,
                    draft_updated_at = NOW(),
                    draft_updated_by = $6
                 WHERE id = $7",
            )
            .bind(display_name)
            .bind(description)
            .bind(sqlx::types::Json(draft))
            .bind(sqlx::types::Json(input_schema))
            .bind(sqlx::types::Json(output_schema))
            .bind(user_id)
            .bind(&existing.meta.id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO sub_rule_assets (
                    id, org_id, project_id, scope, name, display_name, description,
                    draft, input_schema, output_schema, draft_seq,
                    draft_updated_at, draft_updated_by, created_at, created_by
                 )
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1, NOW(), $11, NOW(), $11)",
            )
            .bind(id)
            .bind(org_id)
            .bind(project_id)
            .bind(scope.to_string())
            .bind(name)
            .bind(display_name)
            .bind(description)
            .bind(sqlx::types::Json(draft))
            .bind(sqlx::types::Json(input_schema))
            .bind(sqlx::types::Json(output_schema))
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        }

        self.get_sub_rule_asset(org_id, scope, project_id, name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("sub-rule asset was not found after upsert"))
    }

    pub async fn delete_sub_rule_asset(
        &self,
        org_id: &str,
        scope: SubRuleScope,
        project_id: Option<&str>,
        name: &str,
    ) -> Result<bool> {
        let result = match scope {
            SubRuleScope::Org => {
                sqlx::query(
                    "DELETE FROM sub_rule_assets
                     WHERE org_id = $1 AND scope = 'org' AND name = $2",
                )
                .bind(org_id)
                .bind(name)
                .execute(&self.pool)
                .await?
            }
            SubRuleScope::Project => {
                let project_id = project_id.ok_or_else(|| {
                    anyhow::anyhow!("project_id is required for project-scoped sub-rule")
                })?;
                sqlx::query(
                    "DELETE FROM sub_rule_assets
                     WHERE org_id = $1 AND scope = 'project' AND project_id = $2 AND name = $3",
                )
                .bind(org_id)
                .bind(project_id)
                .bind(name)
                .execute(&self.pool)
                .await?
            }
        };

        Ok(result.rows_affected() > 0)
    }
}

fn row_to_sub_rule_meta(row: sqlx::postgres::PgRow) -> Result<SubRuleAssetMeta> {
    let scope: String = row.get("scope");
    Ok(SubRuleAssetMeta {
        id: row.get("id"),
        org_id: row.get("org_id"),
        project_id: row.get("project_id"),
        scope: SubRuleScope::from_str(&scope).map_err(anyhow::Error::msg)?,
        name: row.get("name"),
        display_name: row.get("display_name"),
        description: row.get("description"),
        draft_seq: row.get("draft_seq"),
        draft_updated_at: row.get("draft_updated_at"),
        draft_updated_by: row.get("draft_updated_by"),
        created_at: row.get("created_at"),
        created_by: row.get("created_by"),
    })
}

fn row_to_sub_rule_asset(row: sqlx::postgres::PgRow) -> Result<SubRuleAsset> {
    let draft: sqlx::types::Json<JsonValue> = row.get("draft");
    let input_schema: sqlx::types::Json<JsonValue> = row.get("input_schema");
    let output_schema: sqlx::types::Json<JsonValue> = row.get("output_schema");

    Ok(SubRuleAsset {
        meta: row_to_sub_rule_meta(row)?,
        draft: draft.0,
        input_schema: input_schema.0,
        output_schema: output_schema.0,
    })
}
