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
    let names = match args.name {
        Some(n) => vec![crate::project::ruleset_name(&n)],
        None => project.ruleset_names()?,
    };
    let concepts = project.load_concepts()?;

    let mut reports = Vec::with_capacity(names.len());
    for name in &names {
        reports.push(validate_one(&project, name, &concepts));
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
                    message: e.to_string(),
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
