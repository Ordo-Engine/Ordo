use anyhow::{Context, Result};
use clap::Args;
use ordo_core::prelude::*;

#[derive(Args)]
pub struct ExecArgs {
    /// Rule file (JSON or YAML)
    #[arg(long, value_name = "FILE")]
    rule: String,

    /// Input data as JSON string
    #[arg(long)]
    input: Option<String>,

    /// Input data from file (JSON or YAML)
    #[arg(long, value_name = "FILE")]
    input_file: Option<String>,

    /// External data files (loaded as $data.<stem>)
    #[arg(long = "data", value_name = "FILE")]
    data_files: Vec<String>,

    /// Enable execution trace
    #[arg(long)]
    trace: bool,
}

pub fn run(args: ExecArgs) -> Result<()> {
    let ruleset = load_ruleset(&args.rule)?;
    let mut input = load_input(args.input.as_deref(), args.input_file.as_deref())?;

    // Load external data files and inject as $data
    if !args.data_files.is_empty() {
        inject_external_data(&mut input, &args.data_files)?;
    }

    let executor = RuleExecutor::new();
    let options = if args.trace {
        Some(ExecutionOptions::default().trace(true))
    } else {
        None
    };

    let result = executor
        .execute_with_options(&ruleset, input, options.as_ref())
        .map_err(|e| anyhow::anyhow!("Execution error: {}", e))?;

    // Output result
    let output = serde_json::json!({
        "code": result.code,
        "message": result.message,
        "output": result.output,
        "duration_us": result.duration_us,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);

    // Output trace if enabled
    if let Some(trace) = &result.trace {
        eprintln!("\n--- Execution Trace ---");
        for step in &trace.steps {
            eprintln!(
                "  {} ({}) → {}µs",
                step.step_id, step.step_name, step.duration_us
            );
        }
        eprintln!("  Total: {}µs", result.duration_us);
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

fn load_input(inline: Option<&str>, file: Option<&str>) -> Result<Value> {
    if let Some(json) = inline {
        let val: Value = serde_json::from_str(json).context("Failed to parse --input JSON")?;
        return Ok(val);
    }

    if let Some(path) = file {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read input file: {}", path))?;
        let val: Value = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content).context("Failed to parse YAML input")?
        } else {
            serde_json::from_str(&content).context("Failed to parse JSON input")?
        };
        return Ok(val);
    }

    Ok(Value::object(std::collections::HashMap::new()))
}

fn inject_external_data(input: &mut Value, data_files: &[String]) -> Result<()> {
    let mut data_map = std::collections::HashMap::new();

    for path in data_files {
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Cannot extract filename from: {}", path))?
            .to_string();

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read data file: {}", path))?;
        let val: Value = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content).context("Failed to parse YAML data")?
        } else {
            serde_json::from_str(&content).context("Failed to parse JSON data")?
        };

        data_map.insert(stem, val);
    }

    // Inject data_map into the input object as "$data" field
    match input {
        Value::Object(ref mut obj) => {
            let data_obj = Value::object(data_map);
            obj.insert(std::sync::Arc::from("$data"), data_obj);
        }
        _ => {
            anyhow::bail!(
                "--data requires input to be a JSON object, got {}",
                match input {
                    Value::Array(_) => "array",
                    Value::Null => "null",
                    Value::Bool(_) => "bool",
                    Value::Int(_) => "int",
                    Value::Float(_) => "float",
                    Value::String(_) => "string",
                    Value::Object(_) => unreachable!(),
                }
            );
        }
    }

    Ok(())
}
