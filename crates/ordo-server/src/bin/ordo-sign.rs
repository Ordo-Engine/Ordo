use clap::Parser;
use ordo_core::prelude::CompiledRuleSet;
use ordo_core::signature::signer::RuleSigner;
use ordo_core::signature::{strip_signature, SIGNATURE_FIELD};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "ordo-sign", about = "Sign Ordo rule files (JSON/YAML/.ordo)")]
struct Args {
    /// Private key file (base64)
    #[arg(long)]
    key: PathBuf,
    /// Input ruleset file (.json/.yaml/.yml/.ordo)
    #[arg(long)]
    input: PathBuf,
    /// Output file (optional)
    #[arg(long)]
    output: Option<PathBuf>,
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

fn default_output_path(input: &Path, format: &InputFormat) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ruleset");
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    match format {
        InputFormat::Json => parent.join(format!("{stem}.signed.json")),
        InputFormat::Yaml => parent.join(format!("{stem}.signed.yaml")),
        InputFormat::Compiled => parent.join(format!("{stem}.signed.ordo")),
    }
}

fn sign_json_value(signer: &RuleSigner, mut value: JsonValue) -> anyhow::Result<JsonValue> {
    let _existing = strip_signature(&mut value)?;
    let signed_at = Some(chrono::Utc::now().to_rfc3339());
    let signature = signer.sign_json_value(&value, signed_at)?;

    let JsonValue::Object(map) = &mut value else {
        return Err(anyhow::anyhow!("Ruleset JSON must be an object"));
    };
    map.insert(
        SIGNATURE_FIELD.to_string(),
        serde_json::to_value(signature)?,
    );
    Ok(value)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let private_key = fs::read_to_string(&args.key)?;
    let signer = RuleSigner::from_private_key_base64(private_key.trim())?;

    let format = detect_format(&args.input)?;
    let output = args
        .output
        .unwrap_or_else(|| default_output_path(&args.input, &format));

    match format {
        InputFormat::Compiled => {
            let bytes = fs::read(&args.input)?;
            let mut compiled = CompiledRuleSet::deserialize(&bytes)?;
            compiled.sign_with_signer(&signer)?;
            let signed_bytes = compiled.serialize();
            fs::write(&output, signed_bytes)?;
        }
        InputFormat::Json => {
            let content = fs::read_to_string(&args.input)?;
            let value: JsonValue = serde_json::from_str(&content)?;
            let signed_value = sign_json_value(&signer, value)?;
            let out = serde_json::to_string_pretty(&signed_value)?;
            fs::write(&output, out)?;
        }
        InputFormat::Yaml => {
            let content = fs::read_to_string(&args.input)?;
            let value: JsonValue = serde_yaml::from_str(&content)?;
            let signed_value = sign_json_value(&signer, value)?;
            let out = serde_yaml::to_string(&signed_value)?;
            fs::write(&output, out)?;
        }
    }

    println!("Signed file written to {}", output.display());
    Ok(())
}
