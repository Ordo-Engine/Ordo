//! `ordo mcp` — expose Ordo as a Model Context Protocol server over stdio, so a
//! coding agent (Claude Code / Cursor / …) gets Ordo's tools natively.
//!
//! Minimal JSON-RPC 2.0 over newline-delimited stdio (the MCP stdio transport).
//! Tools operate on the local project files + engine — offline and instant —
//! except `publish`, which reaches the platform. High-risk ops are gated by
//! policy: local edits are allowed (git-backed, reversible); `publish` and
//! deleting a `rulesets/*` file require `--allow-publish` / `--allow-delete`.

use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::project::Project;
use crate::runtime::{execute_loaded_rule, LoadedRule};

const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(clap::Args)]
pub struct McpArgs {
    /// Allow the `publish` tool (deploys a ruleset to an environment)
    #[arg(long)]
    pub allow_publish: bool,
    /// Allow the `delete_file` tool to remove ruleset files
    #[arg(long)]
    pub allow_delete: bool,
}

/// Headless policy for high-risk tools (set from CLI flags).
#[derive(Clone, Copy)]
pub struct Policy {
    pub allow_publish: bool,
    pub allow_delete: bool,
}

#[derive(Deserialize)]
struct Req {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

enum Outcome {
    Result(Value),
    Error(i64, String),
    /// A notification — no reply.
    None,
}

pub async fn run(policy: Policy) -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let req: Req = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let reply = match dispatch(&req, policy).await {
            Outcome::None => None,
            Outcome::Result(result) => {
                Some(json!({ "jsonrpc": "2.0", "id": req.id, "result": result }))
            }
            Outcome::Error(code, message) => Some(
                json!({ "jsonrpc": "2.0", "id": req.id, "error": { "code": code, "message": message } }),
            ),
        };
        if let Some(reply) = reply {
            stdout
                .write_all(serde_json::to_string(&reply)?.as_bytes())
                .await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

async fn dispatch(req: &Req, policy: Policy) -> Outcome {
    match req.method.as_str() {
        "initialize" => Outcome::Result(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "ordo", "version": env!("CARGO_PKG_VERSION") },
        })),
        "ping" => Outcome::Result(json!({})),
        "tools/list" => Outcome::Result(json!({ "tools": tool_specs() })),
        "tools/call" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match call_tool(name, &args, policy).await {
                Ok(text) => Outcome::Result(json!({
                    "content": [{ "type": "text", "text": text }], "isError": false,
                })),
                Err(msg) => Outcome::Result(json!({
                    "content": [{ "type": "text", "text": msg }], "isError": true,
                })),
            }
        }
        // Notifications carry no id and expect no reply.
        m if m.starts_with("notifications/") => Outcome::None,
        _ if req.id.is_none() => Outcome::None,
        other => Outcome::Error(-32601, format!("method not found: {other}")),
    }
}

/// Run blocking engine work off the async reactor (the engine's capability
/// client is `reqwest::blocking`, which cannot live on a tokio worker thread).
async fn run_blocking<F>(f: F) -> std::result::Result<String, String>
where
    F: FnOnce() -> std::result::Result<String, String> + Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(r) => r,
        Err(e) => Err(format!("internal task error: {e}")),
    }
}

fn arg_str<'a>(args: &'a Value, key: &str) -> std::result::Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing required argument: {key}"))
}

/// Execute a tool, returning its text result (Ok) or an error message (Err).
async fn call_tool(
    name: &str,
    args: &Value,
    policy: Policy,
) -> std::result::Result<String, String> {
    let project = || Project::discover(None).map_err(|e| e.to_string());

    match name {
        "list_files" => {
            let p = project()?;
            Ok(json!(p.list_files()).to_string())
        }
        "read_file" => {
            let p = project()?;
            let path = p
                .resolve(arg_str(args, "path")?)
                .map_err(|e| e.to_string())?;
            std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))
        }
        "grep" => {
            let p = project()?;
            let query = arg_str(args, "query")?;
            let mut hits = Vec::new();
            for rel in p.list_files() {
                if let Ok(text) = std::fs::read_to_string(p.root.join(&rel)) {
                    for (i, l) in text.lines().enumerate() {
                        if l.contains(query) {
                            hits.push(format!("{rel}:{}: {}", i + 1, l.trim()));
                        }
                    }
                }
            }
            Ok(if hits.is_empty() {
                "No matches".to_string()
            } else {
                hits.join("\n")
            })
        }
        "write_file" => {
            let p = project()?;
            let rel = arg_str(args, "path")?;
            let content = arg_str(args, "content")?;
            let path = p.resolve(rel).map_err(|e| e.to_string())?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, content).map_err(|e| format!("cannot write {rel}: {e}"))?;
            Ok(format!("wrote {rel}"))
        }
        "delete_file" => {
            let p = project()?;
            let rel = arg_str(args, "path")?;
            let is_ruleset = rel.starts_with("rulesets/");
            if is_ruleset && !policy.allow_delete {
                return Err(format!(
                    "deleting a ruleset ({rel}) is high-risk; restart with --allow-delete to permit it"
                ));
            }
            if !(is_ruleset || rel.starts_with("tests/") || rel.starts_with("contracts/")) {
                return Err(format!(
                    "refusing to delete {rel} (only rulesets/tests/contracts)"
                ));
            }
            let path = p.resolve(rel).map_err(|e| e.to_string())?;
            std::fs::remove_file(&path).map_err(|e| format!("cannot delete {rel}: {e}"))?;
            Ok(format!("deleted {rel}"))
        }
        "validate" => {
            let p = project()?;
            let concepts = p.load_concepts().map_err(|e| e.to_string())?;
            let names = match args.get("path").and_then(|v| v.as_str()) {
                Some(path) => vec![crate::project::ruleset_name(path)],
                None => p.ruleset_names().map_err(|e| e.to_string())?,
            };
            let reports: Vec<_> = names
                .iter()
                .map(|n| crate::validate::validate_one(&p, n, &concepts))
                .collect();
            Ok(json!(reports).to_string())
        }
        "run_tests" => {
            // Executes the engine (blocking capability client) — run off the reactor.
            let name = crate::project::ruleset_name(arg_str(args, "ruleset")?);
            run_blocking(move || {
                let p = Project::discover(None).map_err(|e| e.to_string())?;
                crate::test_runner::run_project_ruleset(&p, &name)
                    .map(|s| s.to_string())
                    .map_err(|e| e.to_string())
            })
            .await
        }
        "trace" => {
            let name = crate::project::ruleset_name(arg_str(args, "ruleset")?);
            let input_val = args.get("input").cloned().unwrap_or_else(|| json!({}));
            run_blocking(move || {
                let p = Project::discover(None).map_err(|e| e.to_string())?;
                let input: ordo_core::prelude::Value =
                    serde_json::from_value(input_val).map_err(|e| format!("invalid input: {e}"))?;
                let mut engine = p.load_engine(&name).map_err(|e| e.to_string())?;
                engine
                    .compile()
                    .map_err(|e| format!("compile error: {e}"))?;
                let result = execute_loaded_rule(&LoadedRule::Source(engine), input, true)
                    .map_err(|e| e.to_string())?;
                Ok(json!({
                    "code": result.code, "message": result.message,
                    "output": result.output, "trace": result.trace,
                })
                .to_string())
            })
            .await
        }
        "publish" => {
            if !policy.allow_publish {
                return Err(
                    "publish is high-risk; restart `ordo mcp` with --allow-publish to permit it"
                        .to_string(),
                );
            }
            let p = project()?;
            let linked = crate::api::linked(&p).map_err(|e| e.to_string())?;
            let name = crate::project::ruleset_name(arg_str(args, "ruleset")?);
            let env_id = arg_str(args, "environmentId")?;
            let note = args.get("releaseNote").and_then(|v| v.as_str());
            let dep = linked
                .client
                .publish(&linked.org_id, &linked.project_id, &name, env_id, note)
                .await
                .map_err(|e| e.to_string())?;
            Ok(
                json!({ "deployment": dep.id, "status": dep.status, "version": dep.version })
                    .to_string(),
            )
        }
        other => Err(format!("unknown tool: {other}")),
    }
}

fn tool_specs() -> Vec<Value> {
    let path_arg = |desc: &str| json!({ "type": "object", "properties": { "path": { "type": "string", "description": desc } }, "required": ["path"] });
    vec![
        json!({ "name": "list_files", "description": "List every file in the project (the file tree).", "inputSchema": { "type": "object" } }),
        json!({ "name": "read_file", "description": "Read a file's full contents.", "inputSchema": path_arg("e.g. 'rulesets/loan-approval.json', 'facts.json'") }),
        json!({ "name": "grep", "description": "Search the project's files for a substring. Returns matching path:line hits.", "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] } }),
        json!({ "name": "write_file", "description": "Create or overwrite a file with the full new contents.", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" }, "content": { "type": "string" } }, "required": ["path", "content"] } }),
        json!({ "name": "delete_file", "description": "Delete a rulesets/tests/contracts file. Deleting a ruleset is high-risk.", "inputSchema": path_arg("the file path to delete") }),
        json!({ "name": "validate", "description": "Compile a ruleset and return structured errors (all rulesets if no path).", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } } } }),
        json!({ "name": "run_tests", "description": "Run a ruleset's test cases and return pass/fail results.", "inputSchema": { "type": "object", "properties": { "ruleset": { "type": "string" } }, "required": ["ruleset"] } }),
        json!({ "name": "trace", "description": "Execute a ruleset against an input and return the step-by-step execution path.", "inputSchema": { "type": "object", "properties": { "ruleset": { "type": "string" }, "input": { "type": "object" } }, "required": ["ruleset"] } }),
        json!({ "name": "publish", "description": "HIGH-RISK: publish a ruleset to an environment (requires --allow-publish).", "inputSchema": { "type": "object", "properties": { "ruleset": { "type": "string" }, "environmentId": { "type": "string" }, "releaseNote": { "type": "string" } }, "required": ["ruleset", "environmentId"] } }),
    ]
}
