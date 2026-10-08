//! `ordo impact <ruleset>` — what did my edit change about the decisions?
//!
//! Runs the same inputs through two versions of a ruleset — a baseline (a git
//! revision, `HEAD` by default, or a file) and the working tree — and reports
//! every input whose decision changed: code, message, or output fields.
//!
//! The inputs are the ruleset's test cases, any captured decisions passed with
//! `--inputs`, and boundary probes: for every comparison between an input field
//! and a literal in either version (`amount <= 10000`, `tier == "gold"`, a
//! decision-table cell), the test inputs are re-run with that field set just
//! below, at, and just above the literal. A moved threshold therefore shows up
//! even when no test case sits near it. This is the check a coding agent runs
//! after editing a rule, before it says "done".

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use clap::Args;
use ordo_core::prelude::{BinaryOp, Condition, Expr, RuleSet, StepKind, Value};
use serde::Serialize;
use serde_json::{json, Value as Json};

use crate::project::{ruleset_name, Project};
use crate::runtime::{execute_loaded_rule, LoadedRule};

/// Probes are derived per (threshold × seed); cap the total so a large table
/// stays sub-second.
const MAX_PROBES: usize = 2000;
/// The baseline side of every input when the ruleset is new.
const NEW: &str = "(new)";
/// Changed inputs listed individually in the human report (all of them in --json).
const MAX_LISTED: usize = 20;

#[derive(Args)]
pub struct ImpactArgs {
    /// Ruleset name (or rulesets/<name>.json)
    ruleset: String,

    /// Baseline git revision (default: HEAD)
    #[arg(long, value_name = "REV", conflicts_with = "base_file")]
    base: Option<String>,

    /// Baseline ruleset file instead of a git revision
    #[arg(long, value_name = "FILE")]
    base_file: Option<String>,

    /// Extra inputs: JSONL of captured decisions (`{input, ...}`) or bare input objects
    #[arg(long, value_name = "FILE")]
    inputs: Option<String>,

    /// Skip boundary probes; compare only tests and --inputs
    #[arg(long)]
    no_probes: bool,

    /// Exit non-zero if any decision changed (for CI gating; default: report only)
    #[arg(long)]
    fail_on_change: bool,
}

/// Where the baseline comes from.
pub(crate) enum Base {
    Rev(String),
    File(String),
}

impl Base {
    fn label(&self) -> String {
        match self {
            Base::Rev(r) => r.clone(),
            Base::File(f) => f.clone(),
        }
    }
}

#[derive(Serialize, Clone)]
pub(crate) struct Outcome {
    code: String,
    message: String,
    output: Json,
}

/// One version's decision for an input, or why it has none.
type Side = std::result::Result<Outcome, String>;

/// `{code, message, output}` on success, `{error}` otherwise.
fn ser_side<S: serde::Serializer>(side: &Side, s: S) -> std::result::Result<S::Ok, S::Error> {
    match side {
        Ok(o) => o.serialize(s),
        Err(e) => json!({ "error": e }).serialize(s),
    }
}

#[derive(Serialize)]
pub(crate) struct Change {
    /// Where the input came from: `test:<name>`, `input:<line>`, or `probe:<field>=<value>`
    source: String,
    input: Json,
    #[serde(serialize_with = "ser_side")]
    before: Side,
    #[serde(serialize_with = "ser_side")]
    after: Side,
    /// Human-readable field-level differences
    diffs: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct Report {
    ruleset: String,
    base: String,
    /// `false` when the ruleset does not exist at the baseline (new ruleset)
    base_found: bool,
    inputs: usize,
    tests: usize,
    captured: usize,
    probes: usize,
    unchanged: usize,
    changed: usize,
    /// `code transitions` → count, e.g. `"REJECTED → APPROVED": 3`
    transitions: BTreeMap<String, usize>,
    changes: Vec<Change>,
}

pub fn run(args: ImpactArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let name = ruleset_name(&args.ruleset);
    let base = match args.base_file {
        Some(f) => Base::File(f),
        None => Base::Rev(args.base.unwrap_or_else(|| "HEAD".to_string())),
    };
    let extra = match &args.inputs {
        Some(path) => read_inputs(Path::new(path))?,
        None => Vec::new(),
    };
    let report = compute(&project, &name, &base, extra, !args.no_probes)?;
    let changed = report.changed;
    if json {
        crate::output::emit_json(&report)?;
    } else {
        print_report(&report);
    }
    if args.fail_on_change && changed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Compute the behavior diff for one ruleset (shared with `ordo mcp`).
pub(crate) fn compute(
    project: &Project,
    name: &str,
    base: &Base,
    extra: Vec<Json>,
    probes: bool,
) -> Result<Report> {
    let mut after_rs = project.load_engine(name)?;
    after_rs
        .compile()
        .map_err(|e| anyhow::anyhow!("compile error in {name} (working tree): {e}"))?;

    let before_rs = match load_base(project, name, base)? {
        Some(mut rs) => {
            rs.compile()
                .map_err(|e| anyhow::anyhow!("compile error in {name} at {}: {e}", base.label()))?;
            Some(rs)
        }
        None => None,
    };

    // ── corpus: tests, captured inputs, boundary probes ──
    let mut corpus: Vec<(String, Json)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut push = |corpus: &mut Vec<(String, Json)>, source: String, input: Json| {
        if seen.insert(input.to_string()) {
            corpus.push((source, input));
        }
    };

    let tests = crate::project::read_json_array(&project.tests_path(name))?.unwrap_or_default();
    let mut seeds: Vec<Json> = Vec::new();
    for (i, case) in tests.iter().enumerate() {
        let Some(input) = case.get("input") else {
            continue;
        };
        let label = case
            .get("name")
            .and_then(|n| n.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("#{}", i + 1));
        seeds.push(input.clone());
        push(&mut corpus, format!("test:{label}"), input.clone());
    }
    let n_tests = corpus.len();

    for (i, input) in extra.into_iter().enumerate() {
        seeds.push(input.clone());
        push(&mut corpus, format!("input:{}", i + 1), input);
    }
    let n_captured = corpus.len() - n_tests;

    if probes {
        let mut thresholds = BTreeSet::new();
        collect_thresholds(&after_rs, &mut thresholds);
        if let Some(rs) = &before_rs {
            collect_thresholds(rs, &mut thresholds);
        }
        if seeds.is_empty() {
            seeds.push(json!({}));
        }
        let mut made = 0usize;
        'outer: for t in &thresholds {
            for value in t.candidates() {
                for seed in &seeds {
                    if made >= MAX_PROBES {
                        break 'outer;
                    }
                    let mut input = seed.clone();
                    if set_path(&mut input, &t.field, value.clone()) {
                        made += 1;
                        push(&mut corpus, format!("probe:{}={}", t.field, value), input);
                    }
                }
            }
        }
    }
    let n_probes = corpus.len() - n_tests - n_captured;

    // ── run both versions ──
    let after = LoadedRule::Source(after_rs);
    let before = before_rs.map(LoadedRule::Source);
    let mut changes = Vec::new();
    let mut transitions: BTreeMap<String, usize> = BTreeMap::new();
    let mut unchanged = 0usize;

    for (source, input) in &corpus {
        let after_out = run_one(&after, input);
        let before_out = match &before {
            Some(rule) => run_one(rule, input),
            None => Err(NEW.to_string()),
        };
        let diffs = diff_outcomes(&before_out, &after_out);
        if diffs.is_empty() {
            unchanged += 1;
            continue;
        }
        *transitions
            .entry(format!(
                "{} → {}",
                code_label(&before_out),
                code_label(&after_out)
            ))
            .or_default() += 1;
        changes.push(Change {
            source: source.clone(),
            input: input.clone(),
            before: before_out,
            after: after_out,
            diffs,
        });
    }

    Ok(Report {
        ruleset: name.to_string(),
        base: base.label(),
        base_found: before.is_some(),
        inputs: corpus.len(),
        tests: n_tests,
        captured: n_captured,
        probes: n_probes,
        unchanged,
        changed: changes.len(),
        transitions,
        changes,
    })
}

/// Load the baseline ruleset. `Ok(None)` when it does not exist there (a new
/// ruleset, or a project not under git yet).
fn load_base(project: &Project, name: &str, base: &Base) -> Result<Option<RuleSet>> {
    let (text, concepts) = match base {
        Base::File(path) => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read baseline {path}"))?;
            (text, project.load_concepts()?)
        }
        Base::Rev(rev) => {
            let Some(text) = git_show(&project.root, rev, &format!("rulesets/{name}.json"))? else {
                return Ok(None);
            };
            // Concepts at the same revision, so a concept edit counts as a change.
            let concepts = match git_show(&project.root, rev, "concepts.json")? {
                Some(c) if !c.trim().is_empty() => serde_json::from_str(&c)
                    .with_context(|| format!("invalid concepts.json at {rev}"))?,
                Some(_) => Vec::new(),
                None => project.load_concepts()?,
            };
            (text, concepts)
        }
    };
    crate::project::engine_from_text(name, &text, &concepts).map(Some)
}

/// `git show <rev>:./<rel>` from the project root. `Ok(None)` when the
/// project is not in a git repo or the file does not exist at that revision;
/// an unknown revision is an error.
fn git_show(root: &Path, rev: &str, rel: &str) -> Result<Option<String>> {
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .context("failed to run git (use --base-file to compare without git)")
    };
    if !git(&["rev-parse", "--is-inside-work-tree"])?
        .status
        .success()
    {
        return Ok(None);
    }
    if !git(&[
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("{rev}^{{commit}}"),
    ])?
    .status
    .success()
    {
        anyhow::bail!("unknown git revision '{rev}'");
    }
    let out = git(&["show", &format!("{rev}:./{rel}")])?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// One line per input: a captured record's `input`, or the object itself.
fn read_inputs(path: &Path) -> Result<Vec<Json>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read inputs from {}", path.display()))?;
    let mut inputs = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: Json = serde_json::from_str(line)
            .with_context(|| format!("{}:{}: invalid JSON", path.display(), i + 1))?;
        inputs.push(match v.get("input") {
            Some(input) => input.clone(),
            None => v,
        });
    }
    Ok(inputs)
}

fn run_one(rule: &LoadedRule, input: &Json) -> Side {
    let value: Value = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    match execute_loaded_rule(rule, value, false) {
        Ok(r) => Ok(Outcome {
            code: r.code,
            message: r.message,
            output: serde_json::to_value(&r.output).unwrap_or(Json::Null),
        }),
        Err(e) => Err(format!("{e:#}")),
    }
}

fn code_label(o: &Side) -> String {
    match o {
        Ok(o) => o.code.clone(),
        Err(e) if e == NEW => NEW.to_string(),
        Err(_) => "error".to_string(),
    }
}

fn diff_outcomes(before: &Side, after: &Side) -> Vec<String> {
    match (before, after) {
        (Ok(b), Ok(a)) => {
            let mut diffs = Vec::new();
            if b.code != a.code {
                diffs.push(format!("code: {} → {}", b.code, a.code));
            }
            if b.message != a.message {
                diffs.push(format!("message: {:?} → {:?}", b.message, a.message));
            }
            diff_json("output", &b.output, &a.output, &mut diffs);
            diffs
        }
        (Err(b), Err(a)) if b == a => Vec::new(),
        (Err(b), Err(a)) => vec![format!("error: {b} → {a}")],
        (Err(b), Ok(a)) if b == NEW => vec![format!("new: {}", a.code)],
        (Err(b), Ok(a)) => vec![format!("was error ({b}), now {}", a.code)],
        (Ok(b), Err(a)) => vec![format!("was {}, now error: {a}", b.code)],
    }
}

/// Field-level diff of two JSON values, recursing into objects.
fn diff_json(path: &str, before: &Json, after: &Json, out: &mut Vec<String>) {
    if before == after {
        return;
    }
    if let (Json::Object(b), Json::Object(a)) = (before, after) {
        let keys: BTreeSet<&String> = b.keys().chain(a.keys()).collect();
        for k in keys {
            let p = format!("{path}.{k}");
            match (b.get(k), a.get(k)) {
                (Some(bv), Some(av)) => diff_json(&p, bv, av, out),
                (Some(bv), None) => out.push(format!("{p}: {bv} → (removed)")),
                (None, Some(av)) => out.push(format!("{p}: (added) → {av}")),
                (None, None) => {}
            }
        }
        return;
    }
    // Numerically equal (10 vs 10.0) is not a behavior change.
    if let (Some(b), Some(a)) = (before.as_f64(), after.as_f64()) {
        if b == a {
            return;
        }
    }
    out.push(format!("{path}: {before} → {after}"));
}

// ── boundary probes ──

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Literal {
    /// Stored as its JSON text so the set is ordered and deduplicated.
    Number(String),
    Str(String),
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Threshold {
    field: String,
    literal: Literal,
}

impl Threshold {
    /// Values to probe: just below, at, and just above a number; the string
    /// itself plus a value no rule mentions.
    fn candidates(&self) -> Vec<Json> {
        match &self.literal {
            Literal::Number(text) => {
                let Ok(n) = text.parse::<f64>() else {
                    return Vec::new();
                };
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    let n = n as i64;
                    vec![json!(n - 1), json!(n), json!(n + 1)]
                } else {
                    vec![json!(n - 0.01), json!(n), json!(n + 0.01)]
                }
            }
            Literal::Str(s) => vec![json!(s), json!("__other__")],
        }
    }
}

fn collect_thresholds(rs: &RuleSet, out: &mut BTreeSet<Threshold>) {
    let steps = rs
        .steps
        .values()
        .chain(rs.sub_rules.values().flat_map(|g| g.steps.values()));
    for step in steps {
        match &step.kind {
            StepKind::Decision { branches, .. } => {
                for b in branches {
                    if let Condition::Expression(e) = &b.condition {
                        walk(e, out);
                    }
                }
            }
            StepKind::DecisionTable(table) => {
                for e in table.expressions() {
                    walk(e, out);
                }
            }
            _ => {}
        }
    }
}

fn walk(e: &Expr, out: &mut BTreeSet<Threshold>) {
    match e {
        Expr::Binary { op, left, right } => {
            if is_comparison(*op) {
                note(left, right, out);
                note(right, left, out);
            }
            walk(left, out);
            walk(right, out);
        }
        Expr::Unary { operand, .. } => walk(operand, out),
        Expr::Call { args, .. } => args.iter().for_each(|a| walk(a, out)),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            walk(condition, out);
            walk(then_branch, out);
            walk(else_branch, out);
        }
        Expr::Array(items) | Expr::Coalesce(items) => items.iter().for_each(|a| walk(a, out)),
        Expr::Object(fields) => fields.iter().for_each(|(_, a)| walk(a, out)),
        Expr::Literal(_) | Expr::Field(_) | Expr::Exists(_) => {}
    }
}

fn is_comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge
            | BinaryOp::In
            | BinaryOp::NotIn
    )
}

/// Record `field <op> literal` when `field` is an input path (not a `$variable`).
fn note(field: &Expr, literal: &Expr, out: &mut BTreeSet<Threshold>) {
    let Expr::Field(path) = field else {
        return;
    };
    if path.starts_with('$') || path.is_empty() {
        return;
    }
    let mut add = |v: &Value| {
        let literal = match v {
            Value::Int(_) | Value::Float(_) | Value::Decimal(_) => {
                match serde_json::to_value(v).ok() {
                    Some(Json::Number(n)) => Literal::Number(n.to_string()),
                    Some(Json::String(s)) => Literal::Number(s),
                    _ => return,
                }
            }
            Value::String(s) => Literal::Str(s.to_string()),
            _ => return,
        };
        out.insert(Threshold {
            field: path.clone(),
            literal,
        });
    };
    match literal {
        Expr::Literal(Value::Array(items)) => items.iter().for_each(&mut add),
        Expr::Literal(v) => add(v),
        Expr::Array(items) => {
            for item in items {
                if let Expr::Literal(v) = item {
                    add(v);
                }
            }
        }
        _ => {}
    }
}

/// Set a dotted path in a JSON object, creating intermediate objects. Returns
/// false when the path runs through a non-object.
fn set_path(root: &mut Json, path: &str, value: Json) -> bool {
    let mut cur = root;
    let parts: Vec<&str> = path.split('.').collect();
    for (i, part) in parts.iter().enumerate() {
        let Json::Object(map) = cur else {
            return false;
        };
        if i == parts.len() - 1 {
            map.insert(part.to_string(), value);
            return true;
        }
        cur = map
            .entry(part.to_string())
            .or_insert_with(|| Json::Object(Default::default()));
    }
    false
}

fn print_report(r: &Report) {
    use colored::Colorize;

    println!(
        "{} {}  ({} → working tree)",
        "impact".bold(),
        r.ruleset,
        r.base
    );
    if !r.base_found {
        println!(
            "  {}",
            format!("{} has no baseline at {} (new ruleset)", r.ruleset, r.base).yellow()
        );
    }
    println!(
        "  {} inputs: {} tests, {} captured, {} boundary probes",
        r.inputs, r.tests, r.captured, r.probes
    );
    println!();

    if r.changed == 0 {
        println!("{} no decision changed", "✓".green());
        return;
    }

    for (t, n) in &r.transitions {
        println!(
            "  {}  {} input{}",
            t.yellow().bold(),
            n,
            if *n == 1 { "" } else { "s" }
        );
    }
    if !r.base_found {
        // Every input is "new"; the per-input list adds nothing.
        return;
    }
    println!();
    for c in r.changes.iter().take(MAX_LISTED) {
        println!(
            "{} {}  {}",
            "CHANGED".yellow().bold(),
            c.source,
            c.input.to_string().dimmed()
        );
        for d in &c.diffs {
            println!("    {d}");
        }
    }
    if r.changes.len() > MAX_LISTED {
        println!(
            "  … {} more (use --json for all)",
            r.changes.len() - MAX_LISTED
        );
    }
    println!();
    println!(
        "{} changed · {} unchanged",
        r.changed.to_string().yellow(),
        r.unchanged.to_string().green()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_come_from_comparisons_with_input_fields() {
        let e = ordo_core::prelude::ExprParser::parse(
            r#"amount <= 10000 && tier in ["gold", "vip"] && $risk > 3 && 0.5 < rate"#,
        )
        .unwrap();
        let mut out = BTreeSet::new();
        walk(&e, &mut out);
        let got: Vec<String> = out
            .iter()
            .map(|t| match &t.literal {
                Literal::Number(n) => format!("{}={n}", t.field),
                Literal::Str(s) => format!("{}={s:?}", t.field),
            })
            .collect();
        assert_eq!(
            got,
            vec![
                "amount=10000",
                "rate=0.5",
                r#"tier="gold""#,
                r#"tier="vip""#
            ]
        );
    }

    #[test]
    fn probes_bracket_the_threshold() {
        let t = Threshold {
            field: "amount".into(),
            literal: Literal::Number("10000".into()),
        };
        assert_eq!(
            t.candidates(),
            vec![json!(9999), json!(10000), json!(10001)]
        );
    }

    #[test]
    fn set_path_creates_nested_objects() {
        let mut v = json!({"a": 1});
        assert!(set_path(&mut v, "user.age", json!(30)));
        assert_eq!(v, json!({"a": 1, "user": {"age": 30}}));
        let mut scalar = json!({"user": 5});
        assert!(!set_path(&mut scalar, "user.age", json!(30)));
    }

    #[test]
    fn numerically_equal_outputs_are_not_a_change() {
        let mut out = Vec::new();
        diff_json("output", &json!({"x": 10}), &json!({"x": 10.0}), &mut out);
        assert!(out.is_empty());
        diff_json(
            "output",
            &json!({"x": 1}),
            &json!({"x": 2, "y": true}),
            &mut out,
        );
        assert_eq!(out, vec!["output.x: 1 → 2", "output.y: (added) → true"]);
    }
}
