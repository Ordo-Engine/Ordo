//! `ordo guard init` — scaffold the `.ordo-guard/` policy project and
//! register the hook for one or more coding agents (Claude Code, Codex CLI,
//! Cursor). The policy itself is agent-agnostic; only the hook's wire format
//! and config file differ per agent — see [`super::Agent`].

use anyhow::{Context, Result};
use clap::Args;
use ordo_studio_format::StudioRuleSet;
use std::path::{Path, PathBuf};

use super::settings::{hook_command, register_cursor_hook, register_hook, RegisterOutcome};
use super::Agent;
use crate::project::{ProjectConfig, CONFIG_FILE};

#[derive(Args)]
pub struct GuardInitArgs {
    /// Repo root to guard (default: current directory)
    #[arg(default_value = ".")]
    dir: String,

    /// Register a portable npx command in the git-shared settings file
    /// instead of an absolute binary path in the machine-local one (Claude
    /// Code only distinguishes these; Codex/Cursor always write one
    /// project-local file, but still get the portable command)
    #[arg(long)]
    shared: bool,

    /// Custom hook command to register (overrides the default for every
    /// selected agent, taken verbatim)
    #[arg(long, value_name = "CMD")]
    command: Option<String>,

    /// Scaffold the policy project only; skip hook registration
    #[arg(long)]
    no_hook: bool,

    /// Coding agent(s) to register the hook for. Repeat the flag or use a
    /// comma-separated list (`--agent codex,cursor`) to register more than
    /// one — each gets its own hook command and config file, all evaluating
    /// the same policy.
    #[arg(long = "agent", value_enum, value_delimiter = ',', default_values_t = vec![Agent::Claude])]
    agents: Vec<Agent>,
}

/// The default policy — deliberately opinionated but small, so the first
/// `ordo guard test` run is green and every rule reads as an example to copy.
const POLICY_JSON: &str = r#"{
  "config": {
    "name": "policy",
    "version": "1.0.0",
    "description": "Coding-agent tool-call policy. Evaluated by `ordo guard hook` on every pre-tool-call event. First matching branch wins; PASS defers to the agent's normal permission flow."
  },
  "startStepId": "gate",
  "steps": [
    {
      "id": "gate", "name": "Policy gate", "type": "decision",
      "branches": [
        { "id": "gate-b0", "label": "block destructive shell commands",
          "condition": "tool == 'Bash' && (command contains 'rm -rf' || command contains 'rm -fr' || command contains 'sudo rm' || regex_match('dd\\s+if=|mkfs', command))",
          "nextStepId": "deny_destructive" },
        { "id": "gate-b1", "label": "protect secrets and keys",
          "condition": "tool in ['Read', 'Write', 'Edit'] && (file_path contains '.env' || ends_with(file_path, '.pem') || file_path contains 'id_rsa' || file_path contains '.aws/credentials')",
          "nextStepId": "deny_secrets" },
        { "id": "gate-b2", "label": "guard the guardrails",
          "condition": "tool in ['Write', 'Edit'] && file_path contains '.ordo-guard'",
          "nextStepId": "ask_self_edit" },
        { "id": "gate-b3", "label": "confirm irreversible publishes",
          "condition": "tool == 'Bash' && (command contains 'git push' || command contains 'npm publish' || command contains 'cargo publish')",
          "nextStepId": "ask_push" },
        { "id": "gate-b4", "label": "fast-path read-only git",
          "condition": "tool == 'Bash' && (command == 'git status' || starts_with(command, 'git diff') || starts_with(command, 'git log'))",
          "nextStepId": "allow_readonly_git" }
      ],
      "defaultNextStepId": "pass"
    },
    { "id": "deny_destructive", "name": "Deny destructive command", "type": "terminal",
      "code": "DENY", "message": "Destructive shell command blocked by policy", "output": [] },
    { "id": "deny_secrets", "name": "Deny secret access", "type": "terminal",
      "code": "DENY", "message": "Access to secrets/credentials is blocked by policy", "output": [] },
    { "id": "ask_self_edit", "name": "Ask on guardrail edits", "type": "terminal",
      "code": "ASK", "message": "The agent is modifying its own guardrails — confirm", "output": [] },
    { "id": "ask_push", "name": "Ask before publishing", "type": "terminal",
      "code": "ASK", "message": "Irreversible publish — confirm before running", "output": [] },
    { "id": "allow_readonly_git", "name": "Allow read-only git", "type": "terminal",
      "code": "ALLOW", "message": "Read-only git command", "output": [] },
    { "id": "pass", "name": "No opinion", "type": "terminal",
      "code": "PASS", "message": "No policy rule matched", "output": [] }
  ],
  "subRules": {}
}"#;

const POLICY_TESTS: &str = r#"[
  { "name": "blocks rm -rf",             "input": { "tool": "Bash", "command": "rm -rf /tmp/x" },        "expect": { "code": "DENY" } },
  { "name": "blocks reading .env",       "input": { "tool": "Read", "file_path": "apps/web/.env" },      "expect": { "code": "DENY" } },
  { "name": "asks on guardrail edits",   "input": { "tool": "Edit", "file_path": ".ordo-guard/rulesets/policy.json" }, "expect": { "code": "ASK" } },
  { "name": "asks before git push",      "input": { "tool": "Bash", "command": "git push origin main" }, "expect": { "code": "ASK" } },
  { "name": "allows read-only git",      "input": { "tool": "Bash", "command": "git status" },           "expect": { "code": "ALLOW" } },
  { "name": "no opinion on normal edits","input": { "tool": "Edit", "file_path": "src/main.rs" },        "expect": { "code": "PASS" } },
  { "name": "missing fields are safe",   "input": { "tool": "Glob" },                                    "expect": { "code": "PASS" } }
]
"#;

/// The event fields the hook feeds the policy, pre-registered as input facts.
const POLICY_FACTS: &str = r#"[
  { "name": "tool", "data_type": "string", "source": "input", "null_policy": "default" },
  { "name": "command", "data_type": "string", "source": "input", "null_policy": "default" },
  { "name": "file_path", "data_type": "string", "source": "input", "null_policy": "default" },
  { "name": "url", "data_type": "string", "source": "input", "null_policy": "default" },
  { "name": "cwd", "data_type": "string", "source": "input", "null_policy": "default" },
  { "name": "permission_mode", "data_type": "string", "source": "input", "null_policy": "default" }
]
"#;

/// One agent's hook registration, kept around after `settings.rs` writes the
/// config file so `run()` can render both the human-readable and JSON output.
struct Registration {
    agent: Agent,
    settings_path: PathBuf,
    command: String,
    outcome: RegisterOutcome,
}

/// Where each agent's hook config lives. Claude Code alone distinguishes a
/// shared (committed) vs. local (gitignored) file; Codex CLI and Cursor each
/// have a single project-local config per their docs, regardless of
/// `--shared` (which still controls the *command* — npx vs. absolute path).
fn settings_path_for(root: &Path, agent: Agent, shared: bool) -> PathBuf {
    match agent {
        Agent::Claude => root.join(".claude").join(if shared {
            "settings.json"
        } else {
            "settings.local.json"
        }),
        Agent::Codex => root.join(".codex").join("hooks.json"),
        Agent::Cursor => root.join(".cursor").join("hooks.json"),
    }
}

fn outcome_str(o: &RegisterOutcome) -> &'static str {
    match o {
        RegisterOutcome::Created => "created",
        RegisterOutcome::Updated => "updated",
        RegisterOutcome::Unchanged => "unchanged",
    }
}

pub fn run(args: GuardInitArgs, json: bool) -> Result<()> {
    let root = Path::new(&args.dir);
    std::fs::create_dir_all(root)
        .with_context(|| format!("failed to create {}", root.display()))?;
    let guard_dir = root.join(super::POLICY_DIR_NAME);

    let scaffolded = if guard_dir.join(CONFIG_FILE).is_file() {
        false
    } else {
        scaffold(&guard_dir)?;
        true
    };

    // De-dupe in case an agent was named more than once (`--agent claude
    // --agent claude`), preserving first-seen order.
    let mut agents = Vec::new();
    for a in &args.agents {
        if !agents.contains(a) {
            agents.push(*a);
        }
    }

    let registrations = if args.no_hook {
        Vec::new()
    } else {
        let mut regs = Vec::with_capacity(agents.len());
        for agent in agents {
            let settings_path = settings_path_for(root, agent, args.shared);
            let command = hook_command(args.shared, args.command.clone(), agent)?;
            let outcome = match agent {
                Agent::Claude | Agent::Codex => register_hook(&settings_path, &command)?,
                Agent::Cursor => register_cursor_hook(&settings_path, &command)?,
            };
            regs.push(Registration {
                agent,
                settings_path,
                command,
                outcome,
            });
        }
        regs
    };

    if json {
        // "hook" mirrors the pre-multi-agent single-object contract (Claude's
        // registration specifically) so existing callers that only ever dealt
        // with Claude Code keep working unchanged; "hooks" is the full list.
        let claude_hook = registrations.iter().find(|r| r.agent == Agent::Claude);
        let hook_json = claude_hook.map(|r| {
            serde_json::json!({
                "settings": r.settings_path.display().to_string(),
                "command": r.command,
                "outcome": outcome_str(&r.outcome),
            })
        });
        let hooks_json: Vec<_> = registrations
            .iter()
            .map(|r| {
                serde_json::json!({
                    "agent": r.agent.key(),
                    "settings": r.settings_path.display().to_string(),
                    "command": r.command,
                    "outcome": outcome_str(&r.outcome),
                })
            })
            .collect();
        crate::output::emit_json(&serde_json::json!({
            "policy_dir": guard_dir.display().to_string(),
            "scaffolded": scaffolded,
            "hook": hook_json,
            "hooks": hooks_json,
        }))?;
    } else {
        if scaffolded {
            println!("Scaffolded guard policy in {}", guard_dir.display());
            for f in [
                "ordo.yaml",
                "rulesets/policy.json",
                "tests/policy.json",
                "facts.json",
                "concepts.json",
                "AGENTS.md",
                ".gitignore",
            ] {
                println!("  {f}");
            }
        } else {
            println!("Guard policy already exists in {}", guard_dir.display());
        }
        if registrations.is_empty() {
            println!("Skipped hook registration (--no-hook)");
        } else {
            for r in &registrations {
                let verb = match r.outcome {
                    RegisterOutcome::Created => "Registered",
                    RegisterOutcome::Updated => "Updated",
                    RegisterOutcome::Unchanged => "Already registered:",
                };
                println!(
                    "{verb} {} hook for {} in {}",
                    r.agent.hook_event_name(),
                    r.agent.label(),
                    r.settings_path.display()
                );
                println!("  {}", r.command);
            }
        }
        println!(
            "\nNext: `ordo guard test` · edit .ordo-guard/rulesets/policy.json · `ordo guard log`"
        );
        for r in &registrations {
            println!("{}", r.agent.restart_hint());
        }
    }
    Ok(())
}

fn scaffold(guard_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(guard_dir.join("rulesets"))?;
    std::fs::create_dir_all(guard_dir.join("tests"))?;

    let config = ProjectConfig {
        project: "guard".to_string(),
        org_id: None,
        project_id: None,
        api_url: None,
        environments: Default::default(),
    };
    std::fs::write(guard_dir.join(CONFIG_FILE), serde_yaml::to_string(&config)?)?;

    // Parse + re-serialize so the written policy is valid studio format by
    // construction (same pattern as `ordo new ruleset`).
    let studio: StudioRuleSet =
        serde_json::from_str(POLICY_JSON).context("built-in guard policy is invalid")?;
    std::fs::write(
        guard_dir.join("rulesets/policy.json"),
        format!("{}\n", serde_json::to_string_pretty(&studio)?),
    )?;
    std::fs::write(guard_dir.join("tests/policy.json"), POLICY_TESTS)?;
    std::fs::write(guard_dir.join("facts.json"), POLICY_FACTS)?;
    std::fs::write(guard_dir.join("concepts.json"), "[]\n")?;
    std::fs::write(guard_dir.join(".gitignore"), "log.jsonl\n")?;
    std::fs::write(guard_dir.join("AGENTS.md"), GUARD_AGENTS_MD)?;
    Ok(())
}

const GUARD_AGENTS_MD: &str = r#"# Ordo guard policy

This folder is the tool-call policy for AI coding agents working in the parent
repo. `ordo guard hook` evaluates `rulesets/policy.json` on every pre-tool-call
event and answers allow / deny / ask; any other terminal code (conventionally
`PASS`) means "no opinion" and the agent's normal permission flow applies.
Decisions are appended to `log.jsonl`.

One policy, enforced identically across whichever agents are hooked up —
`ordo guard init --agent <claude|codex|cursor>` (repeatable/comma-separated;
default `claude`). Claude Code and Codex CLI speak the same PreToolUse
protocol; Cursor's `beforeShellExecution` hook only sees shell commands (no
file-edit visibility), so `file_path`-based rules simply never fire for it.

## Input the policy sees
Flattened from the hook event — reference these directly in conditions:
- `tool` — the tool name (`Bash`, `Read`, `Write`, `Edit`, …; always `Bash` for
  Cursor, since it only hooks shell execution)
- hoisted tool inputs: `command` (Bash), `file_path` (Read/Write/Edit), `url`, …
- `cwd`, `permission_mode`, `session_id`; the full `tool_input` object is nested.

Missing fields are *lenient*: a condition referencing an absent field is false,
so a `command`-based rule is safely skipped for non-Bash tools. Careful with
negations — `!(command contains 'x')` is also false when `command` is absent.

## Writing rules
Branch conditions are bare expression strings, first match wins:
- `"tool == 'Bash' && command contains 'terraform destroy'"`
- `"tool in ['Write', 'Edit'] && file_path contains 'migrations/'"`
- `regex_match(pattern, s)` — the **pattern comes first**.
Terminal codes: `DENY` / `ASK` / `ALLOW` / `PASS`. The terminal `message` (or a
`reason` output field) is shown to the agent as the decision reason.

## Workflow
1. Edit `rulesets/policy.json` — add a branch + a terminal (or reuse one).
2. Add a case to `tests/policy.json`: `{ "name", "input": { "tool", ... }, "expect": { "code" } }`.
3. `ordo guard test` — the guardrails themselves must be green.
4. Debug a decision: `cd .ordo-guard && ordo trace policy --input '{"tool":"Bash","command":"git push"}'`.
5. `ordo guard log` — recent live decisions.

Guard is defense-in-depth, not a sandbox: it sees tool calls, not their side
effects (e.g. `sed -i` can edit files a `Write` rule would catch).
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use ordo_studio_format::studio_draft_to_engine_with_concepts;

    #[test]
    fn scaffolded_policy_parses_converts_and_compiles() {
        let studio: StudioRuleSet = serde_json::from_str(POLICY_JSON).unwrap();
        let mut engine = studio_draft_to_engine_with_concepts(&studio, &[]).unwrap();
        engine.compile().unwrap();
        assert_eq!(engine.config.name, "policy");
    }

    #[test]
    fn scaffolded_tests_and_facts_parse() {
        let tests: serde_json::Value = serde_json::from_str(POLICY_TESTS).unwrap();
        assert_eq!(tests.as_array().unwrap().len(), 7);
        for case in tests.as_array().unwrap() {
            assert!(case.get("name").is_some() && case.get("input").is_some());
            assert!(case["expect"]["code"].is_string());
        }
        let facts: serde_json::Value = serde_json::from_str(POLICY_FACTS).unwrap();
        assert!(facts
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["name"] == "command"));
    }

    #[test]
    fn settings_path_for_each_agent() {
        let root = Path::new("/repo");
        assert_eq!(
            settings_path_for(root, Agent::Claude, false),
            Path::new("/repo/.claude/settings.local.json")
        );
        assert_eq!(
            settings_path_for(root, Agent::Claude, true),
            Path::new("/repo/.claude/settings.json")
        );
        // Codex/Cursor: one project-local file regardless of --shared.
        assert_eq!(
            settings_path_for(root, Agent::Codex, false),
            Path::new("/repo/.codex/hooks.json")
        );
        assert_eq!(
            settings_path_for(root, Agent::Codex, true),
            Path::new("/repo/.codex/hooks.json")
        );
        assert_eq!(
            settings_path_for(root, Agent::Cursor, false),
            Path::new("/repo/.cursor/hooks.json")
        );
    }
}
