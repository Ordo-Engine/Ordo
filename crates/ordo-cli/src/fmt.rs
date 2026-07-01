//! `ordo fmt` — canonically pretty-print ruleset files (studio format),
//! normalizing any engine-format file to studio on the way.

use anyhow::{Context, Result};
use clap::Args;

use crate::project::Project;

#[derive(Args)]
pub struct FmtArgs {
    /// Ruleset name to format (default: every ruleset in the project)
    name: Option<String>,

    /// Report which files would change without writing them
    #[arg(long)]
    check: bool,
}

pub fn run(args: FmtArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let names = match args.name {
        Some(n) => vec![n],
        None => project.ruleset_names()?,
    };

    let mut changed = Vec::new();
    for name in &names {
        let path = project.ruleset_path(name);
        let before = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let studio = project.load_studio(name)?;
        let after = format!("{}\n", serde_json::to_string_pretty(&studio)?);
        if before != after {
            changed.push(name.clone());
            if !args.check {
                std::fs::write(&path, &after)?;
            }
        }
    }

    if json {
        crate::output::emit_json(&serde_json::json!({ "changed": changed, "check": args.check }))?;
    } else if changed.is_empty() {
        println!("All rulesets already formatted.");
    } else {
        let verb = if args.check {
            "would reformat"
        } else {
            "formatted"
        };
        for name in &changed {
            println!("{verb} {name}");
        }
    }

    if args.check && !changed.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
