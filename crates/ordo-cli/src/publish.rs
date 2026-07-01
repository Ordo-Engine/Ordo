//! `ordo publish` — deploy a ruleset to an environment.

use anyhow::{Context, Result};
use clap::Args;

use crate::project::Project;

#[derive(Args)]
pub struct PublishArgs {
    /// Ruleset name to publish
    name: String,
    /// Target environment name (or id)
    #[arg(long)]
    env: String,
    /// Release note
    #[arg(long)]
    note: Option<String>,
}

pub async fn run(args: PublishArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let linked = crate::api::linked(&project)?;
    let name = crate::project::ruleset_name(&args.name);

    // Resolve env name → id: the cache in ordo.yaml first, else fetch live.
    let env_id = match project.config.environments.get(&args.env) {
        Some(id) => id.clone(),
        None => {
            let envs = linked
                .client
                .list_environments(&linked.org_id, &linked.project_id)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            envs.iter()
                .find(|e| e.name == args.env || e.id == args.env)
                .map(|e| e.id.clone())
                .with_context(|| {
                    format!(
                        "environment '{}' not found (available: {})",
                        args.env,
                        envs.iter()
                            .map(|e| e.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?
        }
    };

    let dep = linked
        .client
        .publish(
            &linked.org_id,
            &linked.project_id,
            &name,
            &env_id,
            args.note.as_deref(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if json {
        crate::output::emit_json(&serde_json::json!({
            "deployment": dep.id, "ruleset": dep.ruleset_name,
            "version": dep.version, "status": dep.status,
        }))?;
    } else {
        println!(
            "Published {} v{} to {} → {} (deployment {})",
            dep.ruleset_name, dep.version, args.env, dep.status, dep.id
        );
        if dep.status == "dispatched" {
            println!("  (dispatched — run `ordo deployments {name}` to watch it settle)");
        }
    }
    Ok(())
}
