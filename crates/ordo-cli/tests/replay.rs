//! Integration tests for `ordo replay` — drive the real binary against a temp
//! project + a captured JSONL, asserting the consistent/flipped/errored buckets
//! and the --write-tests → `ordo test` regression loop.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_ordo");

fn temp_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ordo-replay-it-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(BIN)
        .args(["init", "."])
        .current_dir(&dir)
        .output()
        .expect("init");
    assert!(
        out.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

fn run(dir: &PathBuf, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run ordo")
}

fn run_stdin(dir: &PathBuf, args: &[&str], body: &str) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ordo");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    child.wait_with_output().expect("wait ordo")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_str(stdout(out).trim())
        .unwrap_or_else(|e| panic!("stdout not JSON: {e}\n{}", stdout(out)))
}

// The init scaffold's loan-approval rule: amount <= 10000 → APPROVED, else REJECTED.

#[test]
fn replay_buckets_consistent_flipped_unknown_replayed_skipped() {
    let dir = temp_project("buckets");
    let cap = concat!(
        r#"{"rule_name":"loan-approval","input":{"amount":5000},"code":"APPROVED"}"#,
        "\n",
        r#"{"rule_name":"loan-approval","input":{"amount":20000},"code":"APPROVED"}"#,
        "\n", // flip: really REJECTED
        r#"{"rule_name":"loan-approval","input":{"amount":8000}}"#,
        "\n", // input-only
        r#"{"rule_name":"nope","input":{"x":1},"code":"FOO"}"#,
        "\n",                 // unknown ruleset
        "this is not json\n", // skipped
    );
    std::fs::write(dir.join("cap.jsonl"), cap).unwrap();

    let out = run(&dir, &["replay", "cap.jsonl", "--json"]);
    assert!(
        out.status.success(),
        "replay exited nonzero: {}",
        stdout(&out)
    );
    let v = json(&out);
    assert_eq!(v["total"], 4);
    assert_eq!(v["consistent"], 1);
    assert_eq!(v["flipped"], 1);
    assert_eq!(v["unknown_ruleset"], 1);
    assert_eq!(v["replayed"], 1);
    assert_eq!(v["skipped"], 1);

    let flip = v["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["status"] == "flipped")
        .unwrap();
    assert_eq!(flip["old_code"], "APPROVED");
    assert_eq!(flip["new_code"], "REJECTED");
}

#[test]
fn write_tests_fixates_captures_and_dedupes_then_ordo_test_passes() {
    let dir = temp_project("writetests");
    // 5000 already exists in the scaffolded tests (dedup); 25000 is new.
    let cap = concat!(
        r#"{"rule_name":"loan-approval","input":{"amount":5000},"code":"APPROVED"}"#,
        "\n",
        r#"{"rule_name":"loan-approval","input":{"amount":25000},"code":"REJECTED"}"#,
        "\n",
    );
    std::fs::write(dir.join("cap.jsonl"), cap).unwrap();

    let out = run(&dir, &["replay", "cap.jsonl", "--write-tests", "--json"]);
    assert!(out.status.success());
    assert_eq!(
        json(&out)["written_tests"][0],
        "tests/loan-approval.json (+1)"
    );

    // The fixated case joins the scaffolded ones and all pass.
    let tests: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("tests/loan-approval.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(tests.as_array().unwrap().len(), 3);

    let out = run(&dir, &["test", "--json"]);
    assert!(out.status.success(), "ordo test failed: {}", stdout(&out));
    assert_eq!(json(&out)["failed"], 0);

    // Re-running write-tests is idempotent (both inputs now present → +0).
    let out = run(&dir, &["replay", "cap.jsonl", "--write-tests", "--json"]);
    assert_eq!(
        json(&out)["written_tests"][0],
        "tests/loan-approval.json (+0)"
    );
}

#[test]
fn replay_reads_stdin_dash() {
    let dir = temp_project("stdin");
    let body = concat!(
        r#"{"rule_name":"loan-approval","input":{"amount":5000},"code":"APPROVED"}"#,
        "\n",
        r#"{"rule_name":"loan-approval","input":{"amount":25000},"code":"REJECTED"}"#,
        "\n",
    );
    let out = run_stdin(&dir, &["replay", "-", "--json"], body);
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["total"], 2);
    assert_eq!(v["consistent"], 2);
}

#[test]
fn fail_on_flip_exits_nonzero() {
    let dir = temp_project("failflip");
    std::fs::write(
        dir.join("cap.jsonl"),
        concat!(
            r#"{"rule_name":"loan-approval","input":{"amount":25000},"code":"APPROVED"}"#,
            "\n"
        ),
    )
    .unwrap();

    // Without the flag: report-only, exit 0.
    assert!(run(&dir, &["replay", "cap.jsonl"]).status.success());
    // With the flag: a flip fails the run.
    let out = run(&dir, &["replay", "cap.jsonl", "--fail-on-flip"]);
    assert!(!out.status.success(), "expected nonzero exit on flip");
}

#[test]
fn ruleset_override_replays_all_records_against_one_rule() {
    let dir = temp_project("override");
    // records name a bogus rule; --ruleset forces loan-approval.
    let cap = concat!(
        r#"{"rule_name":"whatever","input":{"amount":5000},"code":"APPROVED"}"#,
        "\n",
        r#"{"rule_name":"whatever","input":{"amount":25000},"code":"REJECTED"}"#,
        "\n",
    );
    std::fs::write(dir.join("cap.jsonl"), cap).unwrap();
    let out = run(
        &dir,
        &[
            "replay",
            "cap.jsonl",
            "--ruleset",
            "loan-approval",
            "--json",
        ],
    );
    assert!(out.status.success());
    let v = json(&out);
    assert_eq!(v["consistent"], 2);
    assert_eq!(v["unknown_ruleset"], 0);
}
