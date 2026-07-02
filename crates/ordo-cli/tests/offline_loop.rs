//! End-to-end test of the offline dev loop, driving the real `ordo` binary.
//!
//! This exercises the whole local surface (init → validate → test → trace → fmt
//! → lint → new → eval) as a user would — through `#[tokio::main]`-free `main`,
//! the embedded engine, and the `reqwest::blocking` capability invoker. A unit
//! test can't catch a runtime-drop panic in that path; this does.

use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_ordo");

fn temp_project(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ordo-cli-e2e-{}-{}-{}",
        tag,
        std::process::id(),
        nanos
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(dir: &PathBuf, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `ordo {}`: {e}", args.join(" ")))
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn assert_ok(out: &Output, what: &str) {
    assert!(
        out.status.success(),
        "`{what}` exited {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[test]
fn offline_loop_runs_end_to_end() {
    let dir = temp_project("loop");

    // init
    assert_ok(&run(&dir, &["init", "."]), "init");
    assert!(dir.join("ordo.yaml").is_file());
    assert!(dir.join("rulesets/loan-approval.json").is_file());

    // validate --json → { ok: true }
    let out = run(&dir, &["validate", "--json"]);
    assert_ok(&out, "validate");
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("validate --json");
    assert_eq!(v["ok"], serde_json::json!(true), "validate should pass");

    // test — this is the path that used to panic on runtime drop.
    let out = run(&dir, &["test", "--json"]);
    assert_ok(&out, "test");
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("test --json");
    assert_eq!(v["failed"], serde_json::json!(0), "no test should fail");
    assert!(
        v["passed"].as_u64().unwrap() >= 1,
        "at least one test passes"
    );

    // trace — also executes the engine.
    let out = run(
        &dir,
        &[
            "trace",
            "loan-approval",
            "--input",
            "{\"amount\":5000}",
            "--json",
        ],
    );
    assert_ok(&out, "trace");
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("trace --json");
    assert_eq!(
        v["code"],
        serde_json::json!("APPROVED"),
        "5000 is within limit"
    );

    // fmt (already formatted right after init) and lint
    assert_ok(&run(&dir, &["fmt"]), "fmt");
    assert_ok(&run(&dir, &["lint"]), "lint");

    // new ruleset → new file, still validates
    assert_ok(
        &run(&dir, &["new", "ruleset", "discount-check"]),
        "new ruleset",
    );
    assert!(dir.join("rulesets/discount-check.json").is_file());
    assert_ok(&run(&dir, &["validate"]), "validate after new");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn eval_expression_json() {
    let dir = temp_project("eval");
    let out = run(&dir, &["eval", "1 + 2 * 3", "--json"]);
    assert_ok(&out, "eval");
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("eval --json");
    assert_eq!(v["value"], serde_json::json!(7));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hard_error_is_json_when_json_flag_set() {
    // Running a project command outside any project must fail — and under --json
    // that failure must itself be machine-readable on stdout.
    let dir = temp_project("noproj");
    let out = run(&dir, &["validate", "--json"]);
    assert!(
        !out.status.success(),
        "validate outside a project should fail"
    );
    let v: serde_json::Value =
        serde_json::from_str(&stdout(&out)).expect("hard error should be JSON under --json");
    assert!(
        v["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Ordo project"),
        "error should mention the missing project, got: {}",
        stdout(&out)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
