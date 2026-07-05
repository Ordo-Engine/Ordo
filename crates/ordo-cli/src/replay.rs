//! `ordo replay <captured.jsonl>` — replay recorded decisions against the
//! current local ruleset.
//!
//! Each JSONL line is a captured decision (`{rule_name, input, code, output}` —
//! the shape ordo-server's `--capture-io` writes, and also what a business app's
//! own decision log looks like). Replay re-runs every `input` through the
//! project's current ruleset and reports which decisions stayed the same, which
//! **flipped** (code/output changed vs. the captured baseline), and which
//! errored — the safety net for changing a rule. `--write-tests` fixates the
//! captured decisions as `tests/<rule>.json` regression cases.

use std::collections::HashMap;
use std::io::{IsTerminal, Read};

use anyhow::{Context, Result};
use clap::Args;
use ordo_core::prelude::Value;
use serde::Deserialize;

use crate::project::{ruleset_name, Project};
use crate::runtime::{execute_loaded_rule, LoadedRule};

#[derive(Args)]
pub struct ReplayArgs {
    /// Captured decisions as JSONL (a file path, or `-` to read stdin)
    source: String,

    /// Replay every record against THIS ruleset, ignoring each record's rule_name
    #[arg(long, value_name = "NAME")]
    ruleset: Option<String>,

    /// Fixate the captured decisions as tests/<rule>.json regression cases
    #[arg(long)]
    write_tests: bool,

    /// Exit non-zero if any decision flipped (for CI gating; default: report only)
    #[arg(long)]
    fail_on_flip: bool,
}

/// One captured decision. Only `input` is required; everything else is optional
/// so a record written by an older/newer producer still replays.
#[derive(Deserialize)]
struct CapturedRecord {
    #[serde(default)]
    rule_name: Option<String>,
    input: Value,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    output: Option<Value>,
}

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Consistent,
    Flipped,
    Errored,
    Unknown,
    Replayed,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Consistent => "consistent",
            Status::Flipped => "flipped",
            Status::Errored => "errored",
            Status::Unknown => "unknown_ruleset",
            Status::Replayed => "replayed",
        }
    }
}

struct RecordResult {
    rule: String,
    status: Status,
    old_code: Option<String>,
    new_code: Option<String>,
    diffs: Vec<String>,
    summary: String,
}

/// A ruleset resolved (or not) from the project, compiled once and reused.
enum Resolved {
    Ready(Box<LoadedRule>),
    NotFound,
    LoadError(String),
}

pub fn run(args: ReplayArgs, json: bool) -> Result<()> {
    let text = read_source(&args.source)?;

    // Tolerant JSONL parse — skip blank/garbage lines, count them.
    let mut records: Vec<CapturedRecord> = Vec::new();
    let mut skipped = 0usize;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<CapturedRecord>(line) {
            Ok(rec) => records.push(rec),
            Err(_) => skipped += 1,
        }
    }

    let project = Project::discover(None)?;
    let known: std::collections::HashSet<String> = project.ruleset_names()?.into_iter().collect();

    let mut cache: HashMap<String, Resolved> = HashMap::new();
    let mut results: Vec<RecordResult> = Vec::new();
    // Writable captured cases per ruleset (only records that carry a code).
    let mut writable: HashMap<String, Vec<CapturedRecord>> = HashMap::new();

    for rec in records {
        let name = match args.ruleset.as_deref().or(rec.rule_name.as_deref()) {
            Some(n) => ruleset_name(n),
            None => {
                results.push(RecordResult {
                    rule: String::new(),
                    status: Status::Unknown,
                    old_code: rec.code.clone(),
                    new_code: None,
                    diffs: vec!["record has no rule_name and --ruleset was not given".into()],
                    summary: summarize(&rec.input),
                });
                continue;
            }
        };

        let resolved = cache
            .entry(name.clone())
            .or_insert_with(|| resolve(&project, &known, &name));

        let summary = summarize(&rec.input);
        let result = match resolved {
            Resolved::NotFound => {
                results.push(RecordResult {
                    rule: name,
                    status: Status::Unknown,
                    old_code: rec.code.clone(),
                    new_code: None,
                    diffs: vec!["no ruleset in this project".to_string()],
                    summary,
                });
                continue;
            }
            Resolved::LoadError(e) => {
                results.push(RecordResult {
                    rule: name,
                    status: Status::Errored,
                    old_code: rec.code.clone(),
                    new_code: None,
                    diffs: vec![format!("load error: {e}")],
                    summary,
                });
                continue;
            }
            Resolved::Ready(loaded) => execute_loaded_rule(loaded, rec.input.clone(), false),
        };

        match result {
            Err(e) => results.push(RecordResult {
                rule: name,
                status: Status::Errored,
                old_code: rec.code.clone(),
                new_code: None,
                diffs: vec![format!("execution error: {e}")],
                summary,
            }),
            Ok(exec) => {
                if args.write_tests && rec.code.is_some() {
                    writable
                        .entry(name.clone())
                        .or_default()
                        .push(CapturedRecord {
                            rule_name: Some(name.clone()),
                            input: rec.input.clone(),
                            code: rec.code.clone(),
                            output: rec.output.clone(),
                        });
                }
                let (status, diffs) = match &rec.code {
                    None => (Status::Replayed, Vec::new()),
                    Some(_) => {
                        let diffs = crate::test_runner::diff_result(
                            rec.code.clone(),
                            rec.output.clone(),
                            &exec,
                        );
                        if diffs.is_empty() {
                            (Status::Consistent, diffs)
                        } else {
                            (Status::Flipped, diffs)
                        }
                    }
                };
                results.push(RecordResult {
                    rule: name,
                    status,
                    old_code: rec.code.clone(),
                    new_code: Some(exec.code),
                    diffs,
                    summary,
                });
            }
        }
    }

    let written = if args.write_tests {
        write_tests(&project, &writable)?
    } else {
        Vec::new()
    };

    let flipped = results
        .iter()
        .filter(|r| r.status == Status::Flipped)
        .count();
    report(&results, skipped, &written, json);

    if args.fail_on_flip && flipped > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn resolve(project: &Project, known: &std::collections::HashSet<String>, name: &str) -> Resolved {
    if !known.contains(name) {
        return Resolved::NotFound;
    }
    match project.load_engine(name) {
        Ok(mut engine) => match engine.compile() {
            Ok(()) => Resolved::Ready(Box::new(LoadedRule::Source(engine))),
            Err(e) => Resolved::LoadError(format!("compile error: {e}")),
        },
        Err(e) => Resolved::LoadError(format!("{e:#}")),
    }
}

fn read_source(source: &str) -> Result<String> {
    if source == "-" {
        if std::io::stdin().is_terminal() {
            anyhow::bail!("`ordo replay -` reads JSONL on stdin, but stdin is a terminal");
        }
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("failed to read captured decisions from stdin")?;
        Ok(buf)
    } else {
        std::fs::read_to_string(source)
            .with_context(|| format!("failed to read captured decisions from {source}"))
    }
}

/// Merge writable captured cases into tests/<rule>.json, deduping by identical
/// input. Returns per-file "<path> (+N)" summaries. Reuses the read-merge-write
/// shape of `new.rs`, building JSON with `json!` since `TestCase` is not
/// `Serialize`.
fn write_tests(
    project: &Project,
    writable: &HashMap<String, Vec<CapturedRecord>>,
) -> Result<Vec<String>> {
    let mut written = Vec::new();
    for (name, recs) in writable {
        let path = project.tests_path(name);
        let mut items: Vec<serde_json::Value> = if path.is_file() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            if text.trim().is_empty() {
                Vec::new()
            } else {
                serde_json::from_str(&text)
                    .with_context(|| format!("invalid tests file {}", path.display()))?
            }
        } else {
            Vec::new()
        };

        let existing_inputs: Vec<serde_json::Value> = items
            .iter()
            .filter_map(|it| it.get("input").cloned())
            .collect();

        let base = items.len();
        let mut added = 0usize;
        for rec in recs {
            let input_json = serde_json::to_value(&rec.input)?;
            if existing_inputs.contains(&input_json) {
                continue;
            }
            let mut expect = serde_json::Map::new();
            if let Some(code) = &rec.code {
                expect.insert("code".into(), serde_json::Value::String(code.clone()));
            }
            if let Some(out) = &rec.output {
                expect.insert("output".into(), serde_json::to_value(out)?);
            }
            items.push(serde_json::json!({
                "name": format!("replay-{}", base + added + 1),
                "input": input_json,
                "expect": serde_json::Value::Object(expect),
            }));
            added += 1;
        }

        if added > 0 {
            std::fs::create_dir_all(project.tests_dir())?;
            std::fs::write(
                &path,
                format!("{}\n", serde_json::to_string_pretty(&items)?),
            )?;
        }
        written.push(format!("tests/{name}.json (+{added})"));
    }
    written.sort();
    Ok(written)
}

/// A short, single-line description of a record's input for the report.
fn summarize(input: &Value) -> String {
    let compact = serde_json::to_value(input)
        .ok()
        .map(|v| v.to_string())
        .unwrap_or_default();
    let mut s: String = compact.chars().take(80).collect();
    if compact.chars().count() > 80 {
        s.push('…');
    }
    s
}

fn report(results: &[RecordResult], skipped: usize, written: &[String], json: bool) {
    use colored::Colorize;

    let count = |s: Status| results.iter().filter(|r| r.status == s).count();
    let consistent = count(Status::Consistent);
    let flipped = count(Status::Flipped);
    let errored = count(Status::Errored);
    let unknown = count(Status::Unknown);
    let replayed = count(Status::Replayed);

    if json {
        let records: Vec<_> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "rule": r.rule,
                    "status": r.status.as_str(),
                    "old_code": r.old_code,
                    "new_code": r.new_code,
                    "diffs": r.diffs,
                    "summary": r.summary,
                })
            })
            .collect();
        let _ = crate::output::emit_json(&serde_json::json!({
            "total": results.len(),
            "consistent": consistent,
            "flipped": flipped,
            "errored": errored,
            "unknown_ruleset": unknown,
            "replayed": replayed,
            "skipped": skipped,
            "written_tests": written,
            "records": records,
        }));
        return;
    }

    // Human: show flips + errors (the interesting ones) with their diffs.
    for r in results {
        match r.status {
            Status::Flipped => {
                let old = r.old_code.as_deref().unwrap_or("?");
                let new = r.new_code.as_deref().unwrap_or("?");
                println!(
                    "{} {}  {}  {} {} {}",
                    "FLIP".yellow().bold(),
                    r.rule,
                    r.summary.dimmed(),
                    old.red(),
                    "→".dimmed(),
                    new.green()
                );
                for d in &r.diffs {
                    println!("     {}", d.dimmed());
                }
            }
            Status::Errored | Status::Unknown => {
                println!(
                    "{} {}  {}",
                    r.status.as_str().to_uppercase().red().bold(),
                    r.rule,
                    r.summary.dimmed()
                );
                for d in &r.diffs {
                    println!("     {}", d.dimmed());
                }
            }
            _ => {}
        }
    }

    println!();
    let mut parts = vec![format!("{} consistent", consistent.to_string().green())];
    if flipped > 0 {
        parts.push(format!("{} flipped", flipped.to_string().yellow()));
    }
    if errored > 0 {
        parts.push(format!("{} errored", errored.to_string().red()));
    }
    if unknown > 0 {
        parts.push(format!("{unknown} unknown-ruleset"));
    }
    if replayed > 0 {
        parts.push(format!("{replayed} replayed"));
    }
    println!("{} records: {}", results.len(), parts.join(" · "));
    if skipped > 0 {
        println!("({skipped} unparseable lines skipped)");
    }
    if !written.is_empty() {
        println!("wrote regression tests: {}", written.join(", "));
    }
}
