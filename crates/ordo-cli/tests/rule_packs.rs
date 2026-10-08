//! The example rule packs under `examples/rule-packs/` must validate, lint and
//! pass their own tests with the current engine and CLI.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_ordo");

fn packs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rule-packs")
}

fn assert_ok(dir: &Path, args: &[&str]) {
    let out = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `ordo {}`: {e}", args.join(" ")));
    assert!(
        out.status.success(),
        "`ordo {}` failed in {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        dir.display(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[test]
fn rule_packs_validate_lint_and_pass_their_tests() {
    let mut packs: Vec<PathBuf> = std::fs::read_dir(packs_dir())
        .expect("examples/rule-packs exists")
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("ordo.yaml").is_file())
        .collect();
    packs.sort();
    assert!(
        packs.len() >= 3,
        "expected at least 3 rule packs, found {packs:?}"
    );

    for pack in &packs {
        assert_ok(pack, &["validate"]);
        assert_ok(pack, &["lint"]);
        assert_ok(pack, &["test"]);
    }
}
