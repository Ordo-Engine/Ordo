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

    std::fs::create_dir_all(project.tests_dir())?;
    let mut pulled = Vec::new();
    let mut test_files = 0usize;
    for meta in &metas {
        let rs = linked
            .client
            .get_ruleset(&linked.org_id, &linked.project_id, &meta.name)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        std::fs::write(
            project.ruleset_path(&meta.name),
            format!("{}\n", serde_json::to_string_pretty(&rs.draft)?),
        )?;
        pulled.push(meta.name.clone());

        // Test cases for this ruleset → tests/<name>.json (written when present).
        let tests = linked
            .client
            .list_tests(&linked.project_id, &meta.name)
            .await
            .unwrap_or_default();
        if !tests.is_empty() {
            std::fs::write(
                project.tests_path(&meta.name),
                format!("{}\n", serde_json::to_string_pretty(&tests)?),
            )?;
            test_files += 1;
        }
    }

    // Decision contracts → contracts/<ruleset>.json
    let contracts = linked
        .client
        .list_contracts(&linked.project_id)
        .await
        .unwrap_or_default();
    if !contracts.is_empty() {
        std::fs::create_dir_all(project.root.join("contracts"))?;
        for c in &contracts {
            if let Some(name) = c.get("ruleset_name").and_then(|v| v.as_str()) {
                std::fs::write(
                    project.root.join("contracts").join(format!("{name}.json")),
                    format!("{}\n", serde_json::to_string_pretty(c)?),
                )?;
            }
        }
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
            "test_files": test_files, "contracts": contracts.len(),
        }))?;
    } else {
        println!(
            "Pulled {} ruleset(s), {} fact(s), {} concept(s), {} test file(s), {} contract(s)",
            pulled.len(),
            facts.len(),
            concepts.len(),
            test_files,
            contracts.len()
        );
        for r in &pulled {
            println!("  rulesets/{r}.json");
        }
    }
    Ok(())
}
