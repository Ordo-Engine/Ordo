//! `ordo decisions <log>` — what did the gateway decide?
//!
//! Reads the decision log written by `ordo-server --decide-log-dir` (one file
//! or the whole directory) and shows how many calls ended in each decision,
//! which models answered, and the latest decisions with the answers they were
//! based on.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use serde_json::{json, Value as Json};

#[derive(Args)]
pub struct DecisionsArgs {
    /// Decision log file, or the --decide-log-dir directory
    path: String,

    /// Only decisions made by this ruleset
    #[arg(long)]
    ruleset: Option<String>,

    /// Only decisions with this code (e.g. AUTO_REFUND)
    #[arg(long)]
    code: Option<String>,

    /// How many of the latest decisions to list
    #[arg(long, default_value_t = 10)]
    last: usize,
}

#[derive(Serialize)]
struct Summary {
    files: Vec<String>,
    /// Gateway calls read (after --ruleset / --code)
    calls: usize,
    /// Calls the model did not answer (upstream error)
    upstream_errors: usize,
    /// Calls where the rules failed after the model answered
    rule_errors: usize,
    /// Calls answered without a ruleset
    no_ruleset: usize,
    models: BTreeMap<String, usize>,
    /// ruleset → code → count
    codes: BTreeMap<String, BTreeMap<String, usize>>,
    /// The latest decisions, newest first
    latest: Vec<Json>,
}

pub fn run(args: DecisionsArgs, json_out: bool) -> Result<()> {
    let files = log_files(Path::new(&args.path))?;
    let mut records = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f)
            .with_context(|| format!("failed to read {}", f.display()))?;
        for (i, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let v: Json = serde_json::from_str(line)
                .with_context(|| format!("{}:{}: invalid JSON", f.display(), i + 1))?;
            records.push(v);
        }
    }
    let records: Vec<Json> = records
        .into_iter()
        .filter(|r| {
            args.ruleset
                .as_deref()
                .map_or(true, |want| str_of(r, "ruleset") == Some(want))
                && args
                    .code
                    .as_deref()
                    .map_or(true, |want| str_of(r, "code") == Some(want))
        })
        .collect();
    let summary = summarize(&files, &records, args.last);
    if json_out {
        crate::output::emit_json(&summary)?;
    } else {
        print_summary(&summary);
    }
    Ok(())
}

/// The file itself, or every `decisions-*.jsonl` in the directory, oldest first.
fn log_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(path)
        .with_context(|| format!("failed to read {}", path.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("decisions-") && n.ends_with(".jsonl"))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        anyhow::bail!(
            "no decisions-*.jsonl in {} (ordo-server writes them with --decide-log-dir)",
            path.display()
        );
    }
    Ok(files)
}

fn str_of<'a>(v: &'a Json, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Json::as_str)
}

fn summarize(files: &[PathBuf], records: &[Json], last: usize) -> Summary {
    let mut s = Summary {
        files: files.iter().map(|f| f.display().to_string()).collect(),
        calls: records.len(),
        upstream_errors: 0,
        rule_errors: 0,
        no_ruleset: 0,
        models: BTreeMap::new(),
        codes: BTreeMap::new(),
        latest: Vec::new(),
    };
    for r in records {
        let answered = r.get("answers").is_some_and(|a| !a.is_null());
        if !answered {
            s.upstream_errors += 1;
            continue;
        }
        *s.models
            .entry(str_of(r, "model").unwrap_or("?").to_string())
            .or_default() += 1;
        match (str_of(r, "ruleset"), str_of(r, "code")) {
            (None, _) => s.no_ruleset += 1,
            (Some(_), None) => s.rule_errors += 1,
            (Some(rs), Some(code)) => {
                *s.codes
                    .entry(rs.to_string())
                    .or_default()
                    .entry(code.to_string())
                    .or_default() += 1
            }
        }
    }
    // Lines are appended in time order; newest first.
    s.latest = records
        .iter()
        .rev()
        .take(last)
        .map(|r| {
            json!({
                "ts": r.get("ts"),
                "decision_id": r.get("decision_id"),
                "model": r.get("model"),
                "ruleset": r.get("ruleset"),
                "code": r.get("code"),
                "error": r.get("error"),
                "answers": r.get("answers").map(crate::remodel::verdict),
                "output": r.get("output"),
                "state": r.get("request").and_then(|q| q.get("state")),
            })
        })
        .collect();
    s
}

/// `chargeback=0.04, intent=refund 0.97, urgency=0.48`
fn answers_line(answers: &Json) -> String {
    let Some(map) = answers.as_object() else {
        return String::new();
    };
    let num = |v: &Json| {
        v.as_f64()
            .map(|n| format!("{}", (n * 100.0).round() / 100.0))
            .unwrap_or_else(|| v.to_string())
    };
    map.iter()
        .map(|(q, a)| {
            let verdict = if let Some(c) = a.get("choice") {
                let c = c
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| c.to_string());
                match a.get("confidence") {
                    Some(p) => format!("{c} {}", num(p)),
                    None => c,
                }
            } else if let Some(v) = a.get("score").or_else(|| a.get("noul")) {
                num(v)
            } else {
                a.to_string()
            };
            format!("{q}={verdict}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_summary(s: &Summary) {
    use colored::Colorize;

    println!(
        "{} {} call{} in {} file{}",
        "decisions".bold(),
        s.calls,
        if s.calls == 1 { "" } else { "s" },
        s.files.len(),
        if s.files.len() == 1 { "" } else { "s" }
    );
    for (model, n) in &s.models {
        println!("  model {model}: {n}");
    }
    if s.upstream_errors > 0 {
        println!(
            "  {}",
            format!("{} not answered (upstream error)", s.upstream_errors).red()
        );
    }
    if s.rule_errors > 0 {
        println!("  {}", format!("{} rule errors", s.rule_errors).red());
    }
    if s.no_ruleset > 0 {
        println!("  {} answered without a ruleset", s.no_ruleset);
    }
    for (ruleset, codes) in &s.codes {
        let total: usize = codes.values().sum();
        println!();
        println!("  {}", ruleset.bold());
        let mut sorted: Vec<_> = codes.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (code, n) in sorted {
            println!(
                "    {:<16} {:>5}  {:>3}%",
                code,
                n,
                (*n as f64 * 100.0 / total as f64).round()
            );
        }
    }
    if s.latest.is_empty() {
        return;
    }
    println!();
    println!("  latest:");
    for r in &s.latest {
        let ts = r["ts"].as_str().unwrap_or("").get(..19).unwrap_or("");
        let code = match (r["code"].as_str(), r["error"].as_str()) {
            (Some(c), _) => c.yellow().bold().to_string(),
            (None, Some(_)) => "error".red().to_string(),
            (None, None) => "-".to_string(),
        };
        println!(
            "  {} {}  {}",
            ts.dimmed(),
            code,
            answers_line(&r["answers"])
        );
        if let Some(state) = r["state"].as_str() {
            let one: String = state.split_whitespace().collect::<Vec<_>>().join(" ");
            let cut: String = one.chars().take(100).collect();
            let more = if one.chars().count() > 100 { "…" } else { "" };
            println!("      {}", format!("\"{cut}{more}\"").dimmed());
        }
        if let Some(e) = r["error"].as_str() {
            println!("      {}", e.red());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_codes_models_and_errors() {
        let records = vec![
            json!({"model":"jev-1.13.0","answers":{"intent":{"choice":"refund","confidence":1}},"ruleset":"r","code":"AUTO_REFUND"}),
            json!({"model":"jev-1.13.0","answers":{"intent":{"choice":"refund","confidence":0.7}},"ruleset":"r","code":"LLM_DRAFT"}),
            json!({"model":"jev-1.13.0","answers":{"intent":{"choice":"refund","confidence":1}},"ruleset":"r","code":"AUTO_REFUND"}),
            json!({"model":"jev-1.13.0","answers":{},"ruleset":"r","error":"missing field"}),
            json!({"model":"jev-1.13","input":null,"upstream_status":429}),
            json!({"model":"clef","answers":{},"input":null}),
        ];
        let s = summarize(&[], &records, 2);
        assert_eq!(s.calls, 6);
        assert_eq!(s.upstream_errors, 1);
        assert_eq!(s.rule_errors, 1);
        assert_eq!(s.no_ruleset, 1);
        assert_eq!(s.models["jev-1.13.0"], 4);
        assert_eq!(s.codes["r"]["AUTO_REFUND"], 2);
        assert_eq!(s.codes["r"]["LLM_DRAFT"], 1);
        assert_eq!(s.latest.len(), 2);
        assert_eq!(s.latest[0]["model"], "clef");
    }

    #[test]
    fn answers_line_reads_choice_score_and_noul() {
        let a = json!({
            "intent": {"choice": "refund", "confidence": 0.973},
            "urgency": {"score": 0.48},
            "chargeback": {"noul": 0.04}
        });
        assert_eq!(
            answers_line(&a),
            "chargeback=0.04, intent=refund 0.97, urgency=0.48"
        );
    }
}
