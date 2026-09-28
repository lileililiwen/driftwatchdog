//! End-to-end tests for the built-in `source-file-size` Gate concern.
//!
//! All tests are Unix-only because the project-runtime adapter launches
//! its target via `sh -c`. The unit tests in `src/gate/source_size.rs`
//! cover the scanner boundary, glob/gitignore matching, counting, and
//! review classification; these fixtures exercise the wiring through
//! `driftwatch gate` (profile resolution, in-process dispatch,
//! aggregation, human/JSON output, and `gate_runs` persistence).
//!
//! The product profile also schedules `product-code-boundary` and
//! `placeholder-threshold`, which are project-owned and would land on
//! `REVIEW_REQUIRED` without a bound command. Every fixture here
//! explicitly relaxes those two to `required = false` so the assertions
//! isolate the built-in scanner.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatchdog::repo::{gates::Gates, Db};
use tempfile::tempdir;

fn driftwatch() -> Command {
    Command::cargo_bin("driftwatchdog").expect("compiled driftwatch binary")
}

fn init_dir() -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    driftwatch()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();
    tmp
}

fn latest_run(tmp: &tempfile::TempDir) -> driftwatchdog::repo::gates::GateRunRow {
    let path = tmp.path().join(".driftwatch/state.db");
    let db = Db::open_read_only(&path).expect("read-only db");
    Gates::new(&db)
        .latest()
        .expect("query gate_runs")
        .expect("at least one gate run")
}

fn write_gate_manifest(tmp: &tempfile::TempDir, body: &str) {
    std::fs::write(tmp.path().join("gate.toml"), body).unwrap();
}

/// Write a UTF-8 text file with exactly `newlines` `0x0A` bytes.
fn write_lines(tmp: &tempfile::TempDir, rel: &str, newlines: usize) {
    let path = tmp.path().join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, "x\n".repeat(newlines)).unwrap();
}

/// A `product` manifest that relaxes the two project-owned concerns and
/// appends the caller's extra policy (e.g. a `[source_size]` table).
fn source_manifest(extra: &str) -> String {
    format!(
        "version = 1\nprofile = \"product\"\n\
[[checks]]\nid = \"product-code-boundary\"\nrequired = false\n\
[[checks]]\nid = \"placeholder-threshold\"\nrequired = false\n{extra}"
    )
}

/// Run `driftwatch gate` with optional flags and capture the process
/// output (status + stdout) so both exit code and rendering can be
/// asserted.
fn gate_output(tmp: &tempfile::TempDir, args: &[&str]) -> std::process::Output {
    let mut cmd = driftwatch();
    cmd.arg("gate");
    for arg in args {
        cmd.arg(arg);
    }
    cmd.current_dir(tmp.path())
        .output()
        .expect("run driftwatch gate")
}

fn persisted_run_count(tmp: &tempfile::TempDir) -> i64 {
    let path = tmp.path().join(".driftwatch/state.db");
    let db = Db::open_read_only(&path).expect("read-only db");
    Gates::new(&db).count().unwrap()
}

#[test]
fn product_profile_passes_at_exactly_1000_lines() {
    let tmp = init_dir();
    write_lines(&tmp, "src/feature.rs", 1000);
    write_gate_manifest(&tmp, &source_manifest(""));

    let output = gate_output(&tmp, &[]);
    assert!(output.status.success(), "1000 lines must pass");
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
    assert!(run.results_json.contains("source-file-size"));
    assert!(run.results_json.contains("\"PASS\""));
}

#[test]
fn rust_product_profile_selects_the_built_in_scanner() {
    let tmp = init_dir();
    write_lines(&tmp, "src/feature.rs", 1000);
    let manifest = "version = 1\nprofile = \"rust-product\"\n\
[[checks]]\nid = \"product-code-boundary\"\nrequired = false\n\
[[checks]]\nid = \"placeholder-threshold\"\nrequired = false\n";
    write_gate_manifest(&tmp, manifest);

    let output = gate_output(&tmp, &[]);
    assert!(output.status.success());
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(run.results_json.contains("source-file-size"));
}

#[test]
fn one_thousand_one_lines_fail_and_block() {
    let tmp = init_dir();
    write_lines(&tmp, "src/big.rs", 1001);
    write_gate_manifest(&tmp, &source_manifest(""));

    let output = gate_output(&tmp, &[]);
    assert!(!output.status.success(), "1001 lines must block");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("source-file-size"),
        "human output: {stdout}"
    );
    assert!(
        stdout.contains("exceed the maximum of 1000"),
        "human output diagnostic: {stdout}"
    );

    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);
    assert!(run.results_json.contains("source-file-size"));
    assert!(run.results_json.contains("1001 physical lines"));
    assert!(run.results_json.contains("src/big.rs"));
}

#[test]
fn json_output_reports_the_source_file_size_finding() {
    let tmp = init_dir();
    write_lines(&tmp, "src/big.rs", 1001);
    write_gate_manifest(&tmp, &source_manifest(""));

    let output = gate_output(&tmp, &["--format", "json"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("json output");
    assert_eq!(doc["status"], "FAIL");
    assert_eq!(doc["blocked"], true);

    let result = doc["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["gate_id"] == "source-file-size")
        .expect("source-file-size result present");
    assert_eq!(result["status"], "FAIL");
    assert_eq!(result["source"], "driftwatchdog");
    assert_eq!(result["findings"][0]["location"], "src/big.rs");
    assert_eq!(result["findings"][0]["rule"], "source-file-size");
    assert_eq!(result["findings"][0]["severity"], "error");

    // The normalized result is also persisted in the existing bounded
    // `results_json` field.
    let run = latest_run(&tmp);
    assert!(run.results_json.contains("src/big.rs"));
}

#[test]
fn dependency_ignored_and_binary_trees_do_not_fail() {
    let tmp = init_dir();
    std::fs::write(tmp.path().join(".gitignore"), "ignored/\n*.log\n").unwrap();
    write_lines(&tmp, "src/ok.rs", 10);
    write_lines(&tmp, "node_modules/dep/index.js", 10_000);
    write_lines(&tmp, "target/debug/build.rs", 10_000);
    write_lines(&tmp, "ignored/big.rs", 10_000);
    write_lines(&tmp, "src/debug.log", 10_000);
    // A NUL-containing file is non-source and skipped, not failed.
    std::fs::write(tmp.path().join("src/binary.rs"), b"line\n\0binary\n").unwrap();

    write_gate_manifest(&tmp, &source_manifest(""));
    let output = gate_output(&tmp, &[]);
    assert!(
        output.status.success(),
        "ignored/dependency/binary content must not fail: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
}

#[test]
fn git_repository_honors_gitignore_for_dependencies() {
    let tmp = init_dir();
    let root = tmp.path();
    // A real Git repository selects `git ls-files --exclude-standard`
    // instead of the directory walk.
    assert!(
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .expect("git init")
            .success(),
        "git init must succeed"
    );
    std::fs::write(root.join(".gitignore"), "node_modules/\n").unwrap();
    write_lines(&tmp, "src/ok.rs", 5);
    write_lines(&tmp, "node_modules/big.js", 10_000);

    write_gate_manifest(&tmp, &source_manifest(""));
    let output = gate_output(&tmp, &[]);
    assert!(output.status.success());
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
}

#[test]
fn explicit_include_and_exclude_override_discovery() {
    let tmp = init_dir();
    write_lines(&tmp, "packages/service/keep.rs", 10);
    write_lines(&tmp, "packages/service/legacy/big.rs", 5_000);
    write_lines(&tmp, "src/huge.rs", 5_000);
    write_gate_manifest(
        &tmp,
        &source_manifest(
            "\n[source_size]\ninclude = [\"packages/service/**\"]\nexclude = [\"packages/service/legacy/**\"]\n",
        ),
    );

    let output = gate_output(&tmp, &[]);
    assert!(
        output.status.success(),
        "excluded and out-of-include files must not fail"
    );
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
}

#[test]
fn include_without_exclude_reports_only_the_included_file() {
    let tmp = init_dir();
    write_lines(&tmp, "packages/service/big.rs", 5_000);
    write_lines(&tmp, "src/huge.rs", 5_000);
    write_gate_manifest(
        &tmp,
        &source_manifest("\n[source_size]\ninclude = [\"packages/service/**\"]\n"),
    );

    let output = gate_output(&tmp, &[]);
    assert!(!output.status.success());
    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.results_json.contains("packages/service/big.rs"));
    assert!(
        !run.results_json.contains("src/huge.rs"),
        "src/huge.rs is outside the explicit include and must not be a candidate"
    );
}

#[test]
fn broken_git_boundary_is_review_required_and_blocks() {
    let tmp = init_dir();
    write_lines(&tmp, "src/ok.rs", 5);
    // An empty `.git` makes `git ls-files` exit non-zero; the scanner
    // must fail closed instead of claiming a complete pass.
    std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
    write_gate_manifest(&tmp, &source_manifest(""));

    let output = gate_output(&tmp, &[]);
    assert!(!output.status.success());
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("REVIEW_REQUIRED"));
    assert!(run.results_json.contains("source-file-size:git"));
}

#[test]
fn optional_source_file_size_failure_is_visible_but_unblocked() {
    let tmp = init_dir();
    write_lines(&tmp, "src/big.rs", 5_000);
    write_gate_manifest(
        &tmp,
        &source_manifest("\n[[checks]]\nid = \"source-file-size\"\nrequired = false\n"),
    );

    let output = gate_output(&tmp, &[]);
    assert!(output.status.success(), "optional failure must not block");
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
    assert!(run.results_json.contains("source-file-size"));
    assert!(run.results_json.contains("\"FAIL\""));
}

#[test]
fn disabled_source_file_size_is_absent_and_not_scanned() {
    let tmp = init_dir();
    write_lines(&tmp, "src/big.rs", 5_000);
    write_gate_manifest(
        &tmp,
        &source_manifest("\n[[checks]]\nid = \"source-file-size\"\nenabled = false\n"),
    );

    let output = gate_output(&tmp, &[]);
    assert!(output.status.success());
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
    assert!(
        !run.results_json.contains("source-file-size"),
        "a disabled concern must be absent from the plan and the results"
    );
}

#[test]
fn dry_run_renders_policy_without_scanning_or_persisting() {
    let tmp = init_dir();
    write_lines(&tmp, "src/big.rs", 5_000);
    write_gate_manifest(&tmp, &source_manifest("\n[source_size]\nmax_lines = 250\n"));

    let output = gate_output(&tmp, &["--dry-run"]);
    assert!(
        output.status.success(),
        "dry-run exits zero even for a failing would-be scan"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("source-file-size"),
        "dry-run plan: {stdout}"
    );
    assert!(
        stdout.contains("source_size: max_lines=250"),
        "dry-run policy: {stdout}"
    );
    assert_eq!(persisted_run_count(&tmp), 0, "dry-run persists nothing");
}

#[test]
fn foreign_runtime_does_not_run_or_persist() {
    let tmp = init_dir();
    write_lines(&tmp, "src/big.rs", 5_000);
    std::fs::create_dir_all(tmp.path().join(".ai-gate")).unwrap();
    std::fs::write(
        tmp.path().join(".ai-gate/gate.yaml"),
        "version: 1\nruntime: other-gate\nprofile: product\n",
    )
    .unwrap();

    let output = gate_output(&tmp, &[]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("other-gate"),
        "foreign runtime notice: {stdout}"
    );
    assert_eq!(
        persisted_run_count(&tmp),
        0,
        "a foreign runtime persists no gate run"
    );
}
