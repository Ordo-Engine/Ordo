use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use ordo_core::prelude::*;
use serde::Deserialize;

#[derive(Args)]
pub struct TestArgs {
    /// Rule file (JSON or YAML)
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

pub fn run(args: TestArgs) -> Result<()> {
    let ruleset = load_ruleset(&args.rule)?;
    let suite = load_tests(&args.tests)?;

    let executor = RuleExecutor::new();
    let mut passed = 0;
    let mut failed = 0;
    let total = suite.tests.len();

    for test in &suite.tests {
        let start = std::time::Instant::now();
        let result = executor.execute(&ruleset, test.input.clone());
        let elapsed = start.elapsed();

        match result {
            Ok(result) => {
                let mut failures = Vec::new();

                if let Some(expected_code) = &test.expect.code {
                    if &result.code != expected_code {
                        failures.push(format!(
                            "expected code: \"{}\", got: \"{}\"",
                            expected_code, result.code
                        ));
                    }
                }

                if let Some(expected_msg) = &test.expect.message {
                    if &result.message != expected_msg {
                        failures.push(format!(
                            "expected message: \"{}\", got: \"{}\"",
                            expected_msg, result.message
                        ));
                    }
                }

                if let Some(expected_output) = &test.expect.output {
                    if &result.output != expected_output {
                        failures.push(format!(
                            "output: expected {:?}, got {:?}",
                            expected_output, result.output
                        ));
                    }
                }

                if failures.is_empty() {
                    println!(
                        "{} {} ({:.3}ms)",
                        "--- PASS:".green(),
                        test.name,
                        elapsed.as_secs_f64() * 1000.0
                    );
                    passed += 1;
                } else {
                    println!(
                        "{} {} ({:.3}ms)",
                        "--- FAIL:".red(),
                        test.name,
                        elapsed.as_secs_f64() * 1000.0
                    );
                    for f in &failures {
                        println!("    {}", f);
                    }
                    failed += 1;
                }
            }
            Err(e) => {
                println!(
                    "{} {} ({:.3}ms)",
                    "--- FAIL:".red(),
                    test.name,
                    elapsed.as_secs_f64() * 1000.0
                );
                println!("    execution error: {}", e);
                failed += 1;
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
        std::process::exit(1);
    } else {
        println!("{} tests: {} passed", total, passed.to_string().green());
    }

    Ok(())
}

fn load_ruleset(path: &str) -> Result<RuleSet> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("Failed to read rule: {}", path))?;
    if path.ends_with(".yaml") || path.ends_with(".yml") {
        RuleSet::from_yaml_compiled(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML rule: {}", e))
    } else {
        RuleSet::from_json_compiled(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse JSON rule: {}", e))
    }
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
