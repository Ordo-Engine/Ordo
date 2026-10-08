//! `ordo guard test` — run the guard policy's test cases.

use anyhow::Result;
use clap::Args;
use colored::Colorize;

use crate::project::Project;
use ordo_core::prelude::Value;

#[derive(Args)]
pub struct GuardTestArgs {
    /// Guard policy project directory (default: auto-discovered)
    #[arg(long, value_name = "DIR")]
    policy_dir: Option<String>,

    /// Ruleset to test within the policy project
    #[arg(long, default_value = super::DEFAULT_RULESET)]
    ruleset: String,
}

pub fn run(args: GuardTestArgs, json: bool) -> Result<()> {
    let policy_dir = super::resolve_policy_dir(args.policy_dir.as_deref()).ok_or_else(|| {
        anyhow::anyhow!(
            "no guard policy found (looked for {}/{}) — run `ordo guard init`",
            super::POLICY_DIR_NAME,
            crate::project::CONFIG_FILE
        )
    })?;
    let project = Project::discover(Some(&policy_dir))?;
    // Cases are written as `{tool, command, …}`; derive the shell facts the
    // live hook would add, so tests exercise the same input shape.
    let summary =
        crate::test_runner::run_project_ruleset_with(&project, &args.ruleset, prepare_case)?;

    let failed = summary["failed"].as_u64().unwrap_or(0);
    if json {
        crate::output::emit_json(&summary)?;
    } else {
        for case in summary["cases"].as_array().into_iter().flatten() {
            let name = case["name"].as_str().unwrap_or("?");
            let ms = case["duration_us"].as_u64().unwrap_or(0) as f64 / 1000.0;
            if case["passed"].as_bool().unwrap_or(false) {
                println!("{} {} ({ms:.3}ms)", "--- PASS:".green(), name);
            } else {
                println!("{} {} ({ms:.3}ms)", "--- FAIL:".red(), name);
                for f in case["failures"].as_array().into_iter().flatten() {
                    println!("    {}", f.as_str().unwrap_or_default());
                }
            }
        }
        let total = summary["total"].as_u64().unwrap_or(0);
        let passed = summary["passed"].as_u64().unwrap_or(0);
        println!();
        if failed > 0 {
            println!(
                "{total} tests: {} passed, {} failed",
                passed.to_string().green(),
                failed.to_string().red()
            );
        } else {
            println!("{total} tests: {} passed", passed.to_string().green());
        }
    }
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Add the hook's derived facts to a test case written as `{tool, command, …}`.
pub(crate) fn prepare_case(input: Value) -> Value {
    let Ok(serde_json::Value::Object(mut map)) = serde_json::to_value(&input) else {
        return input;
    };
    super::hook::add_shell_facts(&mut map);
    serde_json::from_value(serde_json::Value::Object(map)).unwrap_or(input)
}
