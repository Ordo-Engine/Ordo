//! `ordo init` — scaffold a new decision project (rules-as-files).

use anyhow::{Context, Result};
use clap::Args;
use ordo_core::prelude::RuleSet;
use ordo_studio_format::engine_to_studio;
use std::path::Path;

use crate::project::{ProjectConfig, CONFIG_FILE};

#[derive(Args)]
pub struct InitArgs {
    /// Directory to initialize (default: current directory)
    #[arg(default_value = ".")]
    dir: String,

    /// Project name (default: the directory name)
    #[arg(long)]
    name: Option<String>,
}

/// A minimal, valid example ruleset in engine format (converted to studio on
/// write). Approves when `amount` is within a limit.
const EXAMPLE_ENGINE: &str = r#"{
  "config": { "name": "loan-approval", "version": "1.0.0", "entry_step": "check_amount" },
  "steps": {
    "check_amount": {
      "id": "check_amount", "name": "Check amount", "type": "decision",
      "branches": [ { "condition": "amount <= 10000", "next_step": "approve" } ],
      "default_next": "reject"
    },
    "approve": {
      "id": "approve", "name": "Approve", "type": "terminal",
      "result": { "code": "APPROVED", "message": "Within limit",
        "output": [ ["approved", {"Literal": true}], ["amount", {"Field": "amount"}] ] }
    },
    "reject": {
      "id": "reject", "name": "Reject", "type": "terminal",
      "result": { "code": "REJECTED", "message": "Amount exceeds limit",
        "output": [ ["approved", {"Literal": false}] ] }
    }
  }
}"#;

const EXAMPLE_TESTS: &str = r#"[
  { "name": "within limit", "input": { "amount": 5000 }, "expect": { "code": "APPROVED" } },
  { "name": "over limit", "input": { "amount": 20000 }, "expect": { "code": "REJECTED" } }
]
"#;

pub fn run(args: InitArgs, json: bool) -> Result<()> {
    let root = Path::new(&args.dir);
    std::fs::create_dir_all(root)
        .with_context(|| format!("failed to create {}", root.display()))?;
    let config_path = root.join(CONFIG_FILE);
    if config_path.exists() {
        anyhow::bail!(
            "{} already exists — this is already an Ordo project",
            config_path.display()
        );
    }

    let name = args.name.unwrap_or_else(|| {
        root.canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            .filter(|s| !s.is_empty() && s != ".")
            .unwrap_or_else(|| "ordo-project".to_string())
    });

    std::fs::create_dir_all(root.join("rulesets"))?;
    std::fs::create_dir_all(root.join("tests"))?;

    // ordo.yaml
    let config = ProjectConfig {
        project: name.clone(),
        org_id: None,
        project_id: None,
        api_url: None,
        environments: Default::default(),
    };
    std::fs::write(&config_path, serde_yaml::to_string(&config)?)?;

    // example ruleset (engine → studio) + tests
    let engine: RuleSet =
        serde_json::from_str(EXAMPLE_ENGINE).context("built-in example ruleset is invalid")?;
    let studio = engine_to_studio(&engine);
    std::fs::write(
        root.join("rulesets/loan-approval.json"),
        format!("{}\n", serde_json::to_string_pretty(&studio)?),
    )?;
    std::fs::write(root.join("tests/loan-approval.json"), EXAMPLE_TESTS)?;

    // empty catalogs + agent guidance
    std::fs::write(root.join("facts.json"), "[]\n")?;
    std::fs::write(root.join("concepts.json"), "[]\n")?;
    std::fs::write(root.join("AGENTS.md"), AGENTS_MD)?;

    let created = [
        CONFIG_FILE,
        "rulesets/loan-approval.json",
        "tests/loan-approval.json",
        "facts.json",
        "concepts.json",
        "AGENTS.md",
    ];
    if json {
        crate::output::emit_json(&serde_json::json!({
            "project": name,
            "root": root.display().to_string(),
            "created": created,
        }))?;
    } else {
        println!("Initialized Ordo project '{name}' in {}", root.display());
        for f in created {
            println!("  {f}");
        }
        println!("\nNext: `ordo validate` · `ordo test` · `ordo trace loan-approval --input '{{\"amount\":5000}}'`");
    }
    Ok(())
}

const AGENTS_MD: &str = r#"# Ordo decision project

This folder is an Ordo decision project — business rules stored as files and
executed by the Ordo engine. Edit the files, then use the `ordo` CLI to check
your work (everything below runs offline, sub-second).

## Layout
- `rulesets/<name>.json` — a ruleset in Ordo studio format: `config` (name/version),
  `startStepId`, and `steps[]`. Each step is `decision` (ordered `branches`, first
  match wins, plus a default), `action` (assigns variables/outputs), or `terminal`
  (final `code` + `outputs`).
- `facts.json` — fact definitions (external inputs): `{ name, data_type, source, null_policy }`.
- `concepts.json` — derived named expressions: `{ name, data_type, expression, dependencies[] }`.
- `tests/<name>.json` — an array of `{ name, input, expect: { code?, output? } }` for that ruleset.
- `contracts/<name>.json` — the decision contract (input/output fields).
- `ordo.yaml` — project + link config.

## Workflow
1. Read the ruleset and the `facts.json` / `concepts.json` it references. Don't
   invent fact/concept names — grep for them first.
2. Edit `rulesets/<name>.json`.
3. `ordo validate` — compile every condition; fix reported errors.
4. `ordo test` — run the ruleset's test cases.
5. `ordo trace <name> --input '{...}'` — inspect the exact path an input takes.

Add `--json` to any command for machine-readable output.
"#;
