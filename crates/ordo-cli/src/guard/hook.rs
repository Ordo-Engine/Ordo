//! `ordo guard hook` — the pre-tool-call executor on the hot path of every
//! tool call, for whichever agent registered it (`--agent`, default `claude`).
//!
//! Claude Code and Codex CLI speak the same envelope (event JSON on stdin →
//! `{"hookSpecificOutput": {...}}` decision JSON on stdout with exit 0; empty
//! stdout means "no opinion", the agent's normal permission flow applies), so
//! they share every code path below. Cursor's `beforeShellExecution` hook uses
//! its own flat event shape and decision envelope (`{"permission": ...}`) —
//! its event is converted into the same canonical `HookEvent` on the way in,
//! so the policy, audit log, and everything past `decide()` stay identical
//! across all three agents. Only `read_event`/`emit` branch on `Agent`.
//!
//! Stdout purity is critical — nothing but the decision envelope may ever be
//! printed there, so after the TTY check this command never returns `Err`
//! (which would route through `main()`'s `--json` error printer) and never
//! uses colored or pretty output. Internal failures fail open (stderr
//! warning, no opinion) unless `--fail-closed` is set.

use anyhow::{Context, Result};
use clap::Args;
use std::io::{IsTerminal, Read};
use std::path::Path;

use super::audit::{self, AuditEntry};
use super::Agent;
use crate::project::Project;
use crate::runtime::{execute_loaded_rule, LoadedRule};

#[derive(Args)]
pub struct HookArgs {
    /// Guard policy project directory (default: auto-discovered)
    #[arg(long, value_name = "DIR")]
    policy_dir: Option<String>,

    /// Ruleset to evaluate within the policy project
    #[arg(long, default_value = super::DEFAULT_RULESET)]
    ruleset: String,

    /// Deny the tool call when the guard itself fails (default: fail open)
    #[arg(long)]
    fail_closed: bool,

    /// Skip the audit-log append
    #[arg(long)]
    no_log: bool,

    /// Coding agent whose hook protocol this stdin event follows
    #[arg(long, value_enum, default_value_t = Agent::Claude)]
    agent: Agent,
}

/// The pre-tool-call event, in Claude Code / Codex CLI's shared shape. Only
/// `tool_name` is required — every other field is optional so a protocol
/// change on either agent degrades to pass-through instead of breaking the
/// hook. A Cursor event is parsed separately (`read_cursor_event`) and
/// converted into this same shape, so everything past this point is agent-
/// agnostic.
#[derive(serde::Deserialize)]
struct HookEvent {
    tool_name: String,
    #[serde(default)]
    tool_input: serde_json::Value,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    hook_event_name: Option<String>,
}

/// Cursor's `beforeShellExecution` event — shell commands only, no `tool`
/// concept (https://cursor.com/docs/hooks). Converted into a `HookEvent` with
/// `tool_name: "Bash"` so the policy sees the same `tool`/`command` facts it
/// would from Claude Code or Codex.
#[derive(serde::Deserialize, Debug)]
struct CursorShellEvent {
    command: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    conversation_id: Option<String>,
}

impl From<CursorShellEvent> for HookEvent {
    fn from(e: CursorShellEvent) -> Self {
        HookEvent {
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({ "command": e.command }),
            session_id: e.conversation_id,
            cwd: e.cwd,
            permission_mode: None,
            hook_event_name: Some("beforeShellExecution".to_string()),
        }
    }
}

enum Action {
    Allow,
    Deny,
    Ask,
    Pass,
}

impl Action {
    fn as_str(&self) -> &'static str {
        match self {
            Action::Allow => "allow",
            Action::Deny => "deny",
            Action::Ask => "ask",
            Action::Pass => "pass",
        }
    }
}

struct Decision {
    action: Action,
    code: String,
    reason: String,
    duration_us: u64,
}

pub fn run(args: HookArgs, _json: bool) -> Result<()> {
    if std::io::stdin().is_terminal() {
        anyhow::bail!(
            "`ordo guard hook` reads a pre-tool-call event on stdin — it is meant to be \
             invoked by {} as a hook (see `ordo guard init`).\nTry: echo \
             '{{\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":\"ls\"}}}}' | ordo guard hook",
            args.agent.label()
        );
    }

    let event = match read_event(args.agent) {
        Ok(e) => e,
        Err(e) => {
            fail_open(args.agent, &e, args.fail_closed);
            return Ok(());
        }
    };

    let Some(policy_dir) = super::resolve_policy_dir(args.policy_dir.as_deref()) else {
        let e = anyhow::anyhow!(
            "no guard policy found (looked for {}/{}) — run `ordo guard init`",
            super::POLICY_DIR_NAME,
            crate::project::CONFIG_FILE
        );
        fail_open(args.agent, &e, args.fail_closed);
        return Ok(());
    };

    match decide(&policy_dir, &args.ruleset, &event) {
        Ok(decision) => {
            emit(args.agent, &decision);
            if !args.no_log {
                audit::append(
                    &policy_dir,
                    &to_entry(&event, decision.action.as_str(), &decision),
                );
            }
        }
        Err(e) => {
            fail_open(args.agent, &e, args.fail_closed);
            if !args.no_log {
                let errored = Decision {
                    action: Action::Pass,
                    code: "ERROR".to_string(),
                    reason: format!("{e:#}"),
                    duration_us: 0,
                };
                audit::append(&policy_dir, &to_entry(&event, "error", &errored));
            }
        }
    }
    Ok(())
}

fn read_event(agent: Agent) -> Result<HookEvent> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("failed to read the hook event from stdin")?;
    if buf.trim().is_empty() {
        anyhow::bail!("empty hook event on stdin");
    }
    match agent {
        Agent::Claude | Agent::Codex => {
            serde_json::from_str(&buf).context("invalid hook event JSON on stdin")
        }
        Agent::Cursor => {
            let e: CursorShellEvent =
                serde_json::from_str(&buf).context("invalid Cursor hook event JSON on stdin")?;
            Ok(e.into())
        }
    }
}

/// Flatten the event into the ruleset input. Reserved keys always win; every
/// top-level `tool_input` key is hoisted for ergonomic conditions (`command`,
/// `file_path`, `url`, …). Absent values are *omitted*, never written as JSON
/// null — a missing field is lenient-false in a condition, while `null` is a
/// hard type error for operators like `contains`.
fn build_policy_input(event: &HookEvent) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("tool".into(), event.tool_name.clone().into());
    for (key, value) in [
        ("cwd", &event.cwd),
        ("permission_mode", &event.permission_mode),
        ("session_id", &event.session_id),
        ("hook_event_name", &event.hook_event_name),
    ] {
        if let Some(v) = value {
            map.insert(key.into(), v.clone().into());
        }
    }
    if let Some(input) = event.tool_input.as_object() {
        for (key, value) in input {
            if !value.is_null() && !map.contains_key(key) {
                map.insert(key.clone(), value.clone());
            }
        }
    }
    if !event.tool_input.is_null() {
        map.insert("tool_input".into(), event.tool_input.clone());
    }
    serde_json::Value::Object(map)
}

fn decide(policy_dir: &Path, ruleset: &str, event: &HookEvent) -> Result<Decision> {
    let project = Project::discover(Some(policy_dir))?;
    let mut engine = project.load_engine(ruleset)?;
    engine
        .compile()
        .map_err(|e| anyhow::anyhow!("compile error in {ruleset}: {e}"))?;
    let version = engine.config.version.clone();

    let input = serde_json::from_value(build_policy_input(event))
        .context("failed to convert the hook event into engine input")?;
    let result = execute_loaded_rule(&LoadedRule::Source(engine), input, false)?;

    let action = match result.code.as_str() {
        "ALLOW" => Action::Allow,
        "DENY" => Action::Deny,
        "ASK" => Action::Ask,
        _ => Action::Pass,
    };
    let base_reason = result
        .output
        .get_path("reason")
        .and_then(ordo_core::prelude::Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if result.message.is_empty() {
                format!("ordo guard: {}", result.code)
            } else {
                result.message.clone()
            }
        });
    Ok(Decision {
        reason: format!("{base_reason} [policy@{version} · {}]", result.code),
        code: result.code,
        action,
        duration_us: result.duration_us,
    })
}

/// Print the decision envelope in `agent`'s protocol (compact, single line).
fn emit(agent: Agent, decision: &Decision) {
    match agent {
        Agent::Claude | Agent::Codex => emit_pretooluse(decision),
        Agent::Cursor => emit_cursor(decision),
    }
}

/// Claude Code / Codex CLI shape: `Pass` prints nothing (exit 0, empty
/// stdout = no opinion, the agent's own permission flow applies).
fn emit_pretooluse(decision: &Decision) {
    let permission = match decision.action {
        Action::Allow => "allow",
        Action::Deny => "deny",
        Action::Ask => "ask",
        Action::Pass => return,
    };
    let envelope = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": permission,
            "permissionDecisionReason": decision.reason,
        }
    });
    // to_string cannot fail on this literal shape.
    println!("{}", serde_json::to_string(&envelope).unwrap_or_default());
}

/// Cursor shape (https://cursor.com/docs/hooks): a flat `{"permission": ...}`
/// object, no wrapper. Cursor's `beforeShellExecution` schema documents only
/// allow/deny/ask, not an explicit "no opinion" outcome, so unlike the
/// PreToolUse agents `Pass` is emitted explicitly as `allow` (with no
/// message) rather than left to undocumented empty-stdout behavior.
fn emit_cursor(decision: &Decision) {
    let permission = match decision.action {
        Action::Allow | Action::Pass => "allow",
        Action::Deny => "deny",
        Action::Ask => "ask",
    };
    let mut envelope = serde_json::json!({ "permission": permission });
    if !matches!(decision.action, Action::Pass) {
        envelope["user_message"] = serde_json::json!(decision.reason);
        envelope["agent_message"] = serde_json::json!(decision.reason);
    }
    println!("{}", serde_json::to_string(&envelope).unwrap_or_default());
}

/// Fail open: warn on stderr, keep stdout empty (no opinion). With
/// `--fail-closed`, emit a deny decision instead (in `agent`'s envelope).
/// Either way the process exits 0 — exit 2 is Claude Code's "blocking error"
/// and must never fire on an internal guard fault.
fn fail_open(agent: Agent, err: &anyhow::Error, fail_closed: bool) {
    eprintln!(
        "ordo guard: {err:#} ({})",
        if fail_closed {
            "failing closed"
        } else {
            "failing open"
        }
    );
    if fail_closed {
        emit(
            agent,
            &Decision {
                action: Action::Deny,
                code: "ERROR".to_string(),
                reason: format!("guard error (fail-closed): {err:#}"),
                duration_us: 0,
            },
        );
    }
}

fn to_entry(event: &HookEvent, decision: &str, d: &Decision) -> AuditEntry {
    AuditEntry {
        ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        session_id: event.session_id.clone(),
        tool: event.tool_name.clone(),
        decision: decision.to_string(),
        code: d.code.clone(),
        reason: d.reason.clone(),
        duration_us: d.duration_us,
        summary: summarize(&event.tool_input),
        cwd: event.cwd.clone(),
    }
}

/// A one-line, ≤200-char description of what the tool call was about.
fn summarize(tool_input: &serde_json::Value) -> String {
    let text = ["command", "file_path", "url", "description"]
        .iter()
        .find_map(|k| tool_input.get(k).and_then(|v| v.as_str()))
        .unwrap_or_default();
    let mut s: String = text.chars().take(200).collect();
    if text.chars().count() > 200 {
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(json: &str) -> HookEvent {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn input_hoists_tool_input_fields_and_keeps_nested_copy() {
        let e = event(
            r#"{"tool_name":"Bash","cwd":"/w","tool_input":{"command":"ls","description":"list"}}"#,
        );
        let input = build_policy_input(&e);
        assert_eq!(input["tool"], "Bash");
        assert_eq!(input["cwd"], "/w");
        assert_eq!(input["command"], "ls");
        assert_eq!(input["tool_input"]["command"], "ls");
    }

    #[test]
    fn input_omits_nulls_and_absent_fields() {
        let e = event(r#"{"tool_name":"Glob","tool_input":{"pattern":"*.rs","path":null}}"#);
        let input = build_policy_input(&e);
        let obj = input.as_object().unwrap();
        assert!(
            !obj.contains_key("path"),
            "null tool_input values must be omitted"
        );
        assert!(
            !obj.contains_key("cwd"),
            "absent event fields must be omitted"
        );
        assert_eq!(input["pattern"], "*.rs");
    }

    #[test]
    fn input_reserved_keys_are_never_overwritten() {
        let e = event(
            r#"{"tool_name":"Bash","cwd":"/real","tool_input":{"cwd":"/spoofed","tool":"Fake"}}"#,
        );
        let input = build_policy_input(&e);
        assert_eq!(input["cwd"], "/real");
        assert_eq!(input["tool"], "Bash");
    }

    #[test]
    fn input_tolerates_non_object_tool_input() {
        let e = event(r#"{"tool_name":"X","tool_input":"raw"}"#);
        let input = build_policy_input(&e);
        assert_eq!(input["tool_input"], "raw");
    }

    #[test]
    fn only_tool_name_is_required() {
        let e = event(r#"{"tool_name":"Bash"}"#);
        assert_eq!(e.tool_name, "Bash");
        assert!(event(r#"{"tool_name":"Bash","unknown_future_field":1}"#)
            .session_id
            .is_none());
    }

    #[test]
    fn summary_truncates_to_200_chars() {
        let long = "x".repeat(300);
        let s = summarize(&serde_json::json!({ "command": long }));
        assert_eq!(s.chars().count(), 201); // 200 + ellipsis
        assert!(s.ends_with('…'));
    }

    #[test]
    fn cursor_event_converts_to_bash_tool_call() {
        let e: CursorShellEvent = serde_json::from_str(
            r#"{"command":"rm -rf /tmp/x","cwd":"/w","conversation_id":"c1"}"#,
        )
        .unwrap();
        let hook_event: HookEvent = e.into();
        assert_eq!(hook_event.tool_name, "Bash");
        assert_eq!(hook_event.session_id.as_deref(), Some("c1"));
        assert_eq!(hook_event.cwd.as_deref(), Some("/w"));
        let input = build_policy_input(&hook_event);
        assert_eq!(input["tool"], "Bash");
        assert_eq!(input["command"], "rm -rf /tmp/x");
    }

    #[test]
    fn cursor_event_ignores_unknown_fields() {
        // Cursor's real event carries several fields we don't need
        // (sandbox, model, model_id, model_params, cursor_version,
        // workspace_roots, user_email, transcript_path, hook_event_name) —
        // must not fail to parse when they're present.
        let e: Result<CursorShellEvent, _> = serde_json::from_str(
            r#"{"command":"ls","cwd":"/w","sandbox":true,"model":"gpt","hook_event_name":"beforeShellExecution","workspace_roots":["/w"]}"#,
        );
        assert!(e.is_ok(), "{e:?}");
    }
}
