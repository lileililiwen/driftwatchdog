//! Packaging tests bridge. Runs the bash + Node packaging suites via
//! the `tests/packaging/run_all.sh` script so the standard
//! `cargo test` invocation also exercises the release distribution
//! contract. The bash portion is required; the Node portion is best
//! effort (skipped if `node` is not on PATH on the test host).

use std::path::PathBuf;
use std::process::Command;

#[test]
fn packaging_bash_and_node_suites_pass() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runner = repo_root.join("tests/packaging/run_all.sh");
    assert!(
        runner.exists(),
        "expected packaging test runner at {}",
        runner.display()
    );

    let output = Command::new("sh")
        .arg(&runner)
        .current_dir(&repo_root)
        .output()
        .expect("failed to spawn packaging test runner");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "packaging test runner failed (exit={:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        stdout,
        stderr
    );
}
