//! `ordo pull` — fetch rulesets + catalog from the platform into local files.

use anyhow::Result;
use clap::Args;

use crate::project::Project;

#[derive(Args)]
pub struct PullArgs {}

pub async fn run(_args: PullArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let linked = crate::api::linked(&project)?;

    std::fs::create_dir_all(project.rulesets_dir())?;
    let metas = linked
        .client
        .list_rulesets(&linked.org_id, &linked.project_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut pulled = Vec::new();
    for meta in &metas {
        let rs = linked
            .client
            .get_ruleset(&linked.org_id, &linked.project_id, &meta.name)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let path = project.ruleset_path(&meta.name);
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&rs.draft)?),
        )?;
        pulled.push(meta.name.clone());
    }

    // catalog — written verbatim as facts.json / concepts.json
    let facts = linked
        .client
        .list_facts(&linked.project_id)
        .await
        .unwrap_or_default();
    std::fs::write(
        project.facts_path(),
        format!("{}\n", serde_json::to_string_pretty(&facts)?),
    )?;
    let concepts = linked
        .client
        .list_concepts(&linked.project_id)
        .await
        .unwrap_or_default();
    std::fs::write(
        project.concepts_path(),
        format!("{}\n", serde_json::to_string_pretty(&concepts)?),
    )?;

    if json {
        crate::output::emit_json(&serde_json::json!({
            "rulesets": pulled, "facts": facts.len(), "concepts": concepts.len(),
        }))?;
    } else {
        println!(
            "Pulled {} ruleset(s), {} fact(s), {} concept(s)",
            pulled.len(),
            facts.len(),
            concepts.len()
        );
        for r in &pulled {
            println!("  rulesets/{r}.json");
        }
    }
    Ok(())
}
