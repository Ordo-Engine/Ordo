use super::*;

impl PlatformStore {
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
}
