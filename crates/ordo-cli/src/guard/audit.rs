//! Append-only JSONL audit log of guard decisions (`.ordo-guard/log.jsonl`).
//!
//! Logging must never break the hook: append failures degrade to a stderr
//! warning, and unparseable lines are skipped on read.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;

pub const LOG_FILE: &str = "log.jsonl";

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct AuditEntry {
    pub ts: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub tool: String,
    /// allow | deny | ask | pass | error
    pub decision: String,
    pub code: String,
    pub reason: String,
    pub duration_us: u64,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

pub(crate) fn append(policy_dir: &Path, entry: &AuditEntry) {
    let path = policy_dir.join(LOG_FILE);
    let result = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| {
            let line = serde_json::to_string(entry).unwrap_or_default();
            writeln!(f, "{line}")
        });
    if let Err(e) = result {
        eprintln!(
            "ordo guard: failed to append audit log {}: {e}",
            path.display()
        );
    }
}

pub(crate) fn read_tail(policy_dir: &Path, n: usize) -> Result<Vec<AuditEntry>> {
    let path = policy_dir.join(LOG_FILE);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let entries: Vec<AuditEntry> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let skip = entries.len().saturating_sub(n);
    Ok(entries.into_iter().skip(skip).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(tool: &str, decision: &str) -> AuditEntry {
        AuditEntry {
            ts: "2026-07-04T00:00:00.000Z".into(),
            session_id: None,
            tool: tool.into(),
            decision: decision.into(),
            code: "DENY".into(),
            reason: "r".into(),
            duration_us: 42,
            summary: "s".into(),
            cwd: None,
        }
    }

    #[test]
    fn append_and_tail_round_trip_skipping_corrupt_lines() {
        let dir = std::env::temp_dir().join(format!("ordo-guard-audit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        append(&dir, &entry("Bash", "deny"));
        append(&dir, &entry("Edit", "ask"));
        std::fs::write(
            dir.join(LOG_FILE),
            format!(
                "{}\nnot json at all\n{}\n",
                std::fs::read_to_string(dir.join(LOG_FILE))
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap(),
                serde_json::to_string(&entry("Read", "allow")).unwrap()
            ),
        )
        .unwrap();

        let all = read_tail(&dir, 10).unwrap();
        assert_eq!(all.len(), 2);
        let last = read_tail(&dir, 1).unwrap();
        assert_eq!(last[0].tool, "Read");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
