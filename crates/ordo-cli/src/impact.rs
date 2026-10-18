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

    /// Before switching models: ask this model again for every request in the
    /// --inputs decision log (ordo-server --decide-log-requests) and compare
    /// its decisions with the logged ones
    #[arg(long, value_name = "MODEL", requires = "inputs")]
    model: Option<String>,

    /// System One compatible API used by --model (default: $ORDO_DECIDE_UPSTREAM,
    /// else https://openrouter.ai/api)
    #[arg(long, value_name = "URL", requires = "model")]
    upstream: Option<String>,

    /// Ask --model each request N times: models do not answer the same way
    /// twice, and a decision that changes on only some asks sits on a threshold
    #[arg(long, value_name = "N", default_value_t = 1, requires = "model")]
    repeat: usize,
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
    /// With --model: the text the model was asked about
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    /// With --model: how the model's answers moved (`jev.intent.confidence: 0.97 → 0.87`)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    answers: Vec<String>,
    /// With --model --repeat: on how many asks the decision changed
    #[serde(skip_serializing_if = "Option::is_none")]
    asks: Option<Asks>,
}

#[derive(Serialize)]
pub(crate) struct Asks {
    changed: usize,
    answered: usize,
}

/// With --model: which model was asked again, and how that went.
#[derive(Serialize)]
pub(crate) struct ModelSwitch {
    /// The model(s) that answered the logged requests
    from: String,
    /// The model asked again, as the upstream reports it
    to: String,
    upstream: String,
    /// How many times each request was asked
    repeat: usize,
    /// Requests asked
    asked: usize,
    /// Requests the model answered on no ask
    failed: usize,
    /// Decisions for this ruleset logged without their request
    without_request: usize,
    /// The first few failures
    errors: Vec<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<ModelSwitch>,
}

pub fn run(args: ImpactArgs, json: bool) -> Result<()> {
    let project = Project::discover(None)?;
    let name = ruleset_name(&args.ruleset);
    let base = match args.base_file {
        Some(f) => Base::File(f),
        None => Base::Rev(args.base.unwrap_or_else(|| "HEAD".to_string())),
    };
    let report = match (&args.model, &args.inputs) {
        (Some(model), Some(log)) => {
            let upstream = args
                .upstream
                .clone()
                .or_else(|| std::env::var("ORDO_DECIDE_UPSTREAM").ok())
                .unwrap_or_else(|| crate::remodel::DEFAULT_UPSTREAM.to_string());
            compute_remodel(
                &project,
                &name,
                &base,
                Path::new(log),
                model,
                &upstream,
                args.repeat,
            )?
        }
        _ => {
            let extra = match &args.inputs {
                Some(path) => read_inputs(Path::new(path), &name)?,
                None => Vec::new(),
            };
            compute(&project, &name, &base, extra, !args.no_probes)?
        }
    };
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
    let (after_rs, before_rs) = load_versions(project, name, base)?;

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
            state: None,
            answers: Vec::new(),
            asks: None,
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
        model: None,
    })
}

/// The working-tree ruleset and the baseline (`None` for a new ruleset), compiled.
fn load_versions(project: &Project, name: &str, base: &Base) -> Result<(RuleSet, Option<RuleSet>)> {
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
    Ok((after_rs, before_rs))
}

/// `--model`: ask another model the logged requests and compare decisions.
///
/// Before: the baseline ruleset on the logged input (the old model's answers).
/// After: the working-tree ruleset on the same facts with the new model's
/// answers. With no rule edit, every change comes from the model. Only a
/// different code or message counts as a change here: output fields that echo
/// the model's probabilities move on every call.
pub(crate) fn compute_remodel(
    project: &Project,
    name: &str,
    base: &Base,
    log_path: &Path,
    model: &str,
    upstream: &str,
    repeat: usize,
) -> Result<Report> {
    let (after_rs, before_rs) = load_versions(project, name, base)?;
    let log = crate::remodel::read_log(log_path, name)?;
    if log.calls.is_empty() {
        if log.without_request > 0 {
            anyhow::bail!(
                "{} has {} decision(s) for {name} but none with its request, so there is nothing to ask {model}. \
                 Start ordo-server with --decide-log-requests (ORDO_DECIDE_LOG_REQUESTS=true) to log requests.",
                log_path.display(),
                log.without_request
            );
        }
        anyhow::bail!("{} has no decisions for {name}", log_path.display());
    }

    let asker = crate::remodel::Asker::new(upstream, model)?;
    let repeat = repeat.max(1);
    // Each request `repeat` times in a row, so replies[i * repeat..] belong to call i.
    let asks: Vec<&Json> = log
        .calls
        .iter()
        .flat_map(|c| std::iter::repeat(&c.request).take(repeat))
        .collect();
    let mut replies = asker.ask_all(&asks).into_iter();

    let after = LoadedRule::Source(after_rs);
    let before = before_rs.map(LoadedRule::Source);
    let mut changes = Vec::new();
    let mut transitions: BTreeMap<String, usize> = BTreeMap::new();
    let mut unchanged = 0usize;
    let mut errors = Vec::new();
    let mut failed = 0usize;
    let mut to: BTreeSet<String> = BTreeSet::new();
    let from: BTreeSet<String> = log.calls.iter().map(|c| c.model.clone()).collect();

    for call in &log.calls {
        let before_out = match &before {
            Some(rule) => run_one(rule, &call.input),
            None => Err(NEW.to_string()),
        };
        // Every answered ask whose decision moved, plus how many asks answered.
        let mut moved: Vec<(Json, Side, Vec<String>, Json)> = Vec::new();
        let mut answered = 0usize;
        for reply in replies.by_ref().take(repeat) {
            let reply = match reply {
                Ok(r) => r,
                Err(e) => {
                    errors.push(format!("line {}: {e}", call.line));
                    continue;
                }
            };
            answered += 1;
            to.insert(reply.model.clone());
            let mut input = call.input.clone();
            if let Json::Object(map) = &mut input {
                map.insert(call.answers_field.clone(), reply.answers.clone());
            }
            let after_out = run_one(&after, &input);
            let diffs = diff_outcomes(&before_out, &after_out);
            // A new model moves every echoed probability in the output; what
            // counts is whether the decision itself (code, message) changed.
            let changed = match (&before_out, &after_out) {
                (Ok(b), Ok(a)) => b.code != a.code || b.message != a.message,
                _ => !diffs.is_empty(),
            };
            if changed {
                moved.push((input, after_out, diffs, reply.answers));
            }
        }
        if answered == 0 {
            failed += 1;
            continue;
        }
        if moved.is_empty() {
            unchanged += 1;
            continue;
        }
        // Report the new decision the model gave most often.
        let mut tally: BTreeMap<String, usize> = BTreeMap::new();
        for (_, out, _, _) in &moved {
            *tally.entry(code_label(out)).or_default() += 1;
        }
        let top = tally
            .iter()
            .max_by_key(|(_, n)| **n)
            .map(|(c, _)| c.clone())
            .unwrap_or_default();
        let times = moved.len();
        let (input, after_out, diffs, new_answers) = moved
            .into_iter()
            .find(|(_, out, _, _)| code_label(out) == top)
            .expect("top code comes from moved");
        *transitions
            .entry(format!("{} → {}", code_label(&before_out), top))
            .or_default() += 1;
        let old_answers = call
            .input
            .get(&call.answers_field)
            .cloned()
            .unwrap_or(Json::Null);
        let mut answers = Vec::new();
        diff_json(
            &call.answers_field,
            &crate::remodel::verdict(&old_answers),
            &crate::remodel::verdict(&new_answers),
            &mut answers,
        );
        changes.push(Change {
            source: format!("line:{}", call.line),
            input,
            before: before_out,
            after: after_out,
            diffs,
            state: call.request.get("state").map(|s| {
                s.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| s.to_string())
            }),
            answers,
            asks: (repeat > 1).then_some(Asks {
                changed: times,
                answered,
            }),
        });
    }

    if failed == log.calls.len() {
        anyhow::bail!(
            "{model} answered none of the {} requests: {}",
            failed,
            errors.first().map(String::as_str).unwrap_or("")
        );
    }
    errors.truncate(5);
    let asked = log.calls.len();
    let join = |s: BTreeSet<String>| s.into_iter().collect::<Vec<_>>().join(", ");
    Ok(Report {
        ruleset: name.to_string(),
        base: base.label(),
        base_found: before.is_some(),
        inputs: asked - failed,
        tests: 0,
        captured: asked - failed,
        probes: 0,
        unchanged,
        changed: changes.len(),
        transitions,
        changes,
        model: Some(ModelSwitch {
            from: join(from),
            to: if to.is_empty() {
                model.to_string()
            } else {
                join(to)
            },
            upstream: upstream.to_string(),
            repeat,
            asked,
            failed,
            without_request: log.without_request,
            errors,
        }),
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
/// Read `--inputs`: capture or decision-log lines (`{input, ...}`) or bare
/// input objects. Lines recorded for a different ruleset are skipped, so one
/// log file can serve several rulesets.
fn read_inputs(path: &Path, ruleset: &str) -> Result<Vec<Json>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read inputs from {}", path.display()))?;
    let mut inputs = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: Json = serde_json::from_str(line)
            .with_context(|| format!("{}:{}: invalid JSON", path.display(), i + 1))?;
        let recorded_for = v
            .get("ruleset")
            .or_else(|| v.get("rule_name"))
            .and_then(Json::as_str);
        if v.get("input").is_some() && recorded_for.is_some_and(|r| r != ruleset) {
            continue;
        }
        inputs.push(match v.get("input") {
            // Decision-log lines from calls where no ruleset ran have no rule input.
            Some(Json::Null) => continue,
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
    match &r.model {
        Some(m) => {
            println!("  model: {} → {}", m.from, m.to.bold());
            println!(
                "  {} logged requests asked again{}{}",
                m.asked,
                if m.repeat > 1 {
                    format!(", {} times each", m.repeat)
                } else {
                    String::new()
                },
                if m.failed > 0 {
                    format!(", {} failed", m.failed).red().to_string()
                } else {
                    String::new()
                }
            );
            for e in &m.errors {
                println!("    {}", e.red());
            }
            if m.without_request > 0 {
                println!(
                    "  {}",
                    format!(
                        "{} decisions skipped: logged without their request (--decide-log-requests)",
                        m.without_request
                    )
                    .yellow()
                );
            }
        }
        None => println!(
            "  {} inputs: {} tests, {} captured, {} boundary probes",
            r.inputs, r.tests, r.captured, r.probes
        ),
    }
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
        match &c.state {
            Some(state) => {
                let asks = c
                    .asks
                    .as_ref()
                    .map(|a| format!("  ({} of {} asks)", a.changed, a.answered))
                    .unwrap_or_default();
                println!(
                    "{} {}{}  {}",
                    "CHANGED".yellow().bold(),
                    c.source,
                    asks.bold(),
                    short(state, 100)
                );
            }
            None => println!(
                "{} {}  {}",
                "CHANGED".yellow().bold(),
                c.source,
                c.input.to_string().dimmed()
            ),
        }
        for d in &c.diffs {
            println!("    {d}");
        }
        for a in &c.answers {
            println!("    {}", a.dimmed());
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

/// The first `max` characters of `s`, on one line.
fn short(s: &str, max: usize) -> String {
    let one_line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max {
        return one_line;
    }
    let cut: String = one_line.chars().take(max).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inputs_skip_other_rulesets_and_calls_without_a_decision() {
        let path =
            std::env::temp_dir().join(format!("ordo-impact-inputs-{}.jsonl", std::process::id()));
        std::fs::write(
            &path,
            [
                r#"{"ruleset":"refund-routing","input":{"a":1},"code":"LLM_DRAFT"}"#,
                r#"{"ruleset":"other","input":{"a":2}}"#,
                r#"{"model":"jev-1.13.0","answers":{},"input":null}"#,
                r#"{"rule_name":"refund-routing","input":{"a":3}}"#,
                r#"{"a":4}"#,
            ]
            .join("\n"),
        )
        .unwrap();
        let got = read_inputs(&path, "refund-routing").unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(got, vec![json!({"a":1}), json!({"a":3}), json!({"a":4})]);
    }

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
