//! `ordo lint` — validation errors plus non-fatal style warnings.

use anyhow::Result;
use clap::Args;
use serde::Serialize;

use crate::project::Project;
use crate::validate::validate_one;

#[derive(Args)]
pub struct LintArgs {
    /// Ruleset name to lint (default: every ruleset in the project)
    name: Option<String>,
}

#[derive(Serialize)]
struct LintReport {
    ruleset: String,
    errors: Vec<String>,
    warnings: Vec<String>,
}

pub fn run(args: LintArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let names = match args.name {
        Some(n) => vec![crate::project::ruleset_name(&n)],
        None => project.ruleset_names()?,
    };
    let concepts = project.load_concepts()?;

    let mut reports = Vec::with_capacity(names.len());
    for name in &names {
        let report = validate_one(&project, name, &concepts);
        let errors: Vec<String> = report
            .errors
            .into_iter()
            .map(|e| match e.step_id {
                Some(s) => format!("[{s}] {}", e.message),
                None => e.message,
            })
            .collect();

        let mut warnings = Vec::new();
        let tests_path = project.tests_path(name);
        let has_tests = std::fs::read_to_string(&tests_path)
            .ok()
            .map(|t| t.trim() != "[]" && !t.trim().is_empty())
            .unwrap_or(false);
        if !has_tests {
            warnings.push("no test cases — add tests/<name>.json".to_string());
        }

        reports.push(LintReport {
            ruleset: name.clone(),
            errors,
            warnings,
        });
    }

    let has_errors = reports.iter().any(|r| !r.errors.is_empty());

    if json {
        crate::output::emit_json(&serde_json::json!({ "ok": !has_errors, "rulesets": reports }))?;
    } else {
        for r in &reports {
            println!(
                "{} {}",
                if r.errors.is_empty() { "✓" } else { "✗" },
                r.ruleset
            );
            for e in &r.errors {
                println!("    error: {e}");
            }
            for w in &r.warnings {
                println!("    warning: {w}");
            }
        }
    }

    if has_errors {
        std::process::exit(1);
    }
    Ok(())
}
