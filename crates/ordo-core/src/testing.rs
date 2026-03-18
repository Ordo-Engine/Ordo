//! Rule testing framework.
//!
//! Provides test suite definitions and a runner that validates rule execution
//! results against expected outcomes. Used by both the CLI (`ordo test`) and
//! the server-side test API.
//!
//! # Test file format (YAML)
//!
//! ```yaml
//! description: "Discount policy tests"
//! tests:
//!   - name: "adult_discount"
//!     input: { age: 25 }
//!     expect:
//!       code: "ADULT"
//!       message: "Adult discount applied"
//!       output: { discount: 0.1 }
//!   - name: "error_on_invalid"
//!     input: { age: "bad" }
//!     expect:
//!       error: true
//! ```

use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::context::Value;
use crate::rule::{ExecutionOptions, ExecutionResult, RuleExecutor, RuleSet};

// ── Test Suite Definition ───────────────────────────────────────────

/// A collection of test cases for a rule set.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestSuite {
    /// Human-readable description.
    #[serde(default)]
    pub description: String,

    /// Individual test cases.
    pub tests: Vec<TestCase>,
}

/// A single test case.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestCase {
    /// Test name (must be unique within the suite).
    pub name: String,

    /// Optional description.
    #[serde(default)]
    pub description: String,

    /// Input data fed to the rule executor.
    pub input: Value,

    /// Expected outcome.
    pub expect: TestExpectation,

    /// Skip this test.
    #[serde(default)]
    pub skip: bool,
}

/// What we expect from the execution.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TestExpectation {
    /// Expected result code (exact match).
    #[serde(default)]
    pub code: Option<String>,

    /// Expected result message (exact match).
    #[serde(default)]
    pub message: Option<String>,

    /// Expected output value (deep equality).
    #[serde(default)]
    pub output: Option<Value>,

    /// Partial output match — assert that these fields exist with these values,
    /// ignoring any extra fields in the actual output.
    #[serde(default)]
    pub output_includes: Option<Value>,

    /// Expect the execution to fail with an error.
    #[serde(default)]
    pub error: bool,

    /// If `error` is true, optionally require the error message to contain this substring.
    #[serde(default)]
    pub error_contains: Option<String>,

    /// Trace assertions (requires trace to be enabled during execution).
    #[serde(default)]
    pub trace: Option<TraceExpectation>,

    /// Maximum allowed execution duration in microseconds.
    #[serde(default)]
    pub duration_max_us: Option<u64>,
}

/// Assertions on the execution trace.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TraceExpectation {
    /// Expected step IDs in execution order.
    #[serde(default)]
    pub path: Option<Vec<String>>,

    /// Expected number of steps executed.
    #[serde(default)]
    pub step_count: Option<usize>,

    /// Assert that at least one step with this ID was executed.
    #[serde(default)]
    pub contains_step: Option<String>,
}

// ── Test Results ────────────────────────────────────────────────────

/// Overall result of running a test suite.
#[derive(Debug, Clone, Serialize)]
pub struct TestSuiteResult {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_us: u64,
    pub results: Vec<TestCaseResult>,
}

/// Result of a single test case.
#[derive(Debug, Clone, Serialize)]
pub struct TestCaseResult {
    pub name: String,
    pub status: TestStatus,
    pub duration_us: u64,
    /// Populated on failure.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<String>,
    /// The actual execution result (for diagnostics).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<ActualResult>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TestStatus {
    Pass,
    Fail,
    Skip,
    Error,
}

/// Snapshot of the actual execution result for reporting.
#[derive(Debug, Clone, Serialize)]
pub struct ActualResult {
    pub code: String,
    pub message: String,
    pub output: Value,
}

// ── Runner ──────────────────────────────────────────────────────────

/// Run a test suite against a compiled rule set.
pub fn run_test_suite(
    executor: &RuleExecutor,
    ruleset: &RuleSet,
    suite: &TestSuite,
) -> TestSuiteResult {
    let start = Instant::now();
    let mut results = Vec::with_capacity(suite.tests.len());

    for tc in &suite.tests {
        results.push(run_single_test(executor, ruleset, tc));
    }

    let passed = results
        .iter()
        .filter(|r| r.status == TestStatus::Pass)
        .count();
    let failed = results
        .iter()
        .filter(|r| r.status == TestStatus::Fail)
        .count();
    let skipped = results
        .iter()
        .filter(|r| r.status == TestStatus::Skip)
        .count();

    TestSuiteResult {
        total: suite.tests.len(),
        passed,
        failed,
        skipped,
        duration_us: start.elapsed().as_micros() as u64,
        results,
    }
}

fn run_single_test(executor: &RuleExecutor, ruleset: &RuleSet, tc: &TestCase) -> TestCaseResult {
    if tc.skip {
        return TestCaseResult {
            name: tc.name.clone(),
            status: TestStatus::Skip,
            duration_us: 0,
            failures: vec![],
            actual: None,
        };
    }

    let needs_trace = tc.expect.trace.is_some();
    let options = ExecutionOptions {
        enable_trace: if needs_trace { Some(true) } else { None },
        ..Default::default()
    };

    let start = Instant::now();
    let exec_result: crate::error::Result<ExecutionResult> =
        executor.execute_with_options(ruleset, tc.input.clone(), Some(&options));
    let duration_us = start.elapsed().as_micros() as u64;

    match exec_result {
        Ok(result) => {
            if tc.expect.error {
                return TestCaseResult {
                    name: tc.name.clone(),
                    status: TestStatus::Fail,
                    duration_us,
                    failures: vec!["expected error, but execution succeeded".to_string()],
                    actual: Some(ActualResult {
                        code: result.code,
                        message: result.message,
                        output: result.output,
                    }),
                };
            }
            let failures = check_expectations(&tc.expect, &result, duration_us);
            let status = if failures.is_empty() {
                TestStatus::Pass
            } else {
                TestStatus::Fail
            };
            TestCaseResult {
                name: tc.name.clone(),
                status,
                duration_us,
                failures,
                actual: Some(ActualResult {
                    code: result.code,
                    message: result.message,
                    output: result.output,
                }),
            }
        }
        Err(err) => {
            if tc.expect.error {
                let mut failures = vec![];
                if let Some(ref pattern) = tc.expect.error_contains {
                    let msg = err.to_string();
                    if !msg.contains(pattern.as_str()) {
                        failures.push(format!(
                            "error message does not contain \"{}\", got: \"{}\"",
                            pattern, msg
                        ));
                    }
                }
                let status = if failures.is_empty() {
                    TestStatus::Pass
                } else {
                    TestStatus::Fail
                };
                TestCaseResult {
                    name: tc.name.clone(),
                    status,
                    duration_us,
                    failures,
                    actual: None,
                }
            } else {
                TestCaseResult {
                    name: tc.name.clone(),
                    status: TestStatus::Error,
                    duration_us,
                    failures: vec![format!("unexpected error: {}", err)],
                    actual: None,
                }
            }
        }
    }
}

/// Compare the actual result against all expectations, returning a list of failure messages.
fn check_expectations(
    expect: &TestExpectation,
    result: &ExecutionResult,
    duration_us: u64,
) -> Vec<String> {
    let mut failures = Vec::new();

    // Code
    if let Some(ref expected_code) = expect.code {
        if &result.code != expected_code {
            failures.push(format!(
                "code: expected \"{}\", got \"{}\"",
                expected_code, result.code
            ));
        }
    }

    // Message
    if let Some(ref expected_msg) = expect.message {
        if &result.message != expected_msg {
            failures.push(format!(
                "message: expected \"{}\", got \"{}\"",
                expected_msg, result.message
            ));
        }
    }

    // Output (deep equality)
    if let Some(ref expected_output) = expect.output {
        if &result.output != expected_output {
            failures.push(format!(
                "output: expected {}, got {}",
                serde_json::to_string(expected_output).unwrap_or_default(),
                serde_json::to_string(&result.output).unwrap_or_default(),
            ));
        }
    }

    // Output includes (partial match)
    if let Some(Value::Object(expected_fields)) = &expect.output_includes {
        match &result.output {
            Value::Object(actual_fields) => {
                for (key, expected_val) in expected_fields {
                    match actual_fields.get(key) {
                        Some(actual_val) if actual_val == expected_val => {}
                        Some(actual_val) => {
                            failures.push(format!(
                                "output_includes: field \"{}\" expected {}, got {}",
                                key,
                                serde_json::to_string(expected_val).unwrap_or_default(),
                                serde_json::to_string(actual_val).unwrap_or_default(),
                            ));
                        }
                        None => {
                            failures.push(format!(
                                "output_includes: field \"{}\" not found in output",
                                key
                            ));
                        }
                    }
                }
            }
            _ => {
                failures.push("output_includes: actual output is not an object".to_string());
            }
        }
    }

    // Duration
    if let Some(max_us) = expect.duration_max_us {
        if duration_us > max_us {
            failures.push(format!(
                "duration: expected <= {}µs, got {}µs",
                max_us, duration_us
            ));
        }
    }

    // Trace assertions
    if let Some(ref trace_expect) = expect.trace {
        match &result.trace {
            Some(trace) => {
                // Step path
                if let Some(ref expected_path) = trace_expect.path {
                    let actual_path: Vec<&str> =
                        trace.steps.iter().map(|s| s.step_id.as_str()).collect();
                    let expected_refs: Vec<&str> =
                        expected_path.iter().map(|s| s.as_str()).collect();
                    if actual_path != expected_refs {
                        failures.push(format!(
                            "trace.path: expected {:?}, got {:?}",
                            expected_path, actual_path
                        ));
                    }
                }

                // Step count
                if let Some(expected_count) = trace_expect.step_count {
                    if trace.steps.len() != expected_count {
                        failures.push(format!(
                            "trace.step_count: expected {}, got {}",
                            expected_count,
                            trace.steps.len()
                        ));
                    }
                }

                // Contains step
                if let Some(ref step_id) = trace_expect.contains_step {
                    if !trace.steps.iter().any(|s| &s.step_id == step_id) {
                        failures.push(format!(
                            "trace.contains_step: step \"{}\" not found in trace",
                            step_id
                        ));
                    }
                }
            }
            None => {
                failures.push(
                    "trace assertions specified but no trace was captured (enable trace)"
                        .to_string(),
                );
            }
        }
    }

    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::RuleSet;
    use std::collections::HashMap;

    fn make_simple_ruleset() -> RuleSet {
        let json = serde_json::json!({
            "config": {
                "name": "test_rule",
                "version": "1.0.0",
                "entry_step": "approve"
            },
            "steps": {
                "approve": {
                    "id": "approve",
                    "name": "Approve",
                    "type": "terminal",
                    "result": {
                        "code": "APPROVED",
                        "message": "Request approved",
                        "data": { "status": "ok" }
                    }
                }
            }
        });
        RuleSet::from_json_compiled(&json.to_string()).unwrap()
    }

    #[test]
    fn test_passing_test_case() {
        let executor = RuleExecutor::new();
        let ruleset = make_simple_ruleset();
        let suite = TestSuite {
            description: "basic test".to_string(),
            tests: vec![TestCase {
                name: "should_approve".to_string(),
                description: String::new(),
                input: Value::object(HashMap::new()),
                expect: TestExpectation {
                    code: Some("APPROVED".to_string()),
                    message: Some("Request approved".to_string()),
                    ..Default::default()
                },
                skip: false,
            }],
        };

        let result = run_test_suite(&executor, &ruleset, &suite);
        assert_eq!(result.total, 1);
        assert_eq!(result.passed, 1);
        assert_eq!(result.failed, 0);
        assert_eq!(result.results[0].status, TestStatus::Pass);
    }

    #[test]
    fn test_failing_code_mismatch() {
        let executor = RuleExecutor::new();
        let ruleset = make_simple_ruleset();
        let suite = TestSuite {
            description: String::new(),
            tests: vec![TestCase {
                name: "wrong_code".to_string(),
                description: String::new(),
                input: Value::object(HashMap::new()),
                expect: TestExpectation {
                    code: Some("REJECTED".to_string()),
                    ..Default::default()
                },
                skip: false,
            }],
        };

        let result = run_test_suite(&executor, &ruleset, &suite);
        assert_eq!(result.failed, 1);
        assert!(result.results[0].failures[0].contains("code"));
    }

    #[test]
    fn test_output_includes_partial_match() {
        let executor = RuleExecutor::new();
        let ruleset = make_simple_ruleset();
        let mut expected_fields = HashMap::new();
        expected_fields.insert("status".to_string(), Value::string("ok"));

        let suite = TestSuite {
            description: String::new(),
            tests: vec![TestCase {
                name: "partial".to_string(),
                description: String::new(),
                input: Value::object(HashMap::new()),
                expect: TestExpectation {
                    output_includes: Some(Value::object(expected_fields)),
                    ..Default::default()
                },
                skip: false,
            }],
        };

        let result = run_test_suite(&executor, &ruleset, &suite);
        assert_eq!(result.passed, 1);
    }

    #[test]
    fn test_skip() {
        let executor = RuleExecutor::new();
        let ruleset = make_simple_ruleset();
        let suite = TestSuite {
            description: String::new(),
            tests: vec![TestCase {
                name: "skipped".to_string(),
                description: String::new(),
                input: Value::object(HashMap::new()),
                expect: TestExpectation::default(),
                skip: true,
            }],
        };

        let result = run_test_suite(&executor, &ruleset, &suite);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.results[0].status, TestStatus::Skip);
    }

    #[test]
    fn test_trace_path_assertion() {
        let executor = RuleExecutor::new();
        let ruleset = make_simple_ruleset();
        let suite = TestSuite {
            description: String::new(),
            tests: vec![TestCase {
                name: "trace_check".to_string(),
                description: String::new(),
                input: Value::object(HashMap::new()),
                expect: TestExpectation {
                    trace: Some(TraceExpectation {
                        contains_step: Some("approve".to_string()),
                        step_count: Some(1),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                skip: false,
            }],
        };

        let result = run_test_suite(&executor, &ruleset, &suite);
        assert_eq!(result.passed, 1);
    }

    #[test]
    fn test_deserialize_yaml_suite() {
        let yaml = r#"
description: "example"
tests:
  - name: "basic"
    input: { age: 25 }
    expect:
      code: "OK"
      output_includes:
        discount: 0.1
  - name: "error_case"
    input: { age: "bad" }
    expect:
      error: true
      error_contains: "type"
  - name: "with_trace"
    input: {}
    expect:
      trace:
        path: ["step1", "step2"]
        step_count: 2
        contains_step: "step1"
"#;
        let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(suite.tests.len(), 3);
        assert_eq!(suite.tests[0].name, "basic");
        assert!(suite.tests[1].expect.error);
        assert_eq!(
            suite.tests[1].expect.error_contains,
            Some("type".to_string())
        );
        assert!(suite.tests[2].expect.trace.is_some());
        let trace = suite.tests[2].expect.trace.as_ref().unwrap();
        assert_eq!(
            trace.path,
            Some(vec!["step1".to_string(), "step2".to_string()])
        );
        assert_eq!(trace.step_count, Some(2));
    }
}
