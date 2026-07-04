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
