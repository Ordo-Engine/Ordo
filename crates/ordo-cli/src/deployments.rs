//! `ordo deployments` — list recent deployments.

use anyhow::Result;
use clap::Args;

use crate::project::Project;

#[derive(Args)]
pub struct DeploymentsArgs {
    /// Ruleset name (default: every ruleset in the project)
    name: Option<String>,
    /// Max deployments to show
    #[arg(long, default_value_t = 20)]
    limit: u32,
}

pub async fn run(args: DeploymentsArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let linked = crate::api::linked(&project)?;

    let deps = match &args.name {
        Some(n) => linked
            .client
            .list_deployments(
                &linked.org_id,
                &linked.project_id,
                &crate::project::ruleset_name(n),
                args.limit,
            )
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        None => linked
            .client
            .list_project_deployments(&linked.org_id, &linked.project_id, args.limit)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?,
    };

    if json {
        let rows: Vec<_> = deps
            .iter()
            .map(|d| {
                serde_json::json!({
                    "id": d.id, "ruleset": d.ruleset_name, "environment": d.environment_name,
                    "version": d.version, "status": d.status, "deployed_at": d.deployed_at,
                })
            })
            .collect();
        crate::output::emit_json(&serde_json::json!({ "deployments": rows }))?;
    } else if deps.is_empty() {
        println!("(no deployments)");
    } else {
        for d in &deps {
            println!(
                "{:<10} {:<24} v{:<10} {}  {}",
                d.status,
                d.ruleset_name,
                d.version,
                d.environment_name.as_deref().unwrap_or("-"),
                d.deployed_at.as_deref().unwrap_or("")
            );
        }
    }
    Ok(())
}
