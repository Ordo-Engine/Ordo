//! `ordo new <ruleset|fact|concept> <name>` — add a file/entry to the project.

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use ordo_studio_format::StudioRuleSet;

use crate::project::Project;

#[derive(Args)]
pub struct NewArgs {
    #[command(subcommand)]
    kind: NewKind,
}

#[derive(Subcommand)]
enum NewKind {
    /// Create a new ruleset (+ an empty test file)
    Ruleset { name: String },
    /// Add a fact definition to facts.json
    Fact { name: String },
    /// Add a concept definition to concepts.json
    Concept { name: String },
}

// Studio-format skeleton. The branch condition is a bare expression string — the
// concise, hand-authorable form (the whole boolean expression, incl. `&&`/`||`/`!`,
// goes in one string that ordo-core parses). The structured object form also works.
const SKELETON_STUDIO: &str = r#"{
  "config": { "name": "NAME", "version": "1.0.0" },
  "startStepId": "check",
  "steps": [
    {
      "id": "check", "name": "Check", "type": "decision",
      "branches": [
        { "id": "check-b0", "label": "example rule", "condition": "input > 0", "nextStepId": "matched" }
      ],
      "defaultNextStepId": "unmatched"
    },
    { "id": "matched", "name": "Matched", "type": "terminal", "code": "MATCHED", "message": "", "output": [] },
    { "id": "unmatched", "name": "Unmatched", "type": "terminal", "code": "UNMATCHED", "message": "", "output": [] }
  ],
  "subRules": {}
}"#;

pub fn run(args: NewArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let created: Vec<String> = match args.kind {
        NewKind::Ruleset { name } => new_ruleset(&project, &name)?,
        NewKind::Fact { name } => {
            new_catalog_entry(&project, project.facts_path(), fact_entry(&name), &name)?
        }
        NewKind::Concept { name } => new_catalog_entry(
            &project,
            project.concepts_path(),
            concept_entry(&name),
            &name,
        )?,
    };

    if json {
        crate::output::emit_json(&serde_json::json!({ "created": created }))?;
    } else {
        for f in &created {
            println!("created {f}");
        }
    }
    Ok(())
}

fn new_ruleset(project: &Project, name: &str) -> Result<Vec<String>> {
    let path = project.ruleset_path(name);
    if path.exists() {
        anyhow::bail!("ruleset '{name}' already exists");
    }
    std::fs::create_dir_all(project.rulesets_dir())?;
    std::fs::create_dir_all(project.tests_dir())?;

    // Parse + re-serialize so the written file is guaranteed valid studio format.
    let studio: StudioRuleSet = serde_json::from_str(&SKELETON_STUDIO.replace("NAME", name))
        .context("skeleton ruleset is invalid")?;
    std::fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(&studio)?),
    )?;

    let tests_path = project.tests_path(name);
    if !tests_path.exists() {
        std::fs::write(&tests_path, "[]\n")?;
    }
    Ok(vec![rel(project, &path), rel(project, &tests_path)])
}

fn new_catalog_entry(
    project: &Project,
    path: std::path::PathBuf,
    entry: serde_json::Value,
    name: &str,
) -> Result<Vec<String>> {
    let mut items: Vec<serde_json::Value> = if path.is_file() {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if text.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(&text).with_context(|| format!("invalid {}", path.display()))?
        }
    } else {
        Vec::new()
    };
    if items
        .iter()
        .any(|it| it.get("name").and_then(|n| n.as_str()) == Some(name))
    {
        anyhow::bail!("'{name}' already exists in {}", path.display());
    }
    items.push(entry);
    std::fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(&items)?),
    )?;
    Ok(vec![format!("{} (+{name})", rel(project, &path))])
}

fn fact_entry(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name, "data_type": "string", "source": "input", "null_policy": "default"
    })
}

fn concept_entry(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name, "data_type": "number", "expression": "0", "dependencies": []
    })
}

fn rel(project: &Project, path: &std::path::Path) -> String {
    path.strip_prefix(&project.root)
        .unwrap_or(path)
        .display()
        .to_string()
}
