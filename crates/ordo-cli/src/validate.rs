//! `ordo validate` — compile a ruleset offline and report structured errors.
//!
//! Runs the same pipeline the platform's `convert` endpoint does: studio→engine
//! conversion (per-step error attribution), concept materialization, graph
//! validation, and expression compilation.

use anyhow::Result;
use clap::Args;
use ordo_core::prelude::RuleSet;
use ordo_studio_format::{materialize_concepts, ConvertError};
use serde::Serialize;
use std::path::Path;

use crate::project::Project;

#[derive(Args)]
pub struct ValidateArgs {
    /// Ruleset name to validate (default: every ruleset in the project)
    name: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct RulesetReport {
    pub ruleset: String,
    pub ok: bool,
    pub errors: Vec<ValidationError>,
}

#[derive(Serialize)]
pub(crate) struct ValidationError {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    pub message: String,
}

pub fn run(args: ValidateArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let validating_whole_project = args.name.is_none();
    let names = match args.name {
        Some(n) => vec![crate::project::ruleset_name(&n)],
        None => project.ruleset_names()?,
    };
    let concepts = project.load_concepts()?;

    let mut reports = Vec::with_capacity(names.len() + 2);
    for name in &names {
        reports.push(validate_one(&project, name, &concepts));
    }
    // Catalog-level (not per-ruleset), but a bad data_type/null_policy here
    // would otherwise only surface as a 4xx from `ordo push` — same class of
    // problem `validate_one` already catches for rulesets, extended to the
    // two files it never touched. Only when validating the whole project —
    // `ordo validate <one-ruleset>` shouldn't also report on unrelated files.
    if validating_whole_project {
        if let Some(report) = validate_catalog_file(&project.facts_path(), "facts.json", |v| {
            crate::catalog::validate_facts(v)
        })? {
            reports.push(report);
        }
        if let Some(report) =
            validate_catalog_file(&project.concepts_path(), "concepts.json", |v| {
                crate::catalog::validate_concepts(v)
            })?
        {
            reports.push(report);
        }
    }

    let all_ok = reports.iter().all(|r| r.ok);

    if json {
        crate::output::emit_json(&serde_json::json!({ "ok": all_ok, "rulesets": reports }))?;
    } else {
        for r in &reports {
            if r.ok {
                println!("✓ {}", r.ruleset);
            } else {
                println!("✗ {}", r.ruleset);
                for e in &r.errors {
                    match &e.step_id {
                        Some(s) => println!("    [{}] {}", s, e.message),
                        None => println!("    {}", e.message),
                    }
                }
            }
        }
        if names.is_empty() {
            println!("(no rulesets found)");
        }
    }

    if !all_ok {
        std::process::exit(1);
    }
    Ok(())
}

pub(crate) fn validate_one(
    project: &Project,
    name: &str,
    concepts: &[ordo_studio_format::ConceptDefinition],
) -> RulesetReport {
    let mut errors = Vec::new();

    let studio = match project.load_studio(name) {
        Ok(s) => s,
        Err(e) => {
            return RulesetReport {
                ruleset: name.to_string(),
                ok: false,
                errors: vec![ValidationError {
                    step_id: None,
                    // `{:#}` includes the cause chain (e.g. the serde detail of
                    // *why* the file is malformed), not just the outer context.
                    message: format!("{e:#}"),
                }],
            };
        }
    };

    // studio → engine (per-step attribution on failure)
    let mut engine: RuleSet = match RuleSet::try_from(studio) {
        Ok(e) => e,
        Err(ConvertError::Expr(step_id, msg)) => {
            errors.push(ValidationError {
                step_id: Some(step_id),
                message: msg,
            });
            return RulesetReport {
                ruleset: name.to_string(),
                ok: false,
                errors,
            };
        }
        Err(e) => {
            errors.push(ValidationError {
                step_id: None,
                message: e.to_string(),
            });
            return RulesetReport {
                ruleset: name.to_string(),
                ok: false,
                errors,
            };
        }
    };

    // materialize concepts
    if let Err(e) = materialize_concepts(&mut engine, concepts) {
        errors.push(ValidationError {
            step_id: None,
            message: e.to_string(),
        });
        return RulesetReport {
            ruleset: name.to_string(),
            ok: false,
            errors,
        };
    }

    // compile every expression (surfaces parse errors)
    if let Err(e) = engine.compile() {
        errors.push(ValidationError {
            step_id: None,
            message: e.to_string(),
        });
    }

    // graph validation (missing/dangling steps, sub-rule cycles)
    if let Err(graph_errors) = engine.validate() {
        for msg in graph_errors {
            errors.push(ValidationError {
                step_id: None,
                message: msg,
            });
        }
    }

    RulesetReport {
        ruleset: name.to_string(),
        ok: errors.is_empty(),
        errors,
    }
}

/// Validate a catalog file (`facts.json` / `concepts.json`) with `validator`,
/// wrapped as a `RulesetReport` under `label` — reuses the exact same
/// report/print/JSON shape as a ruleset, since the CLI output already prints
/// `✓/✗ <name>` generically. `None` when the file doesn't exist (consistent
/// with `ordo push`'s "an absent catalog file is skipped, not an error"). A
/// read/parse failure becomes a failed report rather than aborting the whole
/// `ordo validate` run, matching how `validate_one` handles an unreadable
/// ruleset.
fn validate_catalog_file(
    path: &Path,
    label: &str,
    validator: impl Fn(&[serde_json::Value]) -> Vec<String>,
) -> Result<Option<RulesetReport>> {
    let values = match crate::project::read_json_array(path) {
        Ok(None) => return Ok(None),
        Ok(Some(v)) => v,
        Err(e) => {
            return Ok(Some(RulesetReport {
                ruleset: label.to_string(),
                ok: false,
                errors: vec![ValidationError {
                    step_id: None,
                    message: format!("{e:#}"),
                }],
            }))
        }
    };
    let errors = validator(&values)
        .into_iter()
        .map(|message| ValidationError {
            step_id: None,
            message,
        })
        .collect::<Vec<_>>();
    Ok(Some(RulesetReport {
        ruleset: label.to_string(),
        ok: errors.is_empty(),
        errors,
    }))
}
