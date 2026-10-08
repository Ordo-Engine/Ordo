//! `ordo guard upgrade` — bring a scaffolded policy project up to the current
//! default.
//!
//! `guard init` never overwrites an existing `.ordo-guard/`, so a repo
//! scaffolded by an older CLI keeps its old default policy forever (the 1.0.0
//! policy matched substrings of `command`, which `rm -r -f` walks past). This
//! command replaces each file that is still an untouched copy of an older
//! default, and leaves files the user edited alone unless `--force` is given,
//! in which case the old file is kept as a `.bak` copy.

use anyhow::{bail, Context, Result};
use clap::Args;
use ordo_studio_format::StudioRuleSet;
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::init::{render_policy, GUARD_AGENTS_MD, POLICY_TESTS};

/// Policies earlier CLIs scaffolded, verbatim. Compared structurally, so
/// formatting differences don't matter.
const LEGACY_POLICIES: &[&str] = &[
    include_str!("legacy/policy-1.0.0.json"),
    include_str!("legacy/policy-1.0.0-multi-agent.json"),
];
const LEGACY_TESTS: &[&str] = &[include_str!("legacy/tests-1.0.0.json")];
const LEGACY_AGENTS_MD: &[&str] = &[
    include_str!("legacy/AGENTS-1.0.0.md"),
    include_str!("legacy/AGENTS-1.0.0-multi-agent.md"),
    include_str!("legacy/AGENTS-1.0.0-task-context.md"),
];

#[derive(Args)]
pub struct UpgradeArgs {
    /// Repo root (default: current directory)
    #[arg(default_value = ".")]
    dir: String,

    /// Guard policy project directory (default: <DIR>/.ordo-guard, else auto-discovered)
    #[arg(long, value_name = "DIR")]
    policy_dir: Option<String>,

    /// Also replace files you have edited (each is kept as a `.bak` copy)
    #[arg(long)]
    force: bool,

    /// Show what would change without writing anything
    #[arg(long)]
    dry_run: bool,
}

/// How a file on disk relates to the shipped defaults.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Current,
    Legacy,
    Custom,
    Missing,
}

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    UpToDate,
    Upgraded,
    Replaced,
    Created,
    Kept,
}

#[derive(serde::Serialize)]
struct FileResult {
    path: String,
    action: Action,
    #[serde(skip_serializing_if = "Option::is_none")]
    backup: Option<String>,
    detail: String,
}

struct ManagedFile {
    rel: &'static str,
    shipped: String,
    state: State,
}

pub fn run(args: UpgradeArgs, json: bool) -> Result<()> {
    let root = PathBuf::from(&args.dir);
    let guard_dir = match args.policy_dir.as_ref().map(PathBuf::from).or_else(|| {
        let local = root.join(super::POLICY_DIR_NAME);
        if local.join(crate::project::CONFIG_FILE).is_file() {
            Some(local)
        } else {
            super::resolve_policy_dir(None)
        }
    }) {
        Some(dir) => dir,
        None => bail!(
            "no {}/ policy project found — run `ordo guard init` first",
            super::POLICY_DIR_NAME
        ),
    };

    let policy = ManagedFile::load(
        &guard_dir,
        "rulesets/policy.json",
        render_policy()?,
        |text| policy_matches(text, LEGACY_POLICIES),
        |text, shipped| policy_matches(text, &[shipped]),
    )?;
    let tests = ManagedFile::load(
        &guard_dir,
        "tests/policy.json",
        POLICY_TESTS.to_string(),
        |text| json_matches(text, LEGACY_TESTS),
        |text, shipped| json_matches(text, &[shipped]),
    )?;
    let agents_md = ManagedFile::load(
        &guard_dir,
        "AGENTS.md",
        GUARD_AGENTS_MD.to_string(),
        |text| LEGACY_AGENTS_MD.iter().any(|l| l.trim() == text.trim()),
        |text, shipped| shipped.trim() == text.trim(),
    )?;

    let mut results = Vec::new();
    let policy_action = policy.apply(&guard_dir, args.force, args.dry_run, &mut results)?;
    // The shipped tests describe the shipped policy: only bring them along
    // when the policy is (or now becomes) the shipped one, so a kept custom
    // policy doesn't start failing its own test suite.
    if policy_action == Action::Kept {
        if tests.state != State::Current {
            results.push(FileResult {
                path: tests.rel.to_string(),
                action: Action::Kept,
                backup: None,
                detail: "left as is: it tests your customized policy".to_string(),
            });
        }
    } else {
        tests.apply(&guard_dir, args.force, args.dry_run, &mut results)?;
    }
    agents_md.apply(&guard_dir, args.force, args.dry_run, &mut results)?;

    let changed = results
        .iter()
        .any(|r| !matches!(r.action, Action::UpToDate | Action::Kept));
    if json {
        crate::output::emit_json(&serde_json::json!({
            "policy_dir": guard_dir.display().to_string(),
            "dry_run": args.dry_run,
            "changed": changed,
            "files": results,
        }))?;
    } else {
        let prefix = if args.dry_run { "would be " } else { "" };
        for r in &results {
            let verb = match r.action {
                Action::UpToDate => "up to date",
                Action::Upgraded => "upgraded",
                Action::Replaced => "replaced",
                Action::Created => "created",
                Action::Kept => "kept",
            };
            let verb = if matches!(r.action, Action::UpToDate | Action::Kept) {
                verb.to_string()
            } else {
                format!("{prefix}{verb}")
            };
            println!("{}: {verb} — {}", r.path, r.detail);
            if let Some(b) = &r.backup {
                println!("  previous version {prefix}saved as {b}");
            }
        }
        if changed && !args.dry_run {
            println!("\nNext: `ordo guard test` · `ordo guard doctor`");
        }
    }
    Ok(())
}

impl ManagedFile {
    fn load(
        guard_dir: &Path,
        rel: &'static str,
        shipped: String,
        is_legacy: impl Fn(&str) -> bool,
        is_current: impl Fn(&str, &str) -> bool,
    ) -> Result<Self> {
        let path = guard_dir.join(rel);
        let state = if !path.is_file() {
            State::Missing
        } else {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            if is_current(&text, &shipped) {
                State::Current
            } else if is_legacy(&text) {
                State::Legacy
            } else {
                State::Custom
            }
        };
        Ok(Self {
            rel,
            shipped,
            state,
        })
    }

    fn apply(
        &self,
        guard_dir: &Path,
        force: bool,
        dry_run: bool,
        results: &mut Vec<FileResult>,
    ) -> Result<Action> {
        let path = guard_dir.join(self.rel);
        let (action, detail) = match self.state {
            State::Current => (Action::UpToDate, "matches the current default".to_string()),
            State::Legacy => (
                Action::Upgraded,
                "was an unedited older default".to_string(),
            ),
            State::Missing => (Action::Created, "was missing".to_string()),
            State::Custom if force => (
                Action::Replaced,
                "had local edits, replaced (--force)".to_string(),
            ),
            State::Custom => (
                Action::Kept,
                "has local edits — rerun with --force to replace it (a .bak copy is kept)"
                    .to_string(),
            ),
        };
        let mut backup = None;
        if action == Action::Replaced {
            let bak = free_backup_path(&path);
            if !dry_run {
                std::fs::copy(&path, &bak)
                    .with_context(|| format!("failed to back up {}", path.display()))?;
            }
            backup = Some(display_rel(guard_dir, &bak));
        }
        if matches!(
            action,
            Action::Upgraded | Action::Replaced | Action::Created
        ) && !dry_run
        {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, &self.shipped)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        results.push(FileResult {
            path: self.rel.to_string(),
            action,
            backup,
            detail,
        });
        Ok(action)
    }
}

/// `<file>.bak`, or `<file>.bak.N` for the first N that doesn't exist yet —
/// never overwrite an earlier backup.
fn free_backup_path(path: &Path) -> PathBuf {
    let base = format!("{}.bak", path.display());
    let mut candidate = PathBuf::from(&base);
    let mut n = 1;
    while candidate.exists() {
        candidate = PathBuf::from(format!("{base}.{n}"));
        n += 1;
    }
    candidate
}

fn display_rel(guard_dir: &Path, path: &Path) -> String {
    path.strip_prefix(guard_dir)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Structural equality through the studio model, so key order, whitespace and
/// serializer defaults don't count as edits.
fn policy_matches(text: &str, candidates: &[&str]) -> bool {
    let Some(value) = canonical_policy(text) else {
        return false;
    };
    candidates
        .iter()
        .any(|c| canonical_policy(c).as_ref() == Some(&value))
}

fn canonical_policy(text: &str) -> Option<Value> {
    let studio: StudioRuleSet = serde_json::from_str(text).ok()?;
    serde_json::to_value(studio).ok()
}

fn json_matches(text: &str, candidates: &[&str]) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    candidates
        .iter()
        .any(|c| serde_json::from_str::<Value>(c).ok().as_ref() == Some(&value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_defaults_are_recognized_and_differ_from_current() {
        let current = render_policy().unwrap();
        for legacy in LEGACY_POLICIES {
            assert!(
                canonical_policy(legacy).is_some(),
                "legacy policy must parse"
            );
            assert!(policy_matches(legacy, LEGACY_POLICIES));
            assert!(!policy_matches(legacy, &[&current]));
        }
        for legacy in LEGACY_TESTS {
            assert!(!json_matches(legacy, &[POLICY_TESTS]));
        }
        for legacy in LEGACY_AGENTS_MD {
            assert_ne!(legacy.trim(), GUARD_AGENTS_MD.trim());
        }
    }

    #[test]
    fn reformatting_is_not_an_edit_but_a_changed_condition_is() {
        let value: Value = serde_json::from_str(LEGACY_POLICIES[0]).unwrap();
        let compact = serde_json::to_string(&value).unwrap();
        assert!(policy_matches(&compact, LEGACY_POLICIES));

        let edited = LEGACY_POLICIES[0].replacen("mkfs", "mkfs|wipefs", 1);
        assert_ne!(edited, LEGACY_POLICIES[0]);
        assert!(!policy_matches(&edited, LEGACY_POLICIES));
    }

    #[test]
    fn backups_never_overwrite_earlier_ones() {
        let dir = std::env::temp_dir().join(format!("ordo-upgrade-bak-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("policy.json");
        std::fs::write(&file, "x").unwrap();
        assert_eq!(free_backup_path(&file), dir.join("policy.json.bak"));
        std::fs::write(dir.join("policy.json.bak"), "y").unwrap();
        assert_eq!(free_backup_path(&file), dir.join("policy.json.bak.1"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
