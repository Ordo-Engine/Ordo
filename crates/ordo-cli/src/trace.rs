//! `ordo trace` — execute a project ruleset against an input and show the
//! step-by-step execution path. The AI agent's primary debugging tool.

use anyhow::{Context, Result};
use clap::Args;
use ordo_core::prelude::Value;

use crate::project::Project;
use crate::runtime::{execute_loaded_rule, LoadedRule};

#[derive(Args)]
pub struct TraceArgs {
    /// Ruleset name to trace
    name: String,

    /// Input data as a JSON string
    #[arg(long)]
    input: Option<String>,

    /// Input data from a file (JSON or YAML)
    #[arg(long, value_name = "FILE")]
    input_file: Option<String>,
}

pub fn run(args: TraceArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let name = crate::project::ruleset_name(&args.name);
    let mut engine = project.load_engine(&name)?;
    engine
        .compile()
        .map_err(|e| anyhow::anyhow!("compile error: {e}"))?;
    let input = load_input(args.input.as_deref(), args.input_file.as_deref())?;

    let result = execute_loaded_rule(&LoadedRule::Source(engine), input, true)?;

    if json {
        crate::output::emit_json(&serde_json::json!({
            "code": result.code,
            "message": result.message,
            "output": result.output,
            "duration_us": result.duration_us,
            "trace": result.trace,
        }))?;
        return Ok(());
    }

    println!("code:    {}", result.code);
    if !result.message.is_empty() {
        println!("message: {}", result.message);
    }
    println!("output:  {}", serde_json::to_string_pretty(&result.output)?);
    if let Some(trace) = &result.trace {
        println!("\npath:    {}", trace.path_string());
        println!("steps:");
        for step in &trace.steps {
            let arrow = step
                .next_step
                .as_deref()
                .map(|n| format!(" → {n}"))
                .unwrap_or_else(|| {
                    if step.is_terminal {
                        " (terminal)".into()
                    } else {
                        String::new()
                    }
                });
            println!(
                "  {} ({}){}  {}µs",
                step.step_id, step.step_name, arrow, step.duration_us
            );
        }
    }
    println!("\n({}µs)", result.duration_us);
    Ok(())
}

fn load_input(inline: Option<&str>, file: Option<&str>) -> Result<Value> {
    if let Some(json) = inline {
        return serde_json::from_str(json).context("failed to parse --input JSON");
    }
    if let Some(path) = file {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read input file: {path}"))?;
        return if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content).context("failed to parse YAML input")
        } else {
            serde_json::from_str(&content).context("failed to parse JSON input")
        };
    }
    Ok(Value::object(std::collections::HashMap::new()))
}
