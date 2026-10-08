//! Integration tests for `ordo impact` — a temp project committed to git, then
//! edited, asserting the changed decisions (including ones no test covers).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_ordo");

fn temp_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ordo-impact-it-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let out = run(&dir, &["init", "."]);
    assert!(
        out.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run ordo")
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn commit_all(dir: &Path) {
    git(dir, &["init", "-q"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", "base"]);
}

fn json(out: &Output) -> serde_json::Value {
    let s = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(s.trim()).unwrap_or_else(|e| panic!("stdout not JSON: {e}\n{s}"))
}

/// The init scaffold's rule is `amount <= 10000` → APPROVED, else REJECTED.
fn move_threshold(dir: &Path, to: &str) {
    let path = dir.join("rulesets/loan-approval.json");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"value\": 10000,"));
    std::fs::write(
        &path,
        text.replace("\"value\": 10000,", &format!("\"value\": {to},")),
    )
    .unwrap();
}

#[test]
fn unchanged_ruleset_reports_no_changes() {
    let dir = temp_project("same");
    commit_all(&dir);
    let out = run(&dir, &["impact", "loan-approval", "--json"]);
    assert!(out.status.success());
    let r = json(&out);
    assert_eq!(r["base_found"], true);
    assert_eq!(r["changed"], 0);
    assert!(r["probes"].as_u64().unwrap() > 0);
}

#[test]
fn moved_threshold_is_caught_even_though_tests_still_pass() {
    let dir = temp_project("moved");
    commit_all(&dir);
    move_threshold(&dir, "15000");

    // The existing tests (5000, 20000) don't notice the change…
    assert!(run(&dir, &["test"]).status.success());

    // …but impact does, via probes around both thresholds.
    let r = json(&run(&dir, &["impact", "loan-approval", "--json"]));
    assert_eq!(r["changed"], 3, "{r:#}");
    assert_eq!(r["transitions"]["REJECTED → APPROVED"], 3);
    let amounts: Vec<i64> = r["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["input"]["amount"].as_i64().unwrap())
        .collect();
    assert_eq!(amounts, vec![10001, 14999, 15000]);
    let first = &r["changes"][0];
    assert_eq!(first["before"]["code"], "REJECTED");
    assert_eq!(first["after"]["code"], "APPROVED");
    assert!(first["diffs"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("output.approved: false → true")));

    // CI gate.
    assert!(!run(&dir, &["impact", "loan-approval", "--fail-on-change"])
        .status
        .success());
    // Without probes only the tests are compared, and they don't change.
    let r = json(&run(
        &dir,
        &["impact", "loan-approval", "--no-probes", "--json"],
    ));
    assert_eq!(r["changed"], 0);
}

#[test]
fn captured_inputs_and_base_file_are_compared() {
    let dir = temp_project("inputs");
    // No git: compare against a saved copy instead.
    std::fs::copy(
        dir.join("rulesets/loan-approval.json"),
        dir.join("baseline.json"),
    )
    .unwrap();
    move_threshold(&dir, "12000");
    std::fs::write(
        dir.join("cases.jsonl"),
        "{\"rule_name\":\"loan-approval\",\"input\":{\"amount\":11000},\"code\":\"REJECTED\"}\n{\"amount\":3000}\n",
    )
    .unwrap();
    let r = json(&run(
        &dir,
        &[
            "impact",
            "loan-approval",
            "--base-file",
            "baseline.json",
            "--inputs",
            "cases.jsonl",
            "--no-probes",
            "--json",
        ],
    ));
    assert_eq!(r["captured"], 2);
    assert_eq!(r["changed"], 1, "{r:#}");
    assert_eq!(r["changes"][0]["source"], "input:1");
    assert_eq!(r["changes"][0]["input"]["amount"], 11000);
}

#[test]
fn new_ruleset_and_bad_revision() {
    let dir = temp_project("new");
    // Not a git repo yet: everything is new, not an error.
    let r = json(&run(&dir, &["impact", "loan-approval", "--json"]));
    assert_eq!(r["base_found"], false);
    assert!(r["transitions"]["(new) → APPROVED"].as_u64().unwrap() > 0);

    commit_all(&dir);
    let out = run(&dir, &["impact", "loan-approval", "--base", "no-such-rev"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown git revision"));
}
