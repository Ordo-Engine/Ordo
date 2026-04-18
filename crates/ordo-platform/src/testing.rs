//! Rule testing system handlers (M1.2).
//!
//! Endpoints:
//!
//!   GET    /api/v1/projects/:pid/rulesets/:name/tests          — list test cases
//!   POST   /api/v1/projects/:pid/rulesets/:name/tests          — create test case (Editor+)
//!   PUT    /api/v1/projects/:pid/rulesets/:name/tests/:tid     — update test case (Editor+)
//!   DELETE /api/v1/projects/:pid/rulesets/:name/tests/:tid     — delete test case (Editor+)
//!   POST   /api/v1/projects/:pid/rulesets/:name/tests/run      — run all tests for a ruleset
//!   POST   /api/v1/projects/:pid/rulesets/:name/tests/:tid/run — run a single test case
//!   GET    /api/v1/projects/:pid/tests/run                     — run all tests across all rulesets
//!   GET    /api/v1/projects/:pid/rulesets/:name/tests/export   — export as ordo-cli YAML/JSON

use crate::{
    catalog::resolve_project,
    error::{ApiResult, PlatformError},
    models::{
        Claims, ProjectTestRunResult, Role, RulesetTestSummary, TestCase, TestExpectation,
        TestRunResult,
    },
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    response::Response,
    Extension, Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TestCaseInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub input: JsonValue,
    pub expect: TestExpectation,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
pub struct ExportQuery {
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "yaml".to_string()
}

// ── ordo-cli compatible export format ────────────────────────────────────────

/// Matches the TestCase format expected by ordo-cli's test_runner.rs.
#[derive(Serialize)]
struct CliTestCase {
    name: String,
    input: JsonValue,
    expect: CliExpectation,
}

#[derive(Serialize)]
struct CliExpectation {
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<JsonValue>,
}

#[derive(Serialize)]
struct CliTestSuite {
    tests: Vec<CliTestCase>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /api/v1/projects/:pid/rulesets/:name/tests
pub async fn list_tests(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name)): Path<(String, String)>,
) -> ApiResult<Json<Vec<TestCase>>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;

    let tests = state
        .store
        .get_tests(&org_id, &project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(tests))
}

/// POST /api/v1/projects/:pid/rulesets/:name/tests
pub async fn create_test(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name)): Path<(String, String)>,
    Json(req): Json<TestCaseInput>,
) -> ApiResult<Json<TestCase>> {
    let (org_id, _role) =
        resolve_project(&state, &project_id, &claims.sub, Some(Role::Editor)).await?;

    let name = req.name.trim();
    if name.is_empty() {
        return Err(PlatformError::bad_request("Test case name is required"));
    }

    let now = Utc::now();
    let tc = TestCase {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        description: req.description,
        input: req.input,
        expect: req.expect,
        tags: req.tags,
        created_at: now,
        updated_at: now,
        created_by: claims.sub.clone(),
    };

    let mut tests = state
        .store
        .get_tests(&org_id, &project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    tests.push(tc.clone());

    state
        .store
        .save_tests(&org_id, &project_id, &ruleset_name, &tests)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(tc))
}

/// PUT /api/v1/projects/:pid/rulesets/:name/tests/:tid
pub async fn update_test(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name, test_id)): Path<(String, String, String)>,
    Json(req): Json<TestCaseInput>,
) -> ApiResult<Json<TestCase>> {
    let (org_id, _role) =
        resolve_project(&state, &project_id, &claims.sub, Some(Role::Editor)).await?;

    let name = req.name.trim();
    if name.is_empty() {
        return Err(PlatformError::bad_request("Test case name is required"));
    }

    let mut tests = state
        .store
        .get_tests(&org_id, &project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    let tc = tests
        .iter_mut()
        .find(|t| t.id == test_id)
        .ok_or_else(|| PlatformError::not_found("Test case not found"))?;

    tc.name = name.to_string();
    tc.description = req.description;
    tc.input = req.input;
    tc.expect = req.expect;
    tc.tags = req.tags;
    tc.updated_at = Utc::now();

    let updated = tc.clone();

    state
        .store
        .save_tests(&org_id, &project_id, &ruleset_name, &tests)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(Json(updated))
}

/// DELETE /api/v1/projects/:pid/rulesets/:name/tests/:tid
pub async fn delete_test(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name, test_id)): Path<(String, String, String)>,
) -> ApiResult<axum::http::StatusCode> {
    let (org_id, _role) =
        resolve_project(&state, &project_id, &claims.sub, Some(Role::Editor)).await?;

    let mut tests = state
        .store
        .get_tests(&org_id, &project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    let before = tests.len();
    tests.retain(|t| t.id != test_id);

    if tests.len() == before {
        return Err(PlatformError::not_found("Test case not found"));
    }

    state
        .store
        .save_tests(&org_id, &project_id, &ruleset_name, &tests)
        .await
        .map_err(PlatformError::Internal)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// POST /api/v1/projects/:pid/rulesets/:name/tests/run
pub async fn run_ruleset_tests(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name)): Path<(String, String)>,
) -> ApiResult<Json<Vec<TestRunResult>>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;

    let tests = state
        .store
        .get_tests(&org_id, &project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    let results = execute_tests(&state, &project_id, &ruleset_name, &tests).await?;
    Ok(Json(results))
}

/// POST /api/v1/projects/:pid/rulesets/:name/tests/:tid/run
pub async fn run_one_test(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name, test_id)): Path<(String, String, String)>,
) -> ApiResult<Json<TestRunResult>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;

    let tests = state
        .store
        .get_tests(&org_id, &project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    let tc = tests
        .iter()
        .find(|t| t.id == test_id)
        .ok_or_else(|| PlatformError::not_found("Test case not found"))?;

    let mut results =
        execute_tests(&state, &project_id, &ruleset_name, std::slice::from_ref(tc)).await?;
    Ok(Json(results.remove(0)))
}

/// GET /api/v1/projects/:pid/tests/run
pub async fn run_project_tests(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> ApiResult<Json<ProjectTestRunResult>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;

    // Discover all ruleset names from test files in the project's asset directory.
    // We enumerate files matching `{pid}_tests_{name}.json` in the projects dir.
    let ruleset_names = list_test_ruleset_names(&state, &org_id, &project_id).await;

    let mut rulesets = Vec::new();
    let mut total = 0u32;
    let mut passed_total = 0u32;
    let mut failed_total = 0u32;

    for rname in &ruleset_names {
        let tests = state
            .store
            .get_tests(&org_id, &project_id, rname)
            .await
            .unwrap_or_default();

        if tests.is_empty() {
            continue;
        }

        let results = execute_tests(&state, &project_id, rname, &tests)
            .await
            .unwrap_or_else(|_| {
                tests
                    .iter()
                    .map(|t| TestRunResult {
                        test_id: t.id.clone(),
                        test_name: t.name.clone(),
                        passed: false,
                        failures: vec!["engine error".to_string()],
                        duration_us: 0,
                        actual_code: None,
                        actual_output: None,
                    })
                    .collect()
            });

        let r_total = results.len() as u32;
        let r_passed = results.iter().filter(|r| r.passed).count() as u32;
        let r_failed = r_total - r_passed;

        total += r_total;
        passed_total += r_passed;
        failed_total += r_failed;

        rulesets.push(RulesetTestSummary {
            ruleset_name: rname.clone(),
            total: r_total,
            passed: r_passed,
            failed: r_failed,
            results,
        });
    }

    Ok(Json(ProjectTestRunResult {
        total,
        passed: passed_total,
        failed: failed_total,
        rulesets,
    }))
}

/// GET /api/v1/projects/:pid/rulesets/:name/tests/export?format=yaml
pub async fn export_tests(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name)): Path<(String, String)>,
    Query(query): Query<ExportQuery>,
) -> ApiResult<Response<axum::body::Body>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;

    let tests = state
        .store
        .get_tests(&org_id, &project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    let suite = CliTestSuite {
        tests: tests
            .iter()
            .map(|t| CliTestCase {
                name: t.name.clone(),
                input: t.input.clone(),
                expect: CliExpectation {
                    code: t.expect.code.clone(),
                    message: t.expect.message.clone(),
                    output: t.expect.output.clone(),
                },
            })
            .collect(),
    };

    let use_yaml = query.format != "json";

    let (body, content_type, filename_ext) = if use_yaml {
        let yaml = serde_yaml::to_string(&suite)
            .map_err(|e| PlatformError::internal(format!("YAML serialization failed: {}", e)))?;
        (yaml.into_bytes(), "application/yaml", "yaml")
    } else {
        let json =
            serde_json::to_string_pretty(&suite).map_err(|e| PlatformError::Internal(e.into()))?;
        (json.into_bytes(), "application/json", "json")
    };

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

    let disposition = format!(
        "attachment; filename=\"{}_tests.{}\"",
        safe_name, filename_ext
    );

    let response = axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Content-Disposition", disposition)
        .body(axum::body::Body::from(body))
        .map_err(|e| PlatformError::internal(format!("Response build failed: {}", e)))?;

    Ok(response)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Execute a batch of test cases against the engine and return run results.
async fn execute_tests(
    state: &AppState,
    project_id: &str,
    ruleset_name: &str,
    tests: &[TestCase],
) -> ApiResult<Vec<TestRunResult>> {
    let execute_url = format!(
        "{}/api/v1/execute/{}",
        state.config.engine_url, ruleset_name
    );

    let mut results = Vec::with_capacity(tests.len());

    for tc in tests {
        let start = std::time::Instant::now();

        let body = serde_json::json!({ "input": tc.input });

        let res = state
            .http_client
            .post(&execute_url)
            .header("X-Tenant-ID", project_id)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        let elapsed_us = start.elapsed().as_micros() as u64;

        let run_result = match res {
            Err(e) => TestRunResult {
                test_id: tc.id.clone(),
                test_name: tc.name.clone(),
                passed: false,
                failures: vec![format!("Engine unreachable: {}", e)],
                duration_us: elapsed_us,
                actual_code: None,
                actual_output: None,
            },
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status().as_u16();
                    let body_text = resp.text().await.unwrap_or_default();
                    TestRunResult {
                        test_id: tc.id.clone(),
                        test_name: tc.name.clone(),
                        passed: false,
                        failures: vec![format!("Engine returned HTTP {}: {}", status, body_text)],
                        duration_us: elapsed_us,
                        actual_code: None,
                        actual_output: None,
                    }
                } else {
                    match resp.json::<JsonValue>().await {
                        Err(e) => TestRunResult {
                            test_id: tc.id.clone(),
                            test_name: tc.name.clone(),
                            passed: false,
                            failures: vec![format!("Failed to parse engine response: {}", e)],
                            duration_us: elapsed_us,
                            actual_code: None,
                            actual_output: None,
                        },
                        Ok(json) => {
                            // Engine returns {code, message, output, duration_us} directly
                            let actual_code = json
                                .get("code")
                                .and_then(|c| c.as_str())
                                .map(str::to_string);
                            let actual_message = json
                                .get("message")
                                .and_then(|m| m.as_str())
                                .map(str::to_string);
                            let actual_output = json.get("output").cloned();

                            let mut failures = Vec::new();

                            // Assert code
                            if let Some(ref expected_code) = tc.expect.code {
                                match &actual_code {
                                    None => failures.push(format!(
                                        "expected code \"{}\" but got none",
                                        expected_code
                                    )),
                                    Some(got) if got != expected_code => failures.push(format!(
                                        "expected code \"{}\", got \"{}\"",
                                        expected_code, got
                                    )),
                                    _ => {}
                                }
                            }

                            // Assert message
                            if let Some(ref expected_msg) = tc.expect.message {
                                match &actual_message {
                                    None => failures.push(format!(
                                        "expected message \"{}\" but got none",
                                        expected_msg
                                    )),
                                    Some(got) if got != expected_msg => failures.push(format!(
                                        "expected message \"{}\", got \"{}\"",
                                        expected_msg, got
                                    )),
                                    _ => {}
                                }
                            }

                            // Assert output fields (per-field, only check supplied keys)
                            if let Some(ref expected_output) = tc.expect.output {
                                if let Some(exp_obj) = expected_output.as_object() {
                                    for (key, exp_val) in exp_obj {
                                        let got_val =
                                            actual_output.as_ref().and_then(|o| o.get(key));
                                        match got_val {
                                            None => failures.push(format!(
                                                "output.{}: expected {:?} but field is missing",
                                                key, exp_val
                                            )),
                                            Some(got) if got != exp_val => failures.push(format!(
                                                "output.{}: expected {}, got {}",
                                                key, exp_val, got
                                            )),
                                            _ => {}
                                        }
                                    }
                                }
                            }

                            TestRunResult {
                                test_id: tc.id.clone(),
                                test_name: tc.name.clone(),
                                passed: failures.is_empty(),
                                failures,
                                duration_us: elapsed_us,
                                actual_code,
                                actual_output,
                            }
                        }
                    }
                }
            }
        };

        results.push(run_result);
    }

    Ok(results)
}

/// Discover all ruleset names that have test cases for this project.
async fn list_test_ruleset_names(state: &AppState, _org_id: &str, project_id: &str) -> Vec<String> {
    state.store.list_test_rulesets(project_id).await
}
