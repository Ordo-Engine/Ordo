//! `ordo diff` — compare local ruleset files against the platform's drafts.

use anyhow::Result;
use clap::Args;
use ordo_api_client::ApiError;
use ordo_core::prelude::RuleSet;
use ordo_studio_format::{engine_to_studio, StudioRuleSet};
use serde_json::Value;

use crate::project::Project;

#[derive(Args)]
pub struct DiffArgs {
    /// Ruleset name (default: every ruleset in the project)
    name: Option<String>,
}

pub async fn run(args: DiffArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let linked = crate::api::linked(&project)?;
    let names = match &args.name {
        Some(n) => vec![crate::project::ruleset_name(n)],
        None => project.ruleset_names()?,
    };

    let mut rows = Vec::new();
    for name in &names {
        let local = serde_json::to_value(project.load_studio(name)?)?;
        let server = match linked
            .client
            .get_ruleset(&linked.org_id, &linked.project_id, name)
            .await
        {
            Ok(r) => Some(canonical_studio(r.draft)?),
            Err(ApiError::Http { status: 404, .. }) => None,
            Err(e) => return Err(anyhow::anyhow!("{e}")),
        };
        let status = match &server {
            None => "local-only",
            Some(s) if *s == local => "in-sync",
            Some(_) => "differs",
        };
        rows.push((name.clone(), status));
    }

    if json {
        let out: Vec<_> = rows
            .iter()
            .map(|(n, s)| serde_json::json!({ "ruleset": n, "status": s }))
            .collect();
        crate::output::emit_json(&serde_json::json!({ "rulesets": out }))?;
    } else {
        for (name, status) in &rows {
            println!("{status:<11} {name}");
        }
    }
    Ok(())
}

/// Normalize a draft `Value` (studio or engine format) to canonical studio JSON.
fn canonical_studio(draft: Value) -> Result<Value> {
    if draft.get("steps").map(|s| s.is_object()).unwrap_or(false) {
        let engine: RuleSet = serde_json::from_value(draft)?;
        Ok(serde_json::to_value(engine_to_studio(&engine))?)
    } else {
        let studio: StudioRuleSet = serde_json::from_value(draft)?;
        Ok(serde_json::to_value(studio)?)
    }
}
