//! `ordo guard log` — show recent guard decisions from the audit log.

use anyhow::Result;
use clap::Args;
use colored::Colorize;

use super::audit;

#[derive(Args)]
pub struct LogArgs {
    /// Number of most-recent entries to show
    #[arg(long, default_value_t = 20)]
    tail: usize,

    /// Guard policy project directory (default: auto-discovered)
    #[arg(long, value_name = "DIR")]
    policy_dir: Option<String>,
}

pub fn run(args: LogArgs, json: bool) -> Result<()> {
    let policy_dir = super::resolve_policy_dir(args.policy_dir.as_deref()).ok_or_else(|| {
        anyhow::anyhow!(
            "no guard policy found (looked for {}/{}) — run `ordo guard init`",
            super::POLICY_DIR_NAME,
            crate::project::CONFIG_FILE
        )
    })?;
    let entries = audit::read_tail(&policy_dir, args.tail)?;

    if json {
        return crate::output::emit_json(&serde_json::json!(entries));
    }
    if entries.is_empty() {
        println!(
            "No guard decisions logged yet ({})",
            policy_dir.join(audit::LOG_FILE).display()
        );
        return Ok(());
    }
    for e in &entries {
        let decision = match e.decision.as_str() {
            "deny" => e.decision.red().bold(),
            "ask" => e.decision.yellow().bold(),
            "allow" => e.decision.green().bold(),
            "error" => e.decision.magenta().bold(),
            _ => e.decision.dimmed(),
        };
        let ms = e.duration_us as f64 / 1000.0;
        println!(
            "{}  {:5}  {:8}  {}",
            e.ts.dimmed(),
            decision,
            e.tool,
            e.summary
        );
        if e.decision != "pass" {
            println!("        {} ({ms:.1}ms)", e.reason.dimmed());
        }
    }
    Ok(())
}
