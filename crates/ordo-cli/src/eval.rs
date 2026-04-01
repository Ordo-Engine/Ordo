use anyhow::{Context, Result};
use clap::Args;
use ordo_core::prelude::*;
use std::io::{IsTerminal, Read};

#[derive(Args)]
pub struct EvalArgs {
    /// Expression to evaluate
    expression: String,

    /// Input data as JSON string
    #[arg(long)]
    input: Option<String>,

    /// Input data from file (JSON or YAML)
    #[arg(long, value_name = "FILE")]
    input_file: Option<String>,
}

pub fn run(args: EvalArgs) -> Result<()> {
    let input = load_input(args.input.as_deref(), args.input_file.as_deref())?;
    let ctx = ordo_core::context::Context::new(input);

    let expr =
        ExprParser::parse(&args.expression).map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    let evaluator = Evaluator::new();
    let result = evaluator
        .eval(&expr, &ctx)
        .map_err(|e| anyhow::anyhow!("Eval error: {}", e))?;

    println!("{}", format_value(&result));
    Ok(())
}

fn load_input(inline: Option<&str>, file: Option<&str>) -> Result<Value> {
    if let Some(json) = inline {
        let val: Value = serde_json::from_str(json).context("Failed to parse --input JSON")?;
        return Ok(val);
    }

    if let Some(path) = file {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path))?;
        let val: Value = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content).context("Failed to parse YAML")?
        } else {
            serde_json::from_str(&content).context("Failed to parse JSON")?
        };
        return Ok(val);
    }

    // Try reading from stdin
    if std::io::stdin().is_terminal() {
        return Ok(Value::Null);
    }

    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("Failed to read stdin")?;
    if buf.trim().is_empty() {
        return Ok(Value::Null);
    }
    let val: Value = serde_json::from_str(&buf).context("Failed to parse stdin JSON")?;
    Ok(val)
}

fn format_value(val: &Value) -> String {
    match val {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => s.to_string(),
        _ => serde_json::to_string_pretty(val).unwrap_or_else(|_| val.to_string()),
    }
}
