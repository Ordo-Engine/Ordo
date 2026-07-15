//! Integration test: `ordo push` rejects an invalid `facts.json`/`concepts.json`
//! locally, before any network call — drives the real compiled binary.
//!
//! `ORDO_API_URL` points at a port nothing listens on (an immediate connection
//! refusal, not a slow DNS/timeout failure) so that if the local validation
//! gate is ever accidentally bypassed, this test fails fast and clearly
//! instead of hanging.

use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_ordo");
const UNREACHABLE_API_URL: &str = "http://127.0.0.1:1";

fn temp_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ordo-push-catalog-it-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("ordo.yaml"),
        "project: test\norg_id: org1\nproject_id: proj1\n",
    )
    .unwrap();
    dir
}

fn run(dir: &PathBuf, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env("ORDO_TOKEN", "test-token")
        .env("ORDO_API_URL", UNREACHABLE_API_URL)
        .output()
        .expect("failed to run ordo")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn push_rejects_invalid_null_policy_without_touching_the_network() {
    let dir = temp_project("null-policy");
    std::fs::write(
        dir.join("facts.json"),
        r#"[{"name":"user_score","data_type":"number","source":"input","null_policy":"reject"}]"#,
    )
    .unwrap();

    let out = run(&dir, &["push", "--json"]);
    assert!(
        !out.status.success(),
        "push must fail when facts.json is invalid"
    );
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let facts_row = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == "facts.json")
        .expect("a facts.json result row");
    let status = facts_row["status"].as_str().unwrap();
    assert!(status.starts_with("invalid:"), "got: {status}");
    assert!(status.contains("user_score"), "got: {status}");
    assert!(status.contains("null_policy"), "got: {status}");
    assert!(status.contains("\"reject\""), "got: {status}");
    assert!(
        status.contains("error, default, skip"),
        "must list the valid values: {status}"
    );
}

#[test]
fn push_rejects_invalid_data_type_in_concepts() {
    let dir = temp_project("concept-data-type");
    std::fs::write(
        dir.join("concepts.json"),
        r#"[{"name":"risk_band","data_type":"int","expression":"1"}]"#,
    )
    .unwrap();

    let out = run(&dir, &["push", "--json"]);
    assert!(!out.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let row = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == "concepts.json")
        .expect("a concepts.json result row");
    let status = row["status"].as_str().unwrap();
    assert!(status.starts_with("invalid:"), "got: {status}");
    assert!(status.contains("risk_band"), "got: {status}");
    assert!(status.contains("data_type"), "got: {status}");
}

#[test]
fn push_with_no_facts_or_concepts_files_has_nothing_to_validate() {
    // No rulesets, no facts.json, no concepts.json: push has nothing to do and
    // must not error just because the catalog is absent.
    let dir = temp_project("empty");
    let out = run(&dir, &["push", "--json"]);
    assert!(out.status.success(), "stdout: {}", stdout(&out));
}

#[test]
fn ordo_validate_reports_invalid_facts_json_too() {
    let dir = temp_project("validate-null-policy");
    std::fs::write(
        dir.join("facts.json"),
        r#"[{"name":"score","data_type":"number","source":"input","null_policy":"default_zero"}]"#,
    )
    .unwrap();

    let out = Command::new(BIN)
        .args(["validate", "--json"])
        .current_dir(&dir)
        .output()
        .expect("failed to run ordo validate");
    assert!(!out.status.success());
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(v["ok"], false);
    let rulesets = v["rulesets"].as_array().unwrap();
    let facts_report = rulesets
        .iter()
        .find(|r| r["ruleset"] == "facts.json")
        .expect("a facts.json report");
    assert_eq!(facts_report["ok"], false);
    let message = facts_report["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("null_policy"), "got: {message}");
    assert!(message.contains("default_zero"), "got: {message}");
}
