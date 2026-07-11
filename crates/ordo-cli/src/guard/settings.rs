//! Registers the guard as a pre-tool-call hook in an agent's config file,
//! merging an entry in and preserving everything else in the file.
//!
//! Claude Code and Codex CLI share one JSON shape (`register_hook`):
//! `{"hooks": {"PreToolUse": [{"matcher": ..., "hooks": [{"type": "command",
//! "command": ..., "timeout": ...}]}]}}` — Codex's own hooks.json docs show
//! the identical nesting, just under `.codex/hooks.json` instead of
//! `.claude/settings*.json`. Cursor's `.cursor/hooks.json` is a different,
//! flatter shape (`register_cursor_hook`): a top-level `"version"` key and
//! `{"command": ..., "timeout": ..., "failClosed": ...}` objects directly in
//! the `beforeShellExecution` array, no nested `"hooks"`/`"type"` wrapper.

use anyhow::{Context, Result};
use std::path::Path;

use super::Agent;

/// Marker used to recognize our own entry across re-runs and binary moves.
const COMMAND_MARKER: &str = "guard hook";
const HOOK_TIMEOUT_SECS: u64 = 10;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RegisterOutcome {
    Created,
    Updated,
    Unchanged,
}

/// The hook command string to register. Default: the absolute path of the
/// running binary (npx installs are not on PATH), suffixed with `--agent
/// <agent>` for every agent but Claude (its wire protocol is the flag's
/// default, so the registered command stays exactly what it always was —
/// zero change for existing Claude Code users). `--shared` uses an
/// npx-portable command safe to commit; `--command` overrides both and is
/// taken verbatim (no suffix appended — the caller owns correctness).
pub(crate) fn hook_command(shared: bool, custom: Option<String>, agent: Agent) -> Result<String> {
    if let Some(cmd) = custom {
        return Ok(cmd);
    }
    let base = if shared {
        "npx -y @ordo-engine/cli guard hook".to_string()
    } else {
        let exe = std::env::current_exe().context("cannot determine the ordo binary path")?;
        let exe = exe.canonicalize().unwrap_or(exe);
        let path = exe.display().to_string();
        let quoted = if path.contains(char::is_whitespace) {
            format!("\"{path}\"")
        } else {
            path
        };
        format!("{quoted} guard hook")
    };
    Ok(match agent {
        Agent::Claude => base,
        Agent::Codex => format!("{base} --agent codex"),
        Agent::Cursor => format!("{base} --agent cursor"),
    })
}

/// Merge the PreToolUse hook entry into `settings_path` (Claude Code's
/// `.claude/settings*.json` or Codex CLI's `.codex/hooks.json` — identical
/// shape). Idempotent: an existing entry with the same command is left
/// alone; an entry pointing at a moved binary is updated in place; otherwise
/// a new matcher group is appended.
pub(crate) fn register_hook(settings_path: &Path, command: &str) -> Result<RegisterOutcome> {
    let mut root: serde_json::Value = if settings_path.is_file() {
        let text = std::fs::read_to_string(settings_path)
            .with_context(|| format!("failed to read {}", settings_path.display()))?;
        if text.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&text)
                .with_context(|| format!("invalid JSON in {}", settings_path.display()))?
        }
    } else {
        serde_json::json!({})
    };
    if !root.is_object() {
        anyhow::bail!(
            "{} is not a JSON object — refusing to overwrite it",
            settings_path.display()
        );
    }

    let entries = root
        .as_object_mut()
        .expect("checked above")
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("\"hooks\" is not an object")?
        .entry("PreToolUse")
        .or_insert_with(|| serde_json::json!([]));
    let entries = entries
        .as_array_mut()
        .context("\"hooks.PreToolUse\" is not an array")?;

    let mut outcome = None;
    'scan: for matcher_entry in entries.iter_mut() {
        let Some(hooks) = matcher_entry
            .get_mut("hooks")
            .and_then(|h| h.as_array_mut())
        else {
            continue;
        };
        for hook in hooks {
            let Some(existing) = hook.get("command").and_then(|c| c.as_str()) else {
                continue;
            };
            if existing.contains(COMMAND_MARKER) {
                outcome = Some(if existing == command {
                    RegisterOutcome::Unchanged
                } else {
                    hook["command"] = serde_json::json!(command);
                    RegisterOutcome::Updated
                });
                break 'scan;
            }
        }
    }
    let outcome = outcome.unwrap_or_else(|| {
        entries.push(serde_json::json!({
            "matcher": "*",
            "hooks": [{ "type": "command", "command": command, "timeout": HOOK_TIMEOUT_SECS }],
        }));
        RegisterOutcome::Created
    });

    if outcome != RegisterOutcome::Unchanged {
        if let Some(parent) = settings_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        std::fs::write(
            settings_path,
            format!("{}\n", serde_json::to_string_pretty(&root)?),
        )
        .with_context(|| format!("failed to write {}", settings_path.display()))?;
    }
    Ok(outcome)
}

/// Merge the `beforeShellExecution` hook entry into `settings_path` (Cursor's
/// `.cursor/hooks.json`). Same idempotency contract as `register_hook`, but a
/// different shape: a flat `{command, timeout, failClosed}` object per entry
/// (no nested `"hooks"` array, no `"type"` field), plus a top-level
/// `"version"` key. `matcher` is deliberately omitted — it's an optional
/// command-pattern filter in Cursor's schema, and guard wants to see every
/// shell command so the policy itself decides.
pub(crate) fn register_cursor_hook(settings_path: &Path, command: &str) -> Result<RegisterOutcome> {
    let mut root: serde_json::Value = if settings_path.is_file() {
        let text = std::fs::read_to_string(settings_path)
            .with_context(|| format!("failed to read {}", settings_path.display()))?;
        if text.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&text)
                .with_context(|| format!("invalid JSON in {}", settings_path.display()))?
        }
    } else {
        serde_json::json!({})
    };
    if !root.is_object() {
        anyhow::bail!(
            "{} is not a JSON object — refusing to overwrite it",
            settings_path.display()
        );
    }
    let obj = root.as_object_mut().expect("checked above");
    obj.entry("version").or_insert_with(|| serde_json::json!(1));

    let entries = obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("\"hooks\" is not an object")?
        .entry("beforeShellExecution")
        .or_insert_with(|| serde_json::json!([]));
    let entries = entries
        .as_array_mut()
        .context("\"hooks.beforeShellExecution\" is not an array")?;

    let mut outcome = None;
    for entry in entries.iter_mut() {
        let Some(existing) = entry.get("command").and_then(|c| c.as_str()) else {
            continue;
        };
        if existing.contains(COMMAND_MARKER) {
            outcome = Some(if existing == command {
                RegisterOutcome::Unchanged
            } else {
                entry["command"] = serde_json::json!(command);
                RegisterOutcome::Updated
            });
            break;
        }
    }
    let outcome = outcome.unwrap_or_else(|| {
        entries.push(serde_json::json!({
            "command": command,
            "timeout": HOOK_TIMEOUT_SECS,
            "failClosed": false,
        }));
        RegisterOutcome::Created
    });

    if outcome != RegisterOutcome::Unchanged {
        if let Some(parent) = settings_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        std::fs::write(
            settings_path,
            format!("{}\n", serde_json::to_string_pretty(&root)?),
        )
        .with_context(|| format!("failed to write {}", settings_path.display()))?;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_settings(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ordo-guard-settings-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("settings.local.json")
    }

    #[test]
    fn creates_settings_file_from_scratch() {
        let path = temp_settings("create");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            register_hook(&path, "/bin/ordo guard hook").unwrap(),
            RegisterOutcome::Created
        );
        let root: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            root["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "/bin/ordo guard hook"
        );
        assert_eq!(root["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"], 10);
    }

    #[test]
    fn preserves_unrelated_keys_and_entries_and_is_idempotent() {
        let path = temp_settings("merge");
        std::fs::write(
            &path,
            r#"{"permissions":{"allow":["Bash(ls)"]},"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"other-tool check"}]}],"PostToolUse":[]}}"#,
        )
        .unwrap();
        assert_eq!(
            register_hook(&path, "/bin/ordo guard hook").unwrap(),
            RegisterOutcome::Created
        );
        assert_eq!(
            register_hook(&path, "/bin/ordo guard hook").unwrap(),
            RegisterOutcome::Unchanged
        );

        let root: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root["permissions"]["allow"][0], "Bash(ls)");
        assert_eq!(root["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        assert_eq!(
            root["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "other-tool check"
        );
        assert!(root["hooks"]["PostToolUse"].is_array());
    }

    #[test]
    fn updates_moved_binary_in_place() {
        let path = temp_settings("update");
        let _ = std::fs::remove_file(&path);
        register_hook(&path, "/old/ordo guard hook").unwrap();
        assert_eq!(
            register_hook(&path, "/new/ordo guard hook").unwrap(),
            RegisterOutcome::Updated
        );
        let root: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let entries = root["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["hooks"][0]["command"], "/new/ordo guard hook");
    }

    #[test]
    fn rejects_non_object_settings_root() {
        let path = temp_settings("reject");
        std::fs::write(&path, "[1, 2]").unwrap();
        assert!(register_hook(&path, "x guard hook").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[1, 2]");
    }

    #[test]
    fn hook_command_variants() {
        assert_eq!(
            hook_command(true, None, Agent::Claude).unwrap(),
            "npx -y @ordo-engine/cli guard hook"
        );
        assert_eq!(
            hook_command(true, Some("custom".into()), Agent::Cursor).unwrap(),
            "custom",
            "a custom command is taken verbatim, no agent suffix appended"
        );
        let default = hook_command(false, None, Agent::Claude).unwrap();
        assert!(
            default.ends_with(" guard hook"),
            "Claude gets no --agent suffix (backward compat): got {default}"
        );
        assert_eq!(
            hook_command(true, None, Agent::Codex).unwrap(),
            "npx -y @ordo-engine/cli guard hook --agent codex"
        );
        assert_eq!(
            hook_command(true, None, Agent::Cursor).unwrap(),
            "npx -y @ordo-engine/cli guard hook --agent cursor"
        );
    }

    fn temp_cursor_settings(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ordo-guard-cursor-settings-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("hooks.json")
    }

    #[test]
    fn cursor_creates_flat_shape_with_version() {
        let path = temp_cursor_settings("create");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            register_cursor_hook(&path, "/bin/ordo guard hook --agent cursor").unwrap(),
            RegisterOutcome::Created
        );
        let root: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root["version"], 1);
        let entries = root["hooks"]["beforeShellExecution"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["command"], "/bin/ordo guard hook --agent cursor");
        assert_eq!(entries[0]["timeout"], 10);
        assert_eq!(entries[0]["failClosed"], false);
        // Flat shape: no nested "hooks" array or "type" field per entry.
        assert!(entries[0].get("hooks").is_none());
        assert!(entries[0].get("type").is_none());
    }

    #[test]
    fn cursor_preserves_unrelated_keys_and_is_idempotent() {
        let path = temp_cursor_settings("merge");
        std::fs::write(
            &path,
            r#"{"version":1,"hooks":{"beforeShellExecution":[{"command":"other-tool check","matcher":"curl"}],"afterFileEdit":[]}}"#,
        )
        .unwrap();
        let cmd = "/bin/ordo guard hook --agent cursor";
        assert_eq!(
            register_cursor_hook(&path, cmd).unwrap(),
            RegisterOutcome::Created
        );
        assert_eq!(
            register_cursor_hook(&path, cmd).unwrap(),
            RegisterOutcome::Unchanged
        );

        let root: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let entries = root["hooks"]["beforeShellExecution"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["command"], "other-tool check");
        assert!(root["hooks"]["afterFileEdit"].is_array());
    }

    #[test]
    fn cursor_updates_moved_binary_in_place() {
        let path = temp_cursor_settings("update");
        let _ = std::fs::remove_file(&path);
        register_cursor_hook(&path, "/old/ordo guard hook --agent cursor").unwrap();
        assert_eq!(
            register_cursor_hook(&path, "/new/ordo guard hook --agent cursor").unwrap(),
            RegisterOutcome::Updated
        );
        let root: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let entries = root["hooks"]["beforeShellExecution"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["command"], "/new/ordo guard hook --agent cursor");
    }

    #[test]
    fn cursor_rejects_non_object_settings_root() {
        let path = temp_cursor_settings("reject");
        std::fs::write(&path, "[1, 2]").unwrap();
        assert!(register_cursor_hook(&path, "x guard hook --agent cursor").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[1, 2]");
    }
}
