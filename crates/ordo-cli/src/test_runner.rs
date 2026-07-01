use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use ordo_core::prelude::*;
use serde::Deserialize;

use crate::runtime::{execute_loaded_rule, load_rule};

#[derive(Args)]
pub struct TestArgs {
    /// Rule file (JSON, YAML, or .ordo)
    #[arg(long, value_name = "FILE")]
    rule: String,

    /// Test file (JSON or YAML)
    #[arg(long, value_name = "FILE")]
    tests: String,
}

#[derive(Deserialize)]
struct TestSuite {
    tests: Vec<TestCase>,
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
    let rule = load_rule(&args.rule)?;
    let suite = load_tests(&args.tests)?;
    let total = suite.tests.len();
    let mut cases = Vec::with_capacity(total);

    for test in &suite.tests {
        let start = std::time::Instant::now();
        let result = execute_loaded_rule(&rule, test.input.clone(), false);
        let duration_us = start.elapsed().as_micros();

        let failures = match result {
            Ok(result) => collect_failures(&test.expect, &result),
            Err(e) => vec![format!("execution error: {}", e)],
        };
        cases.push(CaseResult {
            name: test.name.clone(),
            passed: failures.is_empty(),
            failures,
            duration_us,
        });
    }

    let passed = cases.iter().filter(|c| c.passed).count();
    let failed = total - passed;

    if json {
        crate::output::emit_json(&serde_json::json!({
            "total": total,
            "passed": passed,
            "failed": failed,
            "cases": cases,
        }))?;
    } else {
        for c in &cases {
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

fn load_tests(path: &str) -> Result<TestSuite> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("Failed to read tests: {}", path))?;
    if path.ends_with(".yaml") || path.ends_with(".yml") {
        serde_yaml::from_str(&content).context("Failed to parse YAML tests")
    } else {
        serde_json::from_str(&content).context("Failed to parse JSON tests")
    }
}
