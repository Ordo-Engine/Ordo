//! `ordo guard` — agent guardrails: a deterministic policy gate for Claude Code
//! tool calls.
//!
//! Every PreToolUse event is piped through a local Ordo ruleset that decides
//! allow / deny / ask. The policy lives in `.ordo-guard/` as a normal Ordo
//! project, so the guardrails themselves have a test suite (`ordo guard test`)
//! and are trace-debuggable (`ordo trace`), and every decision is appended to a
//! JSONL audit log (`ordo guard log`).

use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::{Path, PathBuf};

mod audit;
mod hook;
mod init;
mod log;
mod settings;
mod test;

/// Directory holding the guard policy project, relative to the repo root.
pub const POLICY_DIR_NAME: &str = ".ordo-guard";
/// The ruleset name the hook evaluates by default.
pub const DEFAULT_RULESET: &str = "policy";

#[derive(Args)]
pub struct GuardArgs {
    #[command(subcommand)]
    cmd: GuardCmd,
}

#[derive(Subcommand)]
enum GuardCmd {
    /// Scaffold a guard policy project and register the Claude Code hook
    Init(init::GuardInitArgs),
    /// PreToolUse hook executor: reads the event on stdin, prints a decision
    Hook(hook::HookArgs),
    /// Show recent guard decisions from the audit log
    Log(log::LogArgs),
    /// Run the policy project's test cases
    Test(test::GuardTestArgs),
}

pub fn run(args: GuardArgs, json: bool) -> Result<()> {
    match args.cmd {
        GuardCmd::Init(a) => init::run(a, json),
        GuardCmd::Hook(a) => hook::run(a, json),
        GuardCmd::Log(a) => log::run(a, json),
        GuardCmd::Test(a) => test::run(a, json),
    }
}

/// Locate the guard policy project: `--policy-dir` → `$ORDO_GUARD_DIR` →
/// `$CLAUDE_PROJECT_DIR/.ordo-guard` → walk up from the cwd. Except for the
/// explicit flag, a candidate only counts when it contains `ordo.yaml`.
pub(crate) fn resolve_policy_dir(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(dir) = explicit {
        return Some(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var("ORDO_GUARD_DIR") {
        if !dir.trim().is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    if let Ok(project_dir) = std::env::var("CLAUDE_PROJECT_DIR") {
        let candidate = Path::new(&project_dir).join(POLICY_DIR_NAME);
        if is_policy_project(&candidate) {
            return Some(candidate);
        }
    }
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(POLICY_DIR_NAME);
        if is_policy_project(&candidate) {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn is_policy_project(dir: &Path) -> bool {
    dir.join(crate::project::CONFIG_FILE).is_file()
}
