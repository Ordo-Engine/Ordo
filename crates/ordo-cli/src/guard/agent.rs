//! The coding agents `ordo guard` can hook into. Each speaks a slightly
//! different pre-tool-call protocol (event shape on stdin, decision envelope
//! on stdout, hook-registration config format) — this module is the only
//! place that distinction should ever need to live; `hook.rs` and `init.rs`
//! dispatch on it, `settings.rs` registers per-agent config.

use clap::ValueEnum;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[value(rename_all = "lower")]
pub enum Agent {
    /// Claude Code — PreToolUse hook, `.claude/settings(.local).json`.
    #[default]
    Claude,
    /// OpenAI Codex CLI — PreToolUse hook with the same envelope shape as
    /// Claude Code, `.codex/hooks.json`. Currently Bash-only upstream.
    Codex,
    /// Cursor — `beforeShellExecution` hook, its own flat envelope,
    /// `.cursor/hooks.json`. Shell commands only (no file-edit visibility).
    Cursor,
}

impl Agent {
    pub fn label(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex CLI",
            Agent::Cursor => "Cursor",
        }
    }

    /// Lowercase key matching the `--agent` CLI value and JSON output.
    pub fn key(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Cursor => "cursor",
        }
    }

    /// The event name the agent's hook contract calls this trigger.
    pub fn hook_event_name(self) -> &'static str {
        match self {
            Agent::Claude | Agent::Codex => "PreToolUse",
            Agent::Cursor => "beforeShellExecution",
        }
    }

    /// Human hint for what to do after (re-)registering this agent's hook.
    pub fn restart_hint(self) -> &'static str {
        match self {
            Agent::Claude => "Restart Claude Code (or run /hooks) to pick up the new hook.",
            Agent::Codex => {
                "Restart Codex CLI to pick up the new hook (review/trust it via /hooks if prompted)."
            }
            Agent::Cursor => "Restart Cursor to pick up the new hook.",
        }
    }
}
