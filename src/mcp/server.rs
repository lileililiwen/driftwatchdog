//! JSON-RPC 2.0 server loop over stdio.
//!
//! Each line on `stdin` is a single JSON-RPC request (or
//! notification). Responses (one per request, none for notifications)
//! are written as single-line JSON on `stdout`. The server opens the
//! project database read-only; any code path that attempts a write
//! fails at the SQLite driver.
//!
//! Failure semantics (per JSON-RPC 2.0):
//! - `-32700` parse error: line was not valid JSON, or did not
//!   satisfy the request envelope. When the `id` is recoverable the
//!   server replies; otherwise the line is logged to `stderr` and
//!   skipped (the server never exits on a bad line).
//! - `-32600` invalid request: envelope was valid but the method
//!   shape was wrong (we currently use `-32602` for this case).
//! - `-32601` method not found.
//! - `-32602` invalid params: missing or wrong-typed arguments.
//! - `-32603` internal error: tool implementation failed.
//!
//! EOF on `stdin` exits cleanly with status 0.

use std::io::{self, BufRead, Write};
use std::path::Path;

use serde_json::Value;

use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::Db;

use super::tools;

/// Protocol version we speak. Matches MCP `2024-11-05`.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

const ERR_PARSE: i32 = -32700;
const ERR_INVALID_PARAMS: i32 = -32602;
const ERR_METHOD_NOT_FOUND: i32 = -32601;

/// Top-level entry point. Wires `stdin`/`stdout` to the loop and
/// exits 0 on EOF. Returns `Err` only for binary plumbing failures
/// (project discovery, DB open, I/O on the loop boundary); every
/// malformed input line is handled inline.
pub fn run(cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open_read_only(&proj.db_path)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let result = run_with_io(&proj, &mut db, stdin.lock(), &mut out);
    let _ = out.flush();
    result
}

/// Test-friendly entry: any `BufRead` + `Write` pair works. The DB is
/// expected to be open read-only so the read-only invariant is
/// preserved regardless of the call site.
pub fn run_with_io<R: BufRead, W: Write>(
    proj: &ProjectRoot,
    db: &mut Db,
    mut input: R,
    output: W,
) -> Result<i32, Error> {
    let mut session = Session::new(output);
    let mut buf = String::new();
    loop {
        buf.clear();
        let n = input.read_line(&mut buf).map_err(Error::IoBare)?;
        if n == 0 {
            return Ok(0);
        }
        let line = buf.trim_end_matches(['\n', '\r']);
        if line.is_empty() {
            continue;
        }
        session.handle_line(line, proj, db);
    }
}

/// Server session. Owns the output writer and the request counter
/// (used to fabricate an `id` when the client omits one on a
/// notification that turns out to be malformed).
struct Session<W: Write> {
    output: W,
    next_id: i64,
}

impl<W: Write> Session<W> {
    fn new(output: W) -> Self {
        Self { output, next_id: 1 }
    }

    fn alloc_id(&mut self) -> Value {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        Value::Number(serde_json::Number::from(id))
    }

    fn handle_line(&mut self, line: &str, proj: &ProjectRoot, db: &mut Db) {
        match serde_json::from_str::<Value>(line) {
            Ok(v) => {
                let id_opt = v.get("id").cloned();
                let method_opt = v.get("method").and_then(Value::as_str).map(str::to_owned);
                match method_opt {
                    Some(method) => {
                        let params = v.get("params").cloned().unwrap_or(Value::Null);
                        // Notifications (no id) get no response.
                        // Requests get a response that echoes their
                        // id (or a synthesized one when the client
                        // omitted it, which is non-standard but
                        // forgiving for malformed input).
                        let (is_notification, id) = match id_opt {
                            Some(id) => (false, id),
                            None => (true, self.alloc_id()),
                        };
                        let resp = dispatch(Some(&id), &method, params, proj, db);
                        if !is_notification {
                            if let Some(resp) = resp {
                                self.write_response(&resp);
                            }
                        }
                    }
                    None => {
                        // Envelope is valid JSON but not a request.
                        // If we recovered an id, reply with a parse
                        // error; otherwise log to stderr and skip.
                        if let Some(id) = id_opt {
                            let resp =
                                error_response(id, ERR_PARSE, "missing 'method' field".into());
                            self.write_response(&resp);
                        } else {
                            eprintln!("driftwatch mcp: dropping line without method: {line}");
                        }
                    }
                }
            }
            Err(e) => {
                // The line was not valid JSON. We cannot recover the
                // id (there is none), so we follow the
                // malformed-line policy: log to stderr and keep
                // serving. EOF will eventually exit 0.
                eprintln!("driftwatch mcp: parse error: {e}");
            }
        }
    }

    fn write_response(&mut self, resp: &Value) {
        if let Ok(s) = serde_json::to_string(resp) {
            let _ = writeln!(self.output, "{s}");
            let _ = self.output.flush();
        }
    }
}

/// Dispatch a JSON-RPC request to the matching handler. Returns
/// `Some(response)` for a request that must be written back to the
/// client, or `None` for a notification (no response). The
/// `id` argument is the request id (or a synthesized one for a
/// malformed-but-recoverable line); it is echoed in every response.
fn dispatch(
    id: Option<&Value>,
    method: &str,
    params: Value,
    proj: &ProjectRoot,
    db: &mut Db,
) -> Option<Value> {
    let id = id.cloned().unwrap_or(Value::Null);
    match method {
        "initialize" => Some(success_response(id, handle_initialize(&params))),
        "notifications/initialized" => None,
        "ping" => Some(success_response(id, serde_json::json!({}))),
        "tools/list" => Some(success_response(
            id,
            serde_json::json!({ "tools": tools::tool_schemas() }),
        )),
        "tools/call" => match handle_tools_call(&params, proj, db) {
            ToolCallOutcome::Ok(result) => Some(success_response(id, result)),
            ToolCallOutcome::Err { code, message } => Some(error_response(id, code, message)),
        },
        _ => Some(error_response(
            id,
            ERR_METHOD_NOT_FOUND,
            format!("method not found: {method}"),
        )),
    }
}

fn success_response(id: Value, result: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn handle_initialize(params: &Value) -> Value {
    // Echo the client's protocolVersion when it matches what we
    // support, otherwise negotiate down to our supported version.
    let client_version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("");
    let protocol_version = if client_version == PROTOCOL_VERSION {
        client_version
    } else {
        PROTOCOL_VERSION
    };
    serde_json::json!({
        "protocolVersion": protocol_version,
        "serverInfo": {
            "name": "driftwatchdog",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": {
            "tools": {}
        },
    })
}

/// Result of dispatching a `tools/call` request. The dispatcher
/// wraps the success path in a JSON-RPC `result` envelope and the
/// error path in a JSON-RPC `error` envelope.
enum ToolCallOutcome {
    Ok(Value),
    Err { code: i32, message: String },
}

fn handle_tools_call(params: &Value, proj: &ProjectRoot, db: &mut Db) -> ToolCallOutcome {
    let name = match params.get("name").and_then(Value::as_str) {
        Some(n) => n.to_string(),
        None => {
            return ToolCallOutcome::Err {
                code: ERR_INVALID_PARAMS,
                message: "tools/call requires a string 'name'".into(),
            };
        }
    };
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
    match tools::call(&name, arguments, proj, db) {
        Ok(content) => ToolCallOutcome::Ok(serde_json::json!({ "content": content })),
        Err(tools::ToolError::Args(msg)) => ToolCallOutcome::Err {
            code: ERR_INVALID_PARAMS,
            message: msg,
        },
        Err(tools::ToolError::Unknown) => ToolCallOutcome::Err {
            code: ERR_METHOD_NOT_FOUND,
            message: format!("tool not found: {name}"),
        },
        Err(tools::ToolError::Run(e)) => {
            // Surface the underlying error verbatim, plus the
            // library `hint:` so the agent gets the same
            // remediation the CLI prints. Returned as a tool
            // result with `isError: true` per MCP convention; the
            // JSON-RPC envelope itself is still a success.
            let mut msg = e.to_string();
            if let Some(hint) = e.hint() {
                msg.push_str(&format!("\nhint: {hint}"));
            }
            ToolCallOutcome::Ok(serde_json::json!({
                "content": [{"type": "text", "text": msg}],
                "isError": true,
            }))
        }
    }
}

fn error_response(id: Value, code: i32, message: String) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ReportArgs;
    use crate::commands::report_ai::render_ai_for_mcp;
    use std::io::Cursor;

    fn fresh_db() -> (tempfile::TempDir, ProjectRoot, Db) {
        let tmp = tempfile::tempdir().unwrap();
        let proj = ProjectRoot::at(tmp.path());
        std::fs::create_dir_all(&proj.state_dir).unwrap();
        let mut db = Db::open(&proj.db_path).unwrap();
        crate::storage::migrations::apply(db.conn_mut()).unwrap();
        (tmp, proj, db)
    }

    fn run_lines(proj: &ProjectRoot, db: &mut Db, lines: &[&str]) -> Vec<String> {
        let input: Vec<u8> = lines.join("\n").into_bytes();
        let input: Vec<u8> = if input.is_empty() {
            input
        } else {
            // read_line expects a trailing newline on every line.
            let mut v = input;
            v.push(b'\n');
            v
        };
        let mut out = Vec::new();
        run_with_io(proj, db, Cursor::new(input), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        text.lines().map(str::to_owned).collect()
    }

    fn parse(line: &str) -> Value {
        serde_json::from_str(line).unwrap()
    }

    #[test]
    fn initialize_returns_server_info_and_capabilities() {
        let (_tmp, proj, mut db) = fresh_db();
        let lines = run_lines(
            &proj,
            &mut db,
            &[
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
            ],
        );
        assert_eq!(lines.len(), 1);
        let v = parse(&lines[0]);
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 1);
        assert_eq!(v["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(v["result"]["serverInfo"]["name"], "driftwatchdog");
        assert!(v["result"]["serverInfo"]["version"].is_string());
        assert!(v["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn initialize_negotiates_down_when_client_version_unsupported() {
        let (_tmp, proj, mut db) = fresh_db();
        let lines = run_lines(
            &proj,
            &mut db,
            &[
                r#"{"jsonrpc":"2.0","id":7,"method":"initialize","params":{"protocolVersion":"2099-01-01"}}"#,
            ],
        );
        let v = parse(&lines[0]);
        assert_eq!(v["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn notifications_initialized_produces_no_response() {
        let (_tmp, proj, mut db) = fresh_db();
        let lines = run_lines(
            &proj,
            &mut db,
            &[r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#],
        );
        assert!(lines.is_empty(), "got: {lines:?}");
    }

    #[test]
    fn tools_list_includes_all_four_read_only_tools() {
        let (_tmp, proj, mut db) = fresh_db();
        let lines = run_lines(
            &proj,
            &mut db,
            &[r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#],
        );
        let v = parse(&lines[0]);
        let names: Vec<&str> = v["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        for name in ["top_bugs", "show_bug", "ai_report", "doctor_status"] {
            assert!(names.contains(&name), "missing tool {name}: {names:?}");
        }
    }

    #[test]
    fn tools_list_schemas_have_additional_properties_false() {
        let (_tmp, proj, mut db) = fresh_db();
        let lines = run_lines(
            &proj,
            &mut db,
            &[r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#],
        );
        let v = parse(&lines[0]);
        for tool in v["result"]["tools"].as_array().unwrap() {
            let schema = &tool["inputSchema"];
            assert_eq!(
                schema["additionalProperties"], false,
                "tool {} schema is not closed: {}",
                tool["name"], schema,
            );
        }
    }

    #[test]
    fn unknown_method_returns_minus_32601() {
        let (_tmp, proj, mut db) = fresh_db();
        let lines = run_lines(
            &proj,
            &mut db,
            &[r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#],
        );
        let v = parse(&lines[0]);
        assert_eq!(v["id"], 3);
        assert_eq!(v["error"]["code"], ERR_METHOD_NOT_FOUND);
    }

    #[test]
    fn tools_call_with_missing_name_returns_minus_32602() {
        let (_tmp, proj, mut db) = fresh_db();
        let lines = run_lines(
            &proj,
            &mut db,
            &[r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{}}"#],
        );
        let v = parse(&lines[0]);
        assert_eq!(v["error"]["code"], ERR_INVALID_PARAMS);
    }

    #[test]
    fn malformed_line_is_skipped_with_stderr_diagnostic() {
        let (_tmp, proj, mut db) = fresh_db();
        let lines = run_lines(
            &proj,
            &mut db,
            &[
                r#"{"jsonrpc":"2.0","id":5,"method":"ping"}"#,
                "this is not json",
                r#"{"jsonrpc":"2.0","id":6,"method":"ping"}"#,
            ],
        );
        assert_eq!(lines.len(), 2, "got: {lines:?}");
        assert_eq!(parse(&lines[0])["id"], 5);
        assert_eq!(parse(&lines[1])["id"], 6);
    }

    #[test]
    fn empty_lines_are_ignored() {
        let (_tmp, proj, mut db) = fresh_db();
        let lines = run_lines(
            &proj,
            &mut db,
            &["", r#"{"jsonrpc":"2.0","id":5,"method":"ping"}"#, ""],
        );
        assert_eq!(lines.len(), 1);
        assert_eq!(parse(&lines[0])["id"], 5);
    }

    #[test]
    fn session_keeps_serving_after_tool_argument_error() {
        let (_tmp, proj, mut db) = fresh_db();
        let lines = run_lines(
            &proj,
            &mut db,
            &[
                // `limit` is declared as integer; a string value
                // must be rejected with -32602 invalid params.
                r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"top_bugs","arguments":{"limit":"bad type"}}}"#,
                r#"{"jsonrpc":"2.0","id":11,"method":"ping"}"#,
            ],
        );
        // The first call fails (bad type); the second succeeds.
        assert_eq!(lines.len(), 2);
        let v0 = parse(&lines[0]);
        assert_eq!(v0["error"]["code"], ERR_INVALID_PARAMS);
        let v1 = parse(&lines[1]);
        assert_eq!(v1["id"], 11);
        assert!(v1["result"].is_object());
    }

    #[test]
    fn ai_report_bytes_match_render_ai_for_mcp() {
        // The MCP tool's `ai_report` text must be byte-identical
        // to the CLI's `driftwatch report --ai` output, modulo
        // the generated-at timestamp (which uses
        // `chrono::Utc::now()` and so cannot be reproduced
        // exactly across two invocations). The comparison
        // normalizes that single line.
        let (_tmp, proj, mut db) = fresh_db();
        let args = ReportArgs {
            limit: 5,
            days: 30,
            tag: None,
            ai: true,
        };
        let expected = render_ai_for_mcp(&args, &proj, &mut db).unwrap();
        let lines = run_lines(
            &proj,
            &mut db,
            &[
                r#"{"jsonrpc":"2.0","id":42,"method":"tools/call","params":{"name":"ai_report","arguments":{}}}"#,
            ],
        );
        assert_eq!(lines.len(), 1);
        let v = parse(&lines[0]);
        let text = v["result"]["content"][0]["text"].as_str().unwrap();
        // Normalize the `_Generated at <RFC3339>_` line on both
        // sides, then compare the rest byte-for-byte.
        let normalize = |s: &str| -> String {
            s.lines()
                .map(|l| {
                    if l.starts_with("_Generated at ") && l.ends_with("_") {
                        "_Generated at <TS>_".to_string()
                    } else {
                        l.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(normalize(text), normalize(&expected));
    }

    #[test]
    fn read_only_connection_rejects_writes_at_driver_level() {
        // The contract: a write on a connection opened via
        // `open_read_only` fails at the driver with a read-only
        // error, not at the type level. This guards the
        // `SQLITE_OPEN_READ_ONLY` invariant for the MCP path.
        let tmp = tempfile::tempdir().unwrap();
        let proj = ProjectRoot::at(tmp.path());
        std::fs::create_dir_all(&proj.state_dir).unwrap();
        {
            let mut db = Db::open(&proj.db_path).unwrap();
            crate::storage::migrations::apply(db.conn_mut()).unwrap();
        }
        let db = Db::open_read_only(&proj.db_path).unwrap();
        let err = db
            .conn()
            .execute(
                "INSERT INTO runs (program, argv, cwd, started_at) VALUES ('x', '[]', '/tmp', '2026-01-01T00:00:00Z')",
                [],
            )
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("read-only")
                || msg.to_lowercase().contains("readonly")
                || msg.to_lowercase().contains("read only"),
            "expected read-only rejection, got: {msg}"
        );
    }
}
