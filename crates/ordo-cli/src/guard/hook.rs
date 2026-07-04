//! `ordo guard hook` — the PreToolUse executor on the hot path of every tool
//! call.
//!
//! Protocol (Claude Code hooks): the event arrives as JSON on stdin; a decision
//! is a single JSON envelope on stdout with exit code 0; exit 0 with **empty
//! stdout** means "no opinion" and Claude Code's normal permission flow applies.
//! Stdout purity is critical — nothing but the envelope may ever be printed
//! there, so after the TTY check this command never returns `Err` (which would
//! route through `main()`'s `--json` error printer) and never uses colored or
//! pretty output. Internal failures fail open (stderr warning, no opinion)
//! unless `--fail-closed` is set.

use anyhow::{Context, Result};
use clap::Args;
use std::io::{IsTerminal, Read};
use std::path::Path;

use super::audit::{self, AuditEntry};
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
}

/// The PreToolUse event. Only `tool_name` is required — every other field is
/// optional so a Claude Code contract change degrades to pass-through instead
/// of breaking the hook.
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
            "`ordo guard hook` reads a PreToolUse event on stdin — it is meant to be \
             invoked by Claude Code as a hook (see `ordo guard init`).\nTry: echo \
             '{{\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":\"ls\"}}}}' | ordo guard hook"
        );
    }

    let event = match read_event() {
        Ok(e) => e,
        Err(e) => {
            fail_open(&e, args.fail_closed);
            return Ok(());
        }
    };

    let Some(policy_dir) = super::resolve_policy_dir(args.policy_dir.as_deref()) else {
        let e = anyhow::anyhow!(
            "no guard policy found (looked for {}/{}) — run `ordo guard init`",
            super::POLICY_DIR_NAME,
            crate::project::CONFIG_FILE
        );
        fail_open(&e, args.fail_closed);
        return Ok(());
    };

    match decide(&policy_dir, &args.ruleset, &event) {
        Ok(decision) => {
            emit(&decision);
            if !args.no_log {
                audit::append(
                    &policy_dir,
                    &to_entry(&event, decision.action.as_str(), &decision),
                );
            }
        }
        Err(e) => {
            fail_open(&e, args.fail_closed);
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

fn read_event() -> Result<HookEvent> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("failed to read the hook event from stdin")?;
    if buf.trim().is_empty() {
        anyhow::bail!("empty hook event on stdin");
    }
    serde_json::from_str(&buf).context("invalid hook event JSON on stdin")
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

/// Print the decision envelope (compact, single line). `Pass` prints nothing.
fn emit(decision: &Decision) {
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

/// Fail open: warn on stderr, keep stdout empty (no opinion). With
/// `--fail-closed`, emit a deny envelope instead. Either way the process exits
/// 0 — exit 2 is Claude Code's "blocking error" and must never fire on an
/// internal guard fault.
fn fail_open(err: &anyhow::Error, fail_closed: bool) {
    eprintln!(
        "ordo guard: {err:#} ({})",
        if fail_closed {
            "failing closed"
        } else {
            "failing open"
        }
    );
    if fail_closed {
        let envelope = serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": format!("guard error (fail-closed): {err:#}"),
            }
        });
        println!("{}", serde_json::to_string(&envelope).unwrap_or_default());
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
}
