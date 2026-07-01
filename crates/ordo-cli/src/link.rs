//! `ordo link` — bind the local project to a platform org + project.

use anyhow::{Context, Result};
use clap::Args;

use crate::project::{Project, CONFIG_FILE};

#[derive(Args)]
pub struct LinkArgs {
    /// Organization name or id
    #[arg(long)]
    org: Option<String>,
    /// Project name or id
    #[arg(long)]
    project: Option<String>,
}

pub async fn run(args: LinkArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let client = crate::api::authed_client(project.config.api_url.as_deref())?;

    let orgs = client
        .list_orgs()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let org = match &args.org {
        Some(q) => orgs
            .iter()
            .find(|o| &o.id == q || &o.name == q)
            .with_context(|| format!("org '{q}' not found"))?,
        None => {
            print_choices("organizations", orgs.iter().map(|o| &o.name));
            anyhow::bail!("specify one with --org <name>");
        }
    };

    let projects = client
        .list_projects(&org.id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let proj = match &args.project {
        Some(q) => projects
            .iter()
            .find(|p| &p.id == q || &p.name == q)
            .with_context(|| format!("project '{q}' not found in org '{}'", org.name))?,
        None => {
            print_choices("projects", projects.iter().map(|p| &p.name));
            anyhow::bail!("specify one with --project <name>");
        }
    };

    let envs = client
        .list_environments(&org.id, &proj.id)
        .await
        .unwrap_or_default();

    let root = project.root.clone();
    let mut cfg = project.config;
    cfg.org_id = Some(org.id.clone());
    cfg.project_id = Some(proj.id.clone());
    cfg.environments = envs
        .iter()
        .map(|e| (e.name.clone(), e.id.clone()))
        .collect();
    std::fs::write(root.join(CONFIG_FILE), serde_yaml::to_string(&cfg)?)?;

    if json {
        crate::output::emit_json(&serde_json::json!({
            "org": org.name, "org_id": org.id,
            "project": proj.name, "project_id": proj.id,
            "environments": cfg.environments,
        }))?;
    } else {
        println!("Linked to {} / {}", org.name, proj.name);
        if !cfg.environments.is_empty() {
            println!(
                "  environments: {}",
                cfg.environments
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    Ok(())
}

fn print_choices<'a>(label: &str, items: impl Iterator<Item = &'a String>) {
    eprintln!("available {label}:");
    for it in items {
        eprintln!("  {it}");
    }
}
