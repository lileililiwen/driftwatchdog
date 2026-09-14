//! End-to-end tests of `driftwatch mcp` over stdio.
//!
//! Each test spawns the compiled binary, pipes JSON-RPC lines on
//! stdin, and parses the response stream from stdout. The fixture
//! is a fresh `init` plus one or two failing `run` invocations so
//! every tool has at least one row to return.

#![cfg(unix)]

use std::io::Write;
use std::process::{Command, Stdio};

use tempfile::tempdir;

fn driftwatch() -> Command {
    Command::new(env!("CARGO_BIN_EXE_driftwatchdog"))
}

fn init_dir() -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    driftwatch()
        .arg("init")
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    tmp
}

fn run_failing(tmp: &tempfile::TempDir, msg: &str) {
    driftwatch()
        .args(["run", "sh", "-c", &format!("echo {msg} >&2; exit 1")])
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
}

/// Spawn `driftwatch mcp`, write `lines` (each terminated with
/// `\n`) to its stdin, close stdin, and return the stdout bytes
/// plus the exit status. We use a single helper so every test
/// follows the same shape.
fn mcp_roundtrip(cwd: &std::path::Path, lines: &[&str]) -> (String, std::process::ExitStatus) {
    let mut child = driftwatch()
        .arg("mcp")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn driftwatch mcp");
    {
        let stdin = child.stdin.as_mut().expect("stdin pipe");
        for line in lines {
            let _ = writeln!(stdin, "{line}");
        }
    }
    let output = child.wait_with_output().expect("wait driftwatch mcp");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    // Stderr is intentionally available for diagnostic assertions
    // in some tests; the helper returns only stdout by default.
    let _ = output.stderr;
    (stdout, output.status)
}

fn parse_lines(s: &str) -> Vec<serde_json::Value> {
    s.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("parse JSON-RPC line"))
        .collect()
}

#[test]
fn mcp_handshake_then_tools_list() {
    let tmp = init_dir();
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        ],
    );
    assert!(status.success(), "mcp exited non-zero: {status}");
    let lines = parse_lines(&out);
    assert_eq!(
        lines.len(),
        2,
        "expected handshake + tools/list, got: {out}"
    );
    let init = &lines[0];
    assert_eq!(init["id"], 1);
    assert_eq!(init["result"]["serverInfo"]["name"], "driftwatchdog");
    let tools = &lines[1]["result"]["tools"];
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for n in ["top_bugs", "show_bug", "ai_report", "doctor_status"] {
        assert!(names.contains(&n), "missing tool {n}: {names:?}");
    }
}

#[test]
fn mcp_unknown_method_returns_minus_32601() {
    let tmp = init_dir();
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[r#"{"jsonrpc":"2.0","id":99,"method":"not-a-method"}"#],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["id"], 99);
    assert_eq!(lines[0]["error"]["code"], -32601);
}

#[test]
fn mcp_tools_call_missing_name_returns_minus_32602() {
    let tmp = init_dir();
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{}}"#],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["error"]["code"], -32602);
}

#[test]
fn mcp_tools_call_bad_arguments_returns_minus_32602() {
    let tmp = init_dir();
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"top_bugs","arguments":{"limit":"oops"}}}"#,
        ],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["error"]["code"], -32602);
    assert!(
        lines[0]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("top_bugs"),
        "error message should name the tool: {}",
        lines[0]["error"]["message"]
    );
}

#[test]
fn mcp_top_bugs_returns_json_array_with_one_row() {
    let tmp = init_dir();
    run_failing(&tmp, "boom_mcp_top");
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"top_bugs","arguments":{}}}"#,
        ],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    assert_eq!(lines.len(), 1);
    let text = lines[0]["result"]["content"][0]["text"].as_str().unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(text).expect("top_bugs text is JSON");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["count"], 1);
    assert!(arr[0]["hash"].as_str().unwrap().len() >= 8);
}

#[test]
fn mcp_show_bug_returns_human_text() {
    let tmp = init_dir();
    run_failing(&tmp, "boom_mcp_show");
    // Find the 8-char prefix of the fingerprint we just created.
    let conn = rusqlite::Connection::open(tmp.path().join(".driftwatch/state.db")).unwrap();
    let hash: String = conn
        .query_row("SELECT hash FROM fingerprints LIMIT 1", [], |r| r.get(0))
        .unwrap();
    drop(conn);
    let prefix = &hash[..8];
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[&r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"show_bug","arguments":{"id":"<PREFIX>"}}}"#
            .replace("<PREFIX>", prefix)],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    assert_eq!(lines.len(), 1);
    let text = lines[0]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Bug  :"), "missing Bug line: {text}");
    assert!(text.contains(prefix), "missing prefix: {text}");
    assert!(text.contains("boom_mcp_show"), "missing evidence: {text}");
}

#[test]
fn mcp_show_bug_unknown_id_returns_tool_error_with_hint() {
    let tmp = init_dir();
    run_failing(&tmp, "boom_hint");
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"show_bug","arguments":{"id":"deadbeef"}}}"#,
        ],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    assert_eq!(lines.len(), 1);
    // Tool-level failure: result carries `isError: true`.
    assert_eq!(lines[0]["result"]["isError"], true);
    let text = lines[0]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("no fingerprint") || text.contains("deadbeef"),
        "expected error to mention the id, got: {text}"
    );
    assert!(
        text.contains("hint:"),
        "expected library hint to be surfaced, got: {text}"
    );
}

#[test]
fn mcp_ai_report_text_matches_cli_report_ai() {
    let tmp = init_dir();
    run_failing(&tmp, "boom_parity");
    run_failing(&tmp, "boom_parity_2");
    // Capture the CLI's bytes.
    let cli = driftwatch()
        .args(["report", "--ai"])
        .current_dir(tmp.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("driftwatch report --ai");
    assert!(cli.status.success());
    let expected = String::from_utf8_lossy(&cli.stdout).into_owned();
    // Capture the MCP tool's text.
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"ai_report","arguments":{}}}"#,
        ],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    let actual = lines[0]["result"]["content"][0]["text"].as_str().unwrap();
    // The generated-at timestamp uses `Utc::now()` and so cannot
    // be reproduced exactly between the two invocations. Compare
    // everything else byte-for-byte.
    let normalize = |s: &str| -> String {
        s.lines()
            .map(|l| {
                if l.starts_with("_Generated at ") && l.ends_with('_') {
                    "_Generated at <TS>_".to_string()
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        normalize(actual),
        normalize(&expected),
        "MCP ai_report diverged from `driftwatch report --ai`"
    );
}

#[test]
fn mcp_doctor_status_returns_report() {
    let tmp = init_dir();
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"doctor_status","arguments":{}}}"#,
        ],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    let text = lines[0]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Driftwatch doctor"), "missing header: {text}");
    assert!(text.contains("[PASS]"), "missing pass row: {text}");
    assert!(text.contains("db.open"), "missing db.open check: {text}");
}

#[test]
fn mcp_malformed_line_is_skipped() {
    let tmp = init_dir();
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
            "this is not json",
            r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        ],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    assert_eq!(lines.len(), 2, "got: {lines:?}");
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["id"], 2);
}

#[test]
fn mcp_empty_lines_are_ignored() {
    let tmp = init_dir();
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &["", r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#, ""],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    assert_eq!(lines.len(), 1);
}

#[test]
fn mcp_eof_exits_zero() {
    // No request lines at all: the server reads EOF on stdin and
    // exits 0. The agent never sends an `initialize`; this is
    // the "kill -PIPE the parent" scenario.
    let tmp = init_dir();
    let (_, status) = mcp_roundtrip(tmp.path(), &[]);
    assert!(status.success(), "mcp should exit 0 on EOF");
}

#[test]
fn mcp_session_keeps_serving_after_tool_error() {
    let tmp = init_dir();
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"top_bugs","arguments":{"limit":"oops"}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        ],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["error"]["code"], -32602);
    assert_eq!(lines[1]["id"], 2);
    assert!(lines[1]["result"].is_object());
}

#[test]
fn mcp_additional_properties_rejected() {
    let tmp = init_dir();
    let (out, status) = mcp_roundtrip(
        tmp.path(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"top_bugs","arguments":{"limit":5,"unexpected":true}}}"#,
        ],
    );
    assert!(status.success());
    let lines = parse_lines(&out);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["error"]["code"], -32602);
}

#[test]
fn mcp_help_documents_the_subcommand() {
    let out = driftwatch()
        .args(["mcp", "--help"])
        .output()
        .expect("driftwatch mcp --help");
    assert!(out.status.success(), "mcp --help failed: {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.to_lowercase().contains("stdio"),
        "help text should mention stdio, got: {stdout}"
    );
}
