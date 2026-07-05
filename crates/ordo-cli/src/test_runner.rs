use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use ordo_core::prelude::*;
use serde::Deserialize;

use crate::project::Project;
use crate::runtime::{execute_loaded_rule, load_rule, LoadedRule};
use std::path::Path;

#[derive(Args)]
pub struct TestArgs {
    /// Ruleset name to test (project mode). Omit to test every ruleset with tests.
    name: Option<String>,

    /// Standalone rule file instead of a project ruleset (JSON, YAML, or .ordo)
    #[arg(long, value_name = "FILE")]
    rule: Option<String>,

    /// Tests file (default: tests/<name>.json in the project)
    #[arg(long, value_name = "FILE")]
    tests: Option<String>,
}

/// On-disk tests are an array of cases; a legacy `{ "tests": [...] }` wrapper is
/// also accepted.
#[derive(Deserialize)]
#[serde(untagged)]
enum TestFile {
    Array(Vec<TestCase>),
    Wrapped { tests: Vec<TestCase> },
}

impl TestFile {
    fn into_cases(self) -> Vec<TestCase> {
        match self {
            TestFile::Array(v) => v,
            TestFile::Wrapped { tests } => tests,
        }
    }
}

#[derive(Deserialize)]
struct TestCase {
    name: String,
    input: Value,
    expect: TestExpectation,
}

#[derive(Deserialize)]
struct TestExpectation {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    output: Option<Value>,
}

/// The outcome of one evaluated test case.
#[derive(serde::Serialize)]
struct CaseResult {
    name: String,
    passed: bool,
    failures: Vec<String>,
    duration_us: u128,
}

pub fn run(args: TestArgs, json: bool) -> Result<()> {
    // File mode: an explicit `--rule` runs a standalone rule against a tests file.
    let suites: Vec<(String, Vec<CaseResult>)> = if let Some(rule_path) = args.rule.as_deref() {
        let tests_path = args
            .tests
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--tests <FILE> is required when using --rule"))?;
        let rule = load_rule(rule_path)?;
        let cases = run_cases(&rule, &load_tests(Path::new(&tests_path))?);
        let label = Path::new(rule_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("rule")
            .to_string();
        vec![(label, cases)]
    } else {
        // Project mode: resolve ruleset(s) in the discovered project.
        let project = Project::discover(None)?;
        let explicit = args.name.is_some();
        let names = match &args.name {
            Some(n) => vec![crate::project::ruleset_name(n)],
            None => project.ruleset_names()?,
        };
        let mut suites = Vec::new();
        for name in &names {
            let tests_path = args
                .tests
                .clone()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| project.tests_path(name));
            if !tests_path.is_file() {
                if explicit {
                    anyhow::bail!("no tests file for '{name}' ({})", tests_path.display());
                }
                continue;
            }
            let mut engine = project.load_engine(name)?;
            engine
                .compile()
                .map_err(|e| anyhow::anyhow!("compile error in {name}: {e}"))?;
            let cases = run_cases(&LoadedRule::Source(engine), &load_tests(&tests_path)?);
            suites.push((name.clone(), cases));
        }
        suites
    };

    report(suites, json)
}

/// Run a project ruleset's tests and return a JSON summary (used by `ordo mcp`).
pub(crate) fn run_project_ruleset(project: &Project, name: &str) -> Result<serde_json::Value> {
    let tests = load_tests(&project.tests_path(name))?;
    let mut engine = project.load_engine(name)?;
    engine
        .compile()
        .map_err(|e| anyhow::anyhow!("compile error in {name}: {e}"))?;
    let cases = run_cases(&LoadedRule::Source(engine), &tests);
    let passed = cases.iter().filter(|c| c.passed).count();
    Ok(serde_json::json!({
        "total": cases.len(), "passed": passed, "failed": cases.len() - passed, "cases": cases,
    }))
}

fn run_cases(rule: &LoadedRule, tests: &[TestCase]) -> Vec<CaseResult> {
    tests
        .iter()
        .map(|test| {
            let start = std::time::Instant::now();
            let result = execute_loaded_rule(rule, test.input.clone(), false);
            let duration_us = start.elapsed().as_micros();
            let failures = match result {
                Ok(r) => collect_failures(&test.expect, &r),
                Err(e) => vec![format!("execution error: {}", e)],
            };
            CaseResult {
                name: test.name.clone(),
                passed: failures.is_empty(),
                failures,
                duration_us,
            }
        })
        .collect()
}

fn report(suites: Vec<(String, Vec<CaseResult>)>, json: bool) -> Result<()> {
    let total: usize = suites.iter().map(|(_, c)| c.len()).sum();
    let passed: usize = suites
        .iter()
        .flat_map(|(_, c)| c)
        .filter(|c| c.passed)
        .count();
    let failed = total - passed;

    if json {
        let suite_json: Vec<_> = suites
            .iter()
            .map(|(name, cases)| {
                serde_json::json!({
                    "ruleset": name,
                    "passed": cases.iter().filter(|c| c.passed).count(),
                    "failed": cases.iter().filter(|c| !c.passed).count(),
                    "cases": cases,
                })
            })
            .collect();
        crate::output::emit_json(&serde_json::json!({
            "total": total, "passed": passed, "failed": failed, "suites": suite_json,
        }))?;
    } else {
        let multi = suites.len() > 1;
        for (name, cases) in &suites {
            if multi {
                println!("{}", name.as_str().bold());
            }
            for c in cases {
                let ms = c.duration_us as f64 / 1000.0;
                if c.passed {
                    println!("{} {} ({:.3}ms)", "--- PASS:".green(), c.name, ms);
                } else {
                    println!("{} {} ({:.3}ms)", "--- FAIL:".red(), c.name, ms);
                    for f in &c.failures {
                        println!("    {}", f);
                    }
                }
            }
        }
        println!();
        if failed > 0 {
            println!(
                "{} tests: {} passed, {} failed",
                total,
                passed.to_string().green(),
                failed.to_string().red()
            );
        } else {
            println!("{} tests: {} passed", total, passed.to_string().green());
        }
    }

    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Diff an execution result against an expected code/output, reusing the exact
/// wording of `ordo test`. Used by `ordo replay` to describe how a decision
/// flipped against a captured baseline. Returns the (possibly empty) list of
/// human-readable mismatch descriptions.
pub(crate) fn diff_result(
    expected_code: Option<String>,
    expected_output: Option<Value>,
    result: &ExecutionResult,
) -> Vec<String> {
    let expect = TestExpectation {
        code: expected_code,
        message: None,
        output: expected_output,
    };
    collect_failures(&expect, result)
}

/// Compare an execution result against a test's expectations.
fn collect_failures(expect: &TestExpectation, result: &ExecutionResult) -> Vec<String> {
    let mut failures = Vec::new();
    if let Some(expected_code) = &expect.code {
        if &result.code != expected_code {
            failures.push(format!(
                "expected code: \"{}\", got: \"{}\"",
                expected_code, result.code
            ));
        }
    }
    if let Some(expected_msg) = &expect.message {
        if &result.message != expected_msg {
            failures.push(format!(
                "expected message: \"{}\", got: \"{}\"",
                expected_msg, result.message
            ));
        }
    }
    if let Some(expected_output) = &expect.output {
        if &result.output != expected_output {
            failures.push(format!(
                "output: expected {:?}, got {:?}",
                expected_output, result.output
            ));
        }
    }
    failures
}

fn load_tests(path: &Path) -> Result<Vec<TestCase>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read tests: {}", path.display()))?;
    let is_yaml = matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("yaml") | Some("yml")
    );
    let file: TestFile = if is_yaml {
        serde_yaml::from_str(&content).context("failed to parse YAML tests")?
    } else {
        serde_json::from_str(&content).context("failed to parse JSON tests")?
    };
    Ok(file.into_cases())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_accepts_array_and_wrapped_shapes() {
        let arr: TestFile =
            serde_json::from_str(r#"[{"name":"a","input":{},"expect":{"code":"OK"}}]"#).unwrap();
        assert_eq!(arr.into_cases().len(), 1);
        let wrapped: TestFile =
            serde_json::from_str(r#"{"tests":[{"name":"a","input":{},"expect":{}}]}"#).unwrap();
        assert_eq!(wrapped.into_cases().len(), 1);
    }
}
