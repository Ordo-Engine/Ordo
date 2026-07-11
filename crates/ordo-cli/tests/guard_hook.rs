//! Integration tests for `ordo guard` — drive the real binary end-to-end:
//! init → hook decisions over stdin → policy tests → audit log.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_ordo");

fn temp_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ordo-guard-it-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(dir: &PathBuf, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env_remove("CLAUDE_PROJECT_DIR")
        .env_remove("ORDO_GUARD_DIR")
        .output()
        .expect("failed to run ordo")
}

/// Run the binary with `body` piped to stdin (the hook protocol).
fn run_stdin(dir: &PathBuf, args: &[&str], body: &str) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env_remove("CLAUDE_PROJECT_DIR")
        .env_remove("ORDO_GUARD_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn ordo");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    child.wait_with_output().expect("failed to wait for ordo")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn assert_ok(out: &Output, what: &str) {
    assert!(
        out.status.success(),
        "{what} failed\nstdout: {}\nstderr: {}",
        stdout(out),
        stderr(out)
    );
}

fn decision(out: &Output) -> serde_json::Value {
    let text = stdout(out);
    let v: serde_json::Value = serde_json::from_str(text.trim())
        .unwrap_or_else(|e| panic!("hook stdout is not JSON: {e}\nstdout: {text}"));
    v["hookSpecificOutput"].clone()
}

#[test]
fn guard_init_scaffolds_and_registers_idempotently() {
    let dir = temp_project("init");
    let out = run(&dir, &["guard", "init"]);
    assert_ok(&out, "guard init");

    for f in [
        ".ordo-guard/ordo.yaml",
        ".ordo-guard/rulesets/policy.json",
        ".ordo-guard/tests/policy.json",
        ".ordo-guard/facts.json",
        ".ordo-guard/concepts.json",
        ".ordo-guard/AGENTS.md",
        ".ordo-guard/.gitignore",
        ".claude/settings.local.json",
    ] {
        assert!(dir.join(f).is_file(), "missing {f}");
    }

    let settings: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join(".claude/settings.local.json")).unwrap(),
    )
    .unwrap();
    let entries = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    let command = entries[0]["hooks"][0]["command"].as_str().unwrap();
    assert!(command.ends_with("guard hook"), "got command: {command}");

    // Re-run: no duplicate hook entry, scaffold untouched.
    let out = run(&dir, &["guard", "init", "--json"]);
    assert_ok(&out, "guard init rerun");
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["scaffolded"], false);
    assert_eq!(v["hook"]["outcome"], "unchanged");
    let settings: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join(".claude/settings.local.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
}

#[test]
fn guard_hook_denies_rm_rf_and_logs_it() {
    let dir = temp_project("deny");
    assert_ok(&run(&dir, &["guard", "init", "--no-hook"]), "guard init");

    let event = r#"{"session_id":"s1","hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/w","tool_input":{"command":"rm -rf /tmp/x"}}"#;
    let out = run_stdin(&dir, &["guard", "hook"], event);
    assert_ok(&out, "guard hook");

    let d = decision(&out);
    assert_eq!(d["hookEventName"], "PreToolUse");
    assert_eq!(d["permissionDecision"], "deny");
    let reason = d["permissionDecisionReason"].as_str().unwrap();
    assert!(reason.contains("Destructive"), "got reason: {reason}");
    assert!(reason.contains("policy@1.0.0"), "got reason: {reason}");

    let log = std::fs::read_to_string(dir.join(".ordo-guard/log.jsonl")).unwrap();
    let entry: serde_json::Value = serde_json::from_str(log.lines().next().unwrap()).unwrap();
    assert_eq!(entry["decision"], "deny");
    assert_eq!(entry["tool"], "Bash");
    assert_eq!(entry["session_id"], "s1");
    assert_eq!(entry["summary"], "rm -rf /tmp/x");
}

#[test]
fn guard_hook_asks_on_git_push_and_allows_readonly_git() {
    let dir = temp_project("ask-allow");
    assert_ok(&run(&dir, &["guard", "init", "--no-hook"]), "guard init");

    let push = r#"{"tool_name":"Bash","tool_input":{"command":"git push origin main"}}"#;
    let out = run_stdin(&dir, &["guard", "hook"], push);
    assert_ok(&out, "guard hook (push)");
    assert_eq!(decision(&out)["permissionDecision"], "ask");

    let status = r#"{"tool_name":"Bash","tool_input":{"command":"git status"}}"#;
    let out = run_stdin(&dir, &["guard", "hook"], status);
    assert_ok(&out, "guard hook (status)");
    assert_eq!(decision(&out)["permissionDecision"], "allow");
}

#[test]
fn guard_hook_pass_is_silent() {
    let dir = temp_project("pass");
    assert_ok(&run(&dir, &["guard", "init", "--no-hook"]), "guard init");

    let event = r#"{"tool_name":"Edit","tool_input":{"file_path":"src/main.rs"}}"#;
    let out = run_stdin(&dir, &["guard", "hook"], event);
    assert_ok(&out, "guard hook (pass)");
    assert_eq!(stdout(&out), "", "PASS must print nothing to stdout");
}

#[test]
fn guard_hook_fails_open_without_a_policy() {
    let dir = temp_project("failopen");
    let event = r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#;
    let out = run_stdin(&dir, &["guard", "hook"], event);
    assert_ok(&out, "guard hook (no policy)");
    assert_eq!(stdout(&out), "", "fail-open must print nothing to stdout");
    assert!(
        stderr(&out).contains("no guard policy"),
        "got stderr: {}",
        stderr(&out)
    );
}

#[test]
fn guard_hook_fail_closed_denies_on_broken_policy() {
    let dir = temp_project("failclosed");
    assert_ok(&run(&dir, &["guard", "init", "--no-hook"]), "guard init");
    std::fs::write(dir.join(".ordo-guard/rulesets/policy.json"), "not json").unwrap();

    let event = r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#;
    let out = run_stdin(&dir, &["guard", "hook", "--fail-closed"], event);
    assert_ok(&out, "guard hook (fail-closed)");
    let d = decision(&out);
    assert_eq!(d["permissionDecision"], "deny");
    assert!(d["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .contains("fail-closed"));
}

#[test]
fn guard_policy_project_is_testable_and_valid() {
    let dir = temp_project("test");
    assert_ok(&run(&dir, &["guard", "init", "--no-hook"]), "guard init");

    let out = run(&dir, &["guard", "test", "--json"]);
    assert_ok(&out, "guard test");
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["failed"], 0, "policy tests failed: {v}");
    assert_eq!(v["total"], 7);

    // The policy is a normal Ordo project — validate works inside it.
    let guard_dir = dir.join(".ordo-guard");
    let out = run(&guard_dir, &["validate", "--json"]);
    assert_ok(&out, "validate inside .ordo-guard");
}

#[test]
fn guard_log_tails_recent_decisions() {
    let dir = temp_project("log");
    assert_ok(&run(&dir, &["guard", "init", "--no-hook"]), "guard init");

    for cmd in ["rm -rf /a", "git push", "git status"] {
        let event = format!(r#"{{"tool_name":"Bash","tool_input":{{"command":"{cmd}"}}}}"#);
        assert_ok(&run_stdin(&dir, &["guard", "hook"], &event), "guard hook");
    }

    let out = run(&dir, &["guard", "log", "--json", "--tail", "2"]);
    assert_ok(&out, "guard log");
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let entries = v.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["decision"], "ask");
    assert_eq!(entries[1]["decision"], "allow");
}

// ── Multi-agent (Codex CLI / Cursor) ────────────────────────────────────────

#[test]
fn guard_init_registers_codex_hook_in_codex_shape() {
    let dir = temp_project("codex-init");
    let out = run(&dir, &["guard", "init", "--agent", "codex"]);
    assert_ok(&out, "guard init --agent codex");

    // No Claude file written when only Codex was selected.
    assert!(!dir.join(".claude/settings.local.json").exists());
    assert!(dir.join(".codex/hooks.json").is_file());

    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".codex/hooks.json")).unwrap())
            .unwrap();
    let entries = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    let command = entries[0]["hooks"][0]["command"].as_str().unwrap();
    assert!(
        command.ends_with("guard hook --agent codex"),
        "got command: {command}"
    );
}

#[test]
fn guard_init_registers_cursor_hook_in_cursor_flat_shape() {
    let dir = temp_project("cursor-init");
    let out = run(&dir, &["guard", "init", "--agent", "cursor"]);
    assert_ok(&out, "guard init --agent cursor");

    assert!(dir.join(".cursor/hooks.json").is_file());
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".cursor/hooks.json")).unwrap())
            .unwrap();
    assert_eq!(settings["version"], 1);
    let entries = settings["hooks"]["beforeShellExecution"]
        .as_array()
        .unwrap();
    assert_eq!(entries.len(), 1);
    // Flat shape: no nested "hooks" array, no "type" field.
    assert!(entries[0].get("hooks").is_none());
    let command = entries[0]["command"].as_str().unwrap();
    assert!(
        command.ends_with("guard hook --agent cursor"),
        "got command: {command}"
    );
}

#[test]
fn guard_init_registers_multiple_agents_in_one_call() {
    let dir = temp_project("multi-init");
    let out = run(
        &dir,
        &["guard", "init", "--agent", "claude,codex,cursor", "--json"],
    );
    assert_ok(&out, "guard init --agent claude,codex,cursor");

    assert!(dir.join(".claude/settings.local.json").is_file());
    assert!(dir.join(".codex/hooks.json").is_file());
    assert!(dir.join(".cursor/hooks.json").is_file());

    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    // Backward-compat singular "hook" still reflects Claude specifically.
    assert_eq!(v["hook"]["outcome"], "created");
    let hooks = v["hooks"].as_array().unwrap();
    assert_eq!(hooks.len(), 3);
    let agents: Vec<&str> = hooks.iter().map(|h| h["agent"].as_str().unwrap()).collect();
    assert_eq!(agents, vec!["claude", "codex", "cursor"]);

    // Re-run: all three idempotently unchanged, still exactly one entry each.
    let out = run(
        &dir,
        &["guard", "init", "--agent", "claude,codex,cursor", "--json"],
    );
    assert_ok(&out, "guard init rerun");
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    for h in v["hooks"].as_array().unwrap() {
        assert_eq!(
            h["outcome"], "unchanged",
            "agent {} not idempotent",
            h["agent"]
        );
    }
}

#[test]
fn guard_hook_agent_codex_speaks_identical_protocol_to_claude() {
    let dir = temp_project("codex-hook");
    assert_ok(&run(&dir, &["guard", "init", "--no-hook"]), "guard init");

    let event = r#"{"session_id":"s1","hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/w","tool_input":{"command":"rm -rf /tmp/x"}}"#;
    let out = run_stdin(&dir, &["guard", "hook", "--agent", "codex"], event);
    assert_ok(&out, "guard hook --agent codex");

    let d = decision(&out);
    assert_eq!(d["hookEventName"], "PreToolUse");
    assert_eq!(d["permissionDecision"], "deny");
    assert!(d["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .contains("Destructive"));
}

#[test]
fn guard_hook_agent_cursor_parses_flat_event_and_emits_flat_decision() {
    let dir = temp_project("cursor-hook-deny");
    assert_ok(&run(&dir, &["guard", "init", "--no-hook"]), "guard init");

    // Cursor's beforeShellExecution event: no tool_name/tool_input wrapper.
    let event = r#"{"command":"rm -rf /tmp/x","cwd":"/w","conversation_id":"c1","sandbox":true,"model":"gpt-5","hook_event_name":"beforeShellExecution"}"#;
    let out = run_stdin(&dir, &["guard", "hook", "--agent", "cursor"], event);
    assert_ok(&out, "guard hook --agent cursor (deny)");

    let v: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(v["permission"], "deny");
    assert!(v["user_message"].as_str().unwrap().contains("Destructive"));
    assert!(v["agent_message"].as_str().unwrap().contains("Destructive"));
    // Cursor's flat shape has no hookSpecificOutput wrapper.
    assert!(v.get("hookSpecificOutput").is_none());

    let log = std::fs::read_to_string(dir.join(".ordo-guard/log.jsonl")).unwrap();
    let entry: serde_json::Value = serde_json::from_str(log.lines().next().unwrap()).unwrap();
    assert_eq!(
        entry["tool"], "Bash",
        "Cursor events are normalized to tool=Bash"
    );
    assert_eq!(
        entry["session_id"], "c1",
        "conversation_id maps to session_id"
    );

    // Read-only git → allow, still explicit (not silent) on Cursor.
    let allow_event = r#"{"command":"git status","cwd":"/w"}"#;
    let out = run_stdin(&dir, &["guard", "hook", "--agent", "cursor"], allow_event);
    assert_ok(&out, "guard hook --agent cursor (allow)");
    let v: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(v["permission"], "allow");

    // No policy rule matches a plain `ls` → PASS maps to an explicit allow on
    // Cursor (its schema has no documented "no opinion" outcome), unlike
    // Claude/Codex where PASS prints nothing.
    let pass_event = r#"{"command":"ls","cwd":"/w"}"#;
    let out = run_stdin(&dir, &["guard", "hook", "--agent", "cursor"], pass_event);
    assert_ok(&out, "guard hook --agent cursor (pass)");
    let v: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(v["permission"], "allow");
    assert!(v.get("user_message").is_none(), "PASS carries no message");
}
