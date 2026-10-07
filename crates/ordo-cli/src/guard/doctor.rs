//! `ordo guard doctor` — check that guard is actually protecting this repo.
//!
//! Every agent treats a hook that fails to start as non-blocking: if the
//! registered binary is gone (an npx cache clean, a moved checkout) the agent
//! just runs the tool call, and nothing tells the user guard is off. This
//! command checks each link of the chain — policy compiles, tests pass, a
//! hook is registered, its program exists — and then runs every registered
//! hook end to end with a sample destructive command, the way the agent
//! would.

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::settings::registered_commands;
use super::Agent;
use crate::project::Project;

/// The command each hook is sent during the end-to-end check.
const PROBE_COMMAND: &str = "rm -rf /";
/// The same command with split flags — a substring-matching policy misses it.
const SPLIT_FLAGS_PROBE: &str = "rm -r -f /";
/// Generous: the first `npx` run of a fresh version may download it.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Args)]
pub struct DoctorArgs {
    /// Repo root whose agent config to check (default: current directory)
    #[arg(default_value = ".")]
    dir: String,

    /// Guard policy project directory (default: <DIR>/.ordo-guard, else auto-discovered)
    #[arg(long, value_name = "DIR")]
    policy_dir: Option<String>,

    /// Ruleset the hook evaluates
    #[arg(long, default_value = super::DEFAULT_RULESET)]
    ruleset: String,
}

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(serde::Serialize)]
struct Check {
    name: String,
    status: Status,
    detail: String,
}

struct Report(Vec<Check>);

impl Report {
    fn push(&mut self, name: impl Into<String>, status: Status, detail: impl Into<String>) {
        self.0.push(Check {
            name: name.into(),
            status,
            detail: detail.into(),
        });
    }
}

pub fn run(args: DoctorArgs, json: bool) -> Result<()> {
    let root = PathBuf::from(&args.dir);
    let root = root.canonicalize().unwrap_or(root);
    let mut report = Report(Vec::new());

    let policy_dir = args.policy_dir.as_ref().map(PathBuf::from).or_else(|| {
        let local = root.join(super::POLICY_DIR_NAME);
        if local.join(crate::project::CONFIG_FILE).is_file() {
            Some(local)
        } else {
            super::resolve_policy_dir(None)
        }
    });
    match &policy_dir {
        Some(dir) => {
            report.push("policy", Status::Ok, dir.display().to_string());
            check_policy(dir, &args.ruleset, &mut report);
        }
        None => report.push(
            "policy",
            Status::Fail,
            format!(
                "no {}/ policy project found — run `ordo guard init`",
                super::POLICY_DIR_NAME
            ),
        ),
    }

    let mut hooks = 0;
    for (agent, rel) in [
        (Agent::Claude, ".claude/settings.local.json"),
        (Agent::Claude, ".claude/settings.json"),
        (Agent::Codex, ".codex/hooks.json"),
        (Agent::Cursor, ".cursor/hooks.json"),
    ] {
        for command in registered_commands(&root.join(rel), agent) {
            hooks += 1;
            check_hook(&root, agent, rel, &command, &mut report);
        }
    }
    if hooks == 0 {
        report.push(
            "hook",
            Status::Fail,
            "no guard hook registered for Claude Code, Codex CLI or Cursor — run `ordo guard init`",
        );
    }

    let failed = report.0.iter().any(|c| c.status == Status::Fail);
    if json {
        crate::output::emit_json(&serde_json::json!({ "ok": !failed, "checks": report.0 }))?;
    } else {
        for c in &report.0 {
            let mark = match c.status {
                Status::Ok => "✔".green(),
                Status::Warn => "!".yellow(),
                Status::Fail => "✘".red(),
            };
            println!("{mark} {}: {}", c.name, c.detail);
        }
        println!();
        if failed {
            println!("{}", "guard is NOT fully protecting this repo".red());
        } else {
            println!("{}", "guard is active".green());
        }
    }
    if failed {
        std::process::exit(1);
    }
    Ok(())
}

fn check_policy(dir: &Path, ruleset: &str, report: &mut Report) {
    let project = match Project::discover(Some(dir)) {
        Ok(p) => p,
        Err(e) => return report.push("policy evaluates", Status::Fail, format!("{e:#}")),
    };
    // Compile, then evaluate sample events — a ruleset can compile and still
    // fail at run time (e.g. a dangling step id).
    let codes = project.load_engine(ruleset).and_then(|mut engine| {
        engine
            .compile()
            .map_err(|e| anyhow::anyhow!("compile error in {ruleset}: {e}"))?;
        let version = engine.config.version.clone();
        let rule = crate::runtime::LoadedRule::Source(engine);
        let code = |command: &str| -> Result<String> {
            let input = serde_json::from_value(serde_json::json!({
                "tool": "Bash",
                "command": command,
            }))?;
            let result = crate::runtime::execute_loaded_rule(
                &rule,
                super::test::prepare_case(input),
                false,
            )?;
            Ok(result.code)
        };
        Ok((version, code(PROBE_COMMAND)?, code(SPLIT_FLAGS_PROBE)?))
    });
    match codes {
        Ok((version, plain, split)) => {
            report.push(
                "policy evaluates",
                Status::Ok,
                format!("{ruleset}@{version}"),
            );
            if plain == "DENY" && split != "DENY" {
                // A policy scaffolded before shell parsing still matches
                // substrings of `command`.
                report.push(
                    "shell rules",
                    Status::Warn,
                    format!(
                        "`{PROBE_COMMAND}` is denied but `{SPLIT_FLAGS_PROBE}` gets {split} — \
                         match `programs` / `argv` instead of `command contains …` \
                         (see `ordo guard init` in an empty directory for the current default)"
                    ),
                );
            }
        }
        Err(e) => {
            // The hook fails open on this — every tool call goes through.
            return report.push(
                "policy evaluates",
                Status::Fail,
                format!("{e:#} (the hook fails open while this is broken)"),
            );
        }
    }
    if !project.tests_path(ruleset).is_file() {
        return report.push("policy tests", Status::Warn, "no tests file");
    }
    match crate::test_runner::run_project_ruleset_with(&project, ruleset, super::test::prepare_case)
    {
        Ok(summary) => {
            let total = summary["total"].as_u64().unwrap_or(0);
            let failed = summary["failed"].as_u64().unwrap_or(0);
            if failed == 0 {
                report.push("policy tests", Status::Ok, format!("{total} passed"));
            } else {
                report.push(
                    "policy tests",
                    Status::Warn,
                    format!("{failed} of {total} failing — run `ordo guard test`"),
                );
            }
        }
        Err(e) => report.push("policy tests", Status::Warn, format!("{e:#}")),
    }
}

fn check_hook(root: &Path, agent: Agent, rel: &str, command: &str, report: &mut Report) {
    let name = format!("{} hook ({rel})", agent.label());
    let program = first_word(command);
    if !program_exists(&program) {
        return report.push(
            name,
            Status::Fail,
            format!(
                "`{program}` not found — {} will skip this hook silently; re-run `ordo guard init`",
                agent.label()
            ),
        );
    }
    match probe(root, agent, command) {
        Ok(Probe::Decision(d)) if d == "deny" => report.push(
            name,
            Status::Ok,
            format!("answers `{PROBE_COMMAND}` with deny"),
        ),
        Ok(Probe::Decision(d)) => report.push(
            name,
            Status::Warn,
            format!("runs, but the policy answers `{PROBE_COMMAND}` with {d}"),
        ),
        Ok(Probe::FailedOpen(msg)) => report.push(name, Status::Fail, msg),
        Err(e) => report.push(name, Status::Fail, format!("{e:#}")),
    }
}

enum Probe {
    /// allow / deny / ask, or "no opinion".
    Decision(String),
    /// The hook ran but reported an internal error and let the call through.
    FailedOpen(String),
}

/// Run `command` the way the agent would — through a shell, from the repo
/// root, with a pre-tool-call event for `PROBE_COMMAND` on stdin.
fn probe(root: &Path, agent: Agent, command: &str) -> Result<Probe> {
    let event = match agent {
        Agent::Claude | Agent::Codex => serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": { "command": PROBE_COMMAND },
            "cwd": root,
        }),
        Agent::Cursor => serde_json::json!({ "command": PROBE_COMMAND, "cwd": root }),
    };
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };
    let mut child = cmd
        .current_dir(root)
        .env("CLAUDE_PROJECT_DIR", root)
        .env("ORDO_GUARD_NO_LOG", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to start the hook: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(event.to_string().as_bytes());
    }
    let start = Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() > PROBE_TIMEOUT {
            let _ = child.kill();
            anyhow::bail!(
                "the hook did not answer within {}s",
                PROBE_TIMEOUT.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        anyhow::bail!(
            "the hook exited with {} — {}",
            out.status,
            stderr.trim().lines().last().unwrap_or("no output")
        );
    }
    if let Some(line) = stderr.lines().find(|l| l.contains("failing open")) {
        return Ok(Probe::FailedOpen(line.trim().to_string()));
    }
    let decision = if stdout.trim().is_empty() {
        "no opinion".to_string()
    } else {
        let v: serde_json::Value = serde_json::from_str(stdout.trim())
            .map_err(|e| anyhow::anyhow!("the hook printed invalid JSON ({e}): {stdout}"))?;
        let d = match agent {
            Agent::Claude | Agent::Codex => &v["hookSpecificOutput"]["permissionDecision"],
            Agent::Cursor => &v["permission"],
        };
        d.as_str().unwrap_or("no opinion").to_string()
    };
    Ok(Probe::Decision(decision))
}

/// The program part of a registered command (`"/a b/ordo" guard hook` → `/a b/ordo`).
fn first_word(command: &str) -> String {
    let command = command.trim_start();
    if let Some(rest) = command.strip_prefix('"') {
        rest.split('"').next().unwrap_or_default().to_string()
    } else {
        command
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string()
    }
}

fn program_exists(program: &str) -> bool {
    let path = Path::new(program);
    if path.components().count() > 1 {
        return is_executable(path);
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| {
        let candidate = dir.join(program);
        is_executable(&candidate)
            || (cfg!(windows)
                && ["exe", "cmd", "bat"]
                    .iter()
                    .any(|ext| candidate.with_extension(ext).is_file()))
    })
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_word_handles_quoted_paths() {
        assert_eq!(first_word("/bin/ordo guard hook"), "/bin/ordo");
        assert_eq!(
            first_word("\"/Users/a b/ordo\" guard hook --agent codex"),
            "/Users/a b/ordo"
        );
        assert_eq!(first_word("npx -y @ordo-engine/cli guard hook"), "npx");
    }

    #[cfg(unix)]
    #[test]
    fn program_exists_checks_paths_and_path_lookup() {
        assert!(program_exists("sh"));
        assert!(program_exists("/bin/sh"));
        assert!(!program_exists("/nonexistent/ordo"));
        assert!(!program_exists("definitely-not-a-real-program-xyz"));
    }
}
