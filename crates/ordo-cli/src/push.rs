//! `ordo push` — upload local rulesets + catalog + tests to the platform.

use anyhow::Result;
use clap::Args;
use ordo_api_client::{ApiError, Client};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::project::Project;

#[derive(Args)]
pub struct PushArgs {
    /// Ruleset name to push (default: every ruleset in the project)
    name: Option<String>,
    /// Only push rulesets — skip facts / concepts / tests / contracts
    #[arg(long)]
    rulesets_only: bool,
}

pub async fn run(args: PushArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let linked = crate::api::linked(&project)?;
    let client = &linked.client;
    let (org, proj) = (&linked.org_id, &linked.project_id);
    let names = match &args.name {
        Some(n) => vec![crate::project::ruleset_name(n)],
        None => project.ruleset_names()?,
    };

    let mut results: Vec<(String, String)> = Vec::new();
    let mut had_error = false;

    // 1. Rulesets (optimistic-lock; 0 seq for a new one).
    for name in &names {
        let studio = project.load_studio(name)?;
        let body = serde_json::to_value(&studio)?;
        let seq = match client.get_ruleset(org, proj, name).await {
            Ok(r) => r.draft_seq,
            Err(ApiError::Http { status: 404, .. }) => 0,
            Err(e) => return Err(anyhow::anyhow!("{e}")),
        };
        let status = match client.save_ruleset(org, proj, name, body, seq).await {
            Ok(_) => "pushed".to_string(),
            Err(ApiError::Conflict(_)) => {
                had_error = true;
                "conflict — run `ordo pull` and retry".to_string()
            }
            Err(e) => {
                had_error = true;
                format!("error: {e}")
            }
        };
        results.push((format!("rulesets/{name}"), status));
    }

    if !args.rulesets_only {
        // 2. Facts + concepts (whole-catalog sync: upsert local, delete server-only).
        if let Some(facts) = read_array(&project.facts_path())? {
            let (up, del) = sync_facts(client, proj, &facts).await.map_err(anyerr)?;
            results.push(("facts.json".into(), format!("{up} upserted, {del} removed")));
        }
        if let Some(concepts) = read_array(&project.concepts_path())? {
            let (up, del) = sync_concepts(client, proj, &concepts)
                .await
                .map_err(anyerr)?;
            results.push((
                "concepts.json".into(),
                format!("{up} upserted, {del} removed"),
            ));
        }

        // 3. Tests (per ruleset, keyed by name).
        for name in &names {
            let path = project.tests_path(name);
            if let Some(tests) = read_array(&path)? {
                sync_tests(client, proj, name, &tests)
                    .await
                    .map_err(anyerr)?;
                results.push((format!("tests/{name}"), format!("{} synced", tests.len())));
            }
        }

        // 4. Contracts (upsert each local contract).
        for name in &names {
            let path = project.root.join("contracts").join(format!("{name}.json"));
            if path.is_file() {
                let contract: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
                client
                    .upsert_contract(proj, name, contract)
                    .await
                    .map_err(anyerr)?;
                results.push((format!("contracts/{name}"), "pushed".into()));
            }
        }
    }

    if json {
        let rows: Vec<_> = results
            .iter()
            .map(|(f, s)| serde_json::json!({ "path": f, "status": s }))
            .collect();
        crate::output::emit_json(&serde_json::json!({ "results": rows }))?;
    } else {
        for (f, s) in &results {
            println!("{f}: {s}");
        }
    }
    if had_error {
        std::process::exit(1);
    }
    Ok(())
}

fn anyerr(e: ApiError) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn read_array(path: &std::path::Path) -> Result<Option<Vec<Value>>> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)?;
    if text.trim().is_empty() {
        return Ok(Some(Vec::new()));
    }
    Ok(Some(serde_json::from_str(&text)?))
}

fn names_of(items: &[Value]) -> HashSet<String> {
    items
        .iter()
        .filter_map(|it| it.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect()
}

async fn sync_facts(
    client: &Client,
    proj: &str,
    local: &[Value],
) -> ordo_api_client::Result<(usize, usize)> {
    let server = client.list_facts(proj).await?;
    let local_names = names_of(local);
    for f in local {
        client.upsert_fact(proj, f.clone()).await?;
    }
    let mut deleted = 0;
    for s in &server {
        if let Some(n) = s.get("name").and_then(|v| v.as_str()) {
            if !local_names.contains(n) {
                client.delete_fact(proj, n).await?;
                deleted += 1;
            }
        }
    }
    Ok((local.len(), deleted))
}

async fn sync_concepts(
    client: &Client,
    proj: &str,
    local: &[Value],
) -> ordo_api_client::Result<(usize, usize)> {
    let server = client.list_concepts(proj).await?;
    let local_names = names_of(local);
    for c in local {
        client.upsert_concept(proj, c.clone()).await?;
    }
    let mut deleted = 0;
    for s in &server {
        if let Some(n) = s.get("name").and_then(|v| v.as_str()) {
            if !local_names.contains(n) {
                client.delete_concept(proj, n).await?;
                deleted += 1;
            }
        }
    }
    Ok((local.len(), deleted))
}

async fn sync_tests(
    client: &Client,
    proj: &str,
    ruleset: &str,
    local: &[Value],
) -> ordo_api_client::Result<()> {
    let server = client.list_tests(proj, ruleset).await?;
    let server_by_name: HashMap<String, String> = server
        .iter()
        .filter_map(|t| {
            Some((
                t.get("name")?.as_str()?.to_string(),
                t.get("id")?.as_str()?.to_string(),
            ))
        })
        .collect();
    let local_names = names_of(local);

    for t in local {
        let name = t.get("name").and_then(|v| v.as_str()).unwrap_or_default();
        match server_by_name.get(name) {
            Some(id) => client.update_test(proj, ruleset, id, t.clone()).await?,
            None => {
                client.create_test(proj, ruleset, t.clone()).await?;
            }
        }
    }
    for (name, id) in &server_by_name {
        if !local_names.contains(name) {
            client.delete_test(proj, ruleset, id).await?;
        }
    }
    Ok(())
}
