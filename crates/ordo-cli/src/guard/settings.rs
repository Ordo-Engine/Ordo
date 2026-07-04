//! Registers the guard as a Claude Code PreToolUse hook by merging an entry
//! into a settings JSON file, preserving everything else in the file.

use anyhow::{Context, Result};
use std::path::Path;

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
/// running binary (npx installs are not on PATH). `--shared` uses an
/// npx-portable command safe to commit; `--command` overrides both.
pub(crate) fn hook_command(shared: bool, custom: Option<String>) -> Result<String> {
    if let Some(cmd) = custom {
        return Ok(cmd);
    }
    if shared {
        return Ok("npx -y @ordo-engine/cli guard hook".to_string());
    }
    let exe = std::env::current_exe().context("cannot determine the ordo binary path")?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let path = exe.display().to_string();
    let quoted = if path.contains(char::is_whitespace) {
        format!("\"{path}\"")
    } else {
        path
    };
    Ok(format!("{quoted} guard hook"))
}

/// Merge the PreToolUse hook entry into `settings_path`. Idempotent: an
/// existing entry with the same command is left alone; an entry pointing at a
/// moved binary is updated in place; otherwise a new matcher group is appended.
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
            hook_command(true, None).unwrap(),
            "npx -y @ordo-engine/cli guard hook"
        );
        assert_eq!(hook_command(true, Some("custom".into())).unwrap(), "custom");
        let default = hook_command(false, None).unwrap();
        assert!(default.ends_with(" guard hook"), "got: {default}");
    }
}
