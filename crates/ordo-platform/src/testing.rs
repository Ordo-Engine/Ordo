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
//!   GET/POST /api/v1/projects/:pid/tests/run                   — run all tests across all rulesets
//!   GET    /api/v1/projects/:pid/rulesets/:name/tests/export   — export as ordo-cli YAML/JSON

use crate::{
    catalog::resolve_project,
    error::{ApiResult, PlatformError},
    models::{
        Claims, ProjectTestRunResult, Role, RulesetTestSummary, TestCase, TestExecutionTrace,
        TestExecutionTraceStep, TestExpectation, TestRunResult,
    },
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    response::Response,
    Extension, Json,
};
use chrono::Utc;
use ordo_core::{
    context::Value as CoreValue,
    rule::{ExecutionOptions, RuleExecutor, RuleSet},
    trace::ExecutionTrace,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
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

#[derive(Debug, Default, Deserialize)]
pub struct TestRunRequest {
    #[serde(default)]
    pub ruleset: Option<JsonValue>,
    #[serde(default = "default_include_trace")]
    pub include_trace: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct ProjectTestRunRequest {
    #[serde(default)]
    pub rulesets: HashMap<String, JsonValue>,
    #[serde(default = "default_include_trace")]
    pub include_trace: bool,
}

fn default_include_trace() -> bool {
    true
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
    req: Option<Json<TestRunRequest>>,
) -> ApiResult<Json<Vec<TestRunResult>>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;
    let req = req.map(|payload| payload.0).unwrap_or_default();

    let tests = state
        .store
        .get_tests(&org_id, &project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    let results = execute_tests(
        &state,
        &project_id,
        &ruleset_name,
        &tests,
        req.ruleset.as_ref(),
        req.include_trace,
    )
    .await?;
    Ok(Json(results))
}

/// POST /api/v1/projects/:pid/rulesets/:name/tests/:tid/run
pub async fn run_one_test(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((project_id, ruleset_name, test_id)): Path<(String, String, String)>,
    req: Option<Json<TestRunRequest>>,
) -> ApiResult<Json<TestRunResult>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;
    let req = req.map(|payload| payload.0).unwrap_or_default();

    let tests = state
        .store
        .get_tests(&org_id, &project_id, &ruleset_name)
        .await
        .map_err(PlatformError::Internal)?;

    let tc = tests
        .iter()
        .find(|t| t.id == test_id)
        .ok_or_else(|| PlatformError::not_found("Test case not found"))?;

    let mut results = execute_tests(
        &state,
        &project_id,
        &ruleset_name,
        std::slice::from_ref(tc),
        req.ruleset.as_ref(),
        req.include_trace,
    )
    .await?;
    Ok(Json(results.remove(0)))
}

/// GET /api/v1/projects/:pid/tests/run
pub async fn run_project_tests(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    req: Option<Json<ProjectTestRunRequest>>,
) -> ApiResult<Json<ProjectTestRunResult>> {
    let (org_id, _role) = resolve_project(&state, &project_id, &claims.sub, None).await?;
    let req = req.map(|payload| payload.0).unwrap_or_default();

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

        let results = execute_tests(
            &state,
            &project_id,
            rname,
            &tests,
            req.rulesets.get(rname),
            req.include_trace,
        )
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
                        actual_message: None,
                        actual_output: None,
                        trace: None,
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
    provided_ruleset: Option<&JsonValue>,
    include_trace: bool,
) -> ApiResult<Vec<TestRunResult>> {
    let ruleset_json = resolve_execution_ruleset(state, project_id, ruleset_name, provided_ruleset)
        .await?;
    let ruleset = compile_ruleset(&ruleset_json).map_err(|e| {
        PlatformError::bad_request(format!("Failed to compile ruleset for test execution: {}", e))
    })?;
    let executor = RuleExecutor::new();
    let mut results = Vec::with_capacity(tests.len());

    for tc in tests {
        let start = std::time::Instant::now();
        let input: CoreValue = match serde_json::from_value(tc.input.clone()) {
            Ok(input) => input,
            Err(e) => {
                results.push(TestRunResult {
                    test_id: tc.id.clone(),
                    test_name: tc.name.clone(),
                    passed: false,
                    failures: vec![format!("Invalid test input: {}", e)],
                    duration_us: start.elapsed().as_micros() as u64,
                    actual_code: None,
                    actual_message: None,
                    actual_output: None,
                    trace: None,
                });
                continue;
            }
        };

        let options = ExecutionOptions::default().trace(include_trace);
        let exec = executor.execute_with_options(&ruleset, input, Some(&options));
        let elapsed_us = start.elapsed().as_micros() as u64;

        let run_result = match exec {
            Ok(result) => {
                let actual_code = Some(result.code.clone());
                let actual_message = Some(result.message.clone());
                let actual_output = serde_json::to_value(&result.output).ok();
                let trace = result.trace.as_ref().map(map_trace);
                let failures = compare_expectation(
                    &tc.expect,
                    actual_code.as_deref(),
                    actual_message.as_deref(),
                    actual_output.as_ref(),
                );

                TestRunResult {
                    test_id: tc.id.clone(),
                    test_name: tc.name.clone(),
                    passed: failures.is_empty(),
                    failures,
                    duration_us: elapsed_us.max(result.duration_us),
                    actual_code,
                    actual_message,
                    actual_output,
                    trace,
                }
            }
            Err(e) => TestRunResult {
                test_id: tc.id.clone(),
                test_name: tc.name.clone(),
                passed: false,
                failures: vec![format!("Execution failed: {}", e)],
                duration_us: elapsed_us,
                actual_code: None,
                actual_message: None,
                actual_output: None,
                trace: None,
            },
        };

        results.push(run_result);
    }

    Ok(results)
}

/// Discover all ruleset names that have test cases for this project.
async fn list_test_ruleset_names(state: &AppState, _org_id: &str, project_id: &str) -> Vec<String> {
    state.store.list_test_rulesets(project_id).await
}

async fn resolve_execution_ruleset(
    state: &AppState,
    project_id: &str,
    ruleset_name: &str,
    provided_ruleset: Option<&JsonValue>,
) -> ApiResult<JsonValue> {
    if let Some(ruleset) = provided_ruleset {
        return Ok(ruleset.clone());
    }

    let draft = state
        .store
        .get_draft_ruleset(project_id, ruleset_name)
        .await
        .map_err(PlatformError::Internal)?
        .ok_or_else(|| PlatformError::not_found("Ruleset draft not found"))?;

    if looks_like_engine_ruleset(&draft.draft) {
        return Ok(draft.draft);
    }

    Err(PlatformError::bad_request(
        "Engine-format ruleset payload is required for local test execution",
    ))
}

fn looks_like_engine_ruleset(ruleset: &JsonValue) -> bool {
    ruleset
        .get("config")
        .and_then(|config| config.get("entry_step"))
        .and_then(|value| value.as_str())
        .is_some()
        && ruleset.get("steps").is_some_and(JsonValue::is_object)
}

fn compile_ruleset(ruleset: &JsonValue) -> anyhow::Result<RuleSet> {
    let ruleset_json = serde_json::to_string(ruleset)?;
    RuleSet::from_json_compiled(&ruleset_json).map_err(|e| anyhow::anyhow!("{}", e))
}

fn compare_expectation(
    expect: &TestExpectation,
    actual_code: Option<&str>,
    actual_message: Option<&str>,
    actual_output: Option<&JsonValue>,
) -> Vec<String> {
    let mut failures = Vec::new();

    if let Some(expected_code) = &expect.code {
        match actual_code {
            Some(actual) if actual == expected_code => {}
            Some(actual) => failures.push(format!(
                "code: expected \"{}\", got \"{}\"",
                expected_code, actual
            )),
            None => failures.push(format!("code: expected \"{}\", got none", expected_code)),
        }
    }

    if let Some(expected_message) = &expect.message {
        match actual_message {
            Some(actual) if actual == expected_message => {}
            Some(actual) => failures.push(format!(
                "message: expected \"{}\", got \"{}\"",
                expected_message, actual
            )),
            None => failures.push(format!(
                "message: expected \"{}\", got none",
                expected_message
            )),
        }
    }

    if let Some(expected_output) = &expect.output {
        match (expected_output.as_object(), actual_output.and_then(JsonValue::as_object)) {
            (Some(expected_fields), Some(actual_fields)) => {
                for (key, expected_val) in expected_fields {
                    match actual_fields.get(key) {
                        Some(actual) if actual == expected_val => {}
                        Some(actual) => failures.push(format!(
                            "output.{}: expected {}, got {}",
                            key,
                            json_string(expected_val),
                            json_string(actual)
                        )),
                        None => failures.push(format!(
                            "output.{}: expected {}, got missing",
                            key,
                            json_string(expected_val)
                        )),
                    }
                }
            }
            _ => match actual_output {
                Some(actual) if actual == expected_output => {}
                Some(actual) => failures.push(format!(
                    "output: expected {}, got {}",
                    json_string(expected_output),
                    json_string(actual)
                )),
                None => failures.push(format!(
                    "output: expected {}, got none",
                    json_string(expected_output)
                )),
            },
        }
    }

    failures
}

fn json_string(value: &JsonValue) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

fn map_trace(trace: &ExecutionTrace) -> TestExecutionTrace {
    TestExecutionTrace {
        trace_id: trace.trace_id.clone(),
        path: trace.steps.iter().map(|step| step.step_id.clone()).collect(),
        path_string: trace.path_string(),
        result_code: trace.result_code.clone(),
        total_duration_us: trace.total_duration_us,
        error: trace.error.clone(),
        steps: trace
            .steps
            .iter()
            .map(|step| TestExecutionTraceStep {
                id: step.step_id.clone(),
                name: step.step_name.clone(),
                duration_us: step.duration_us,
                next_step: step.next_step.clone(),
                is_terminal: step.is_terminal,
                input_snapshot: step
                    .input_snapshot
                    .as_ref()
                    .and_then(|value| serde_json::to_value(value).ok()),
                variables_snapshot: step
                    .variables_snapshot
                    .as_ref()
                    .and_then(|value| serde_json::to_value(value).ok()),
            })
            .collect(),
    }
}
