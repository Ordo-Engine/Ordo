use clap::Parser;
use ordo_core::prelude::CompiledRuleSet;
use ordo_core::signature::ed25519::decode_public_key;
use ordo_core::signature::strip_signature;
use ordo_core::signature::verifier::RuleVerifier;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "ordo-verify",
    about = "Verify Ordo rule signatures (JSON/YAML/.ordo)"
)]
struct Args {
    /// Public key file (base64)
    #[arg(long)]
    key: PathBuf,
    /// Input ruleset file (.json/.yaml/.yml/.ordo)
    #[arg(long)]
    input: PathBuf,
}

enum InputFormat {
    Json,
    Yaml,
    Compiled,
}

fn detect_format(path: &Path) -> anyhow::Result<InputFormat> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "json" => Ok(InputFormat::Json),
        "yaml" | "yml" => Ok(InputFormat::Yaml),
        "ordo" => Ok(InputFormat::Compiled),
        _ => Err(anyhow::anyhow!("Unsupported input file type: {}", ext)),
    }
}

fn verify_json_value(verifier: &RuleVerifier, mut value: JsonValue) -> anyhow::Result<()> {
    let signature = strip_signature(&mut value)?;
    let Some(signature) = signature else {
        return Err(anyhow::anyhow!("Missing _signature field"));
    };
    verifier.verify_json_value(&value, Some(&signature))?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let public_key = fs::read_to_string(&args.key)?;
    let verifying_key = decode_public_key(public_key.trim())?;
    let verifier = RuleVerifier::new(vec![verifying_key], true);

    let format = detect_format(&args.input)?;
    match format {
        InputFormat::Compiled => {
            CompiledRuleSet::load_from_file_with_verifier(&args.input, &verifier)?;
        }
        InputFormat::Json => {
            let content = fs::read_to_string(&args.input)?;
            let value: JsonValue = serde_json::from_str(&content)?;
            verify_json_value(&verifier, value)?;
        }
        InputFormat::Yaml => {
            let content = fs::read_to_string(&args.input)?;
            let value: JsonValue = serde_yaml::from_str(&content)?;
            verify_json_value(&verifier, value)?;
        }
    }

    println!("Signature verification succeeded.");
    Ok(())
}
