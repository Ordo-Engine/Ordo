//! `ordo push` — upload local ruleset drafts to the platform.

use anyhow::Result;
use clap::Args;
use ordo_api_client::ApiError;

use crate::project::Project;

#[derive(Args)]
pub struct PushArgs {
    /// Ruleset name to push (default: every ruleset in the project)
    name: Option<String>,
}

#[derive(serde::Serialize)]
struct PushResult {
    ruleset: String,
    status: String,
}

pub async fn run(args: PushArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let linked = crate::api::linked(&project)?;
    let names = match &args.name {
        Some(n) => vec![crate::project::ruleset_name(n)],
        None => project.ruleset_names()?,
    };

    let mut results = Vec::new();
    let mut had_error = false;
    for name in &names {
        let studio = project.load_studio(name)?;
        let body = serde_json::to_value(&studio)?;

        // Current server seq (0 if the ruleset doesn't exist yet).
        let seq = match linked
            .client
            .get_ruleset(&linked.org_id, &linked.project_id, name)
            .await
        {
            Ok(r) => r.draft_seq,
            Err(ApiError::Http { status: 404, .. }) => 0,
            Err(e) => return Err(anyhow::anyhow!("{e}")),
        };

        let status = match linked
            .client
            .save_ruleset(&linked.org_id, &linked.project_id, name, body, seq)
            .await
        {
            Ok(_) => "pushed".to_string(),
            Err(ApiError::Conflict(_)) => {
                had_error = true;
                "conflict — server has newer changes; run `ordo pull` and retry".to_string()
            }
            Err(e) => {
                had_error = true;
                format!("error: {e}")
            }
        };
        results.push(PushResult {
            ruleset: name.clone(),
            status,
        });
    }

    if json {
        crate::output::emit_json(&serde_json::json!({ "results": results }))?;
    } else {
        for r in &results {
            println!("{}: {}", r.ruleset, r.status);
        }
    }
    if had_error {
        std::process::exit(1);
    }
    Ok(())
}
