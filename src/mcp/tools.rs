//! MCP tool implementations.
//!
//! Each tool reuses an existing Driftwatch builder (`Bugs::top`,
//! `report_ai::render_ai_for_mcp`, `doctor::run`, `commands::show`)
//! so the MCP surface can never drift from CLI output. The result
//! of every tool is a single text content block so agents can read
//! prose or machine-parse JSON from the same payload.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::cli::{ReportArgs, ShowArgs};
use crate::commands::report_ai::render_ai_for_mcp;
use crate::commands::show_render;
use crate::doctor;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{bugs::Bugs, Db};

/// One content block in an MCP `tools/call` result. Today every tool
/// returns a single `text` block; the struct keeps the door open
/// for image/resource blocks without a breaking change.
#[derive(Debug, Clone, Serialize)]
pub struct TextBlock {
    pub r#type: &'static str,
    pub text: String,
}

impl TextBlock {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            r#type: "text",
            text: text.into(),
        }
    }
}

/// Tool-level errors. The dispatch layer maps each variant to the
/// right JSON-RPC shape (envelope error vs `isError: true`).
#[derive(Debug)]
pub enum ToolError {
    /// Caller supplied a malformed argument object. Maps to
    /// JSON-RPC `-32602`.
    Args(String),
    /// Caller named a tool the server does not implement. Maps to
    /// JSON-RPC `-32601`.
    Unknown,
    /// The tool itself failed (e.g. the requested bug id was not
    /// found). Maps to a `tools/call` result with `isError: true`.
    Run(Error),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::Args(m) => write!(f, "{m}"),
            ToolError::Unknown => write!(f, "unknown tool"),
            ToolError::Run(e) => write!(f, "{e}"),
        }
    }
}

/// Per-tool argument structs. Each `Deserialize` impl rejects extra
/// fields via the `additionalProperties: false` schema and `?`
/// on the missing field.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopBugsArgs {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_days")]
    pub days: i64,
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShowBugArgs {
    pub id: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiReportArgs {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_days")]
    pub days: i64,
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoctorStatusArgs {}

fn default_limit() -> usize {
    20
}
fn default_days() -> i64 {
    30
}

fn parse_args<'a, T: Deserialize<'a>>(
    args: &'a Value,
    tool: &str,
    schema: &str,
) -> Result<T, ToolError> {
    if args.is_null() {
        return T::deserialize(Value::Object(Default::default())).map_err(|e| {
            ToolError::Args(format!(
                "{tool}: invalid arguments: {e}\nexpected: {schema}"
            ))
        });
    }
    T::deserialize(args).map_err(|e| {
        ToolError::Args(format!(
            "{tool}: invalid arguments: {e}\nexpected: {schema}"
        ))
    })
}

/// Tool schemas returned from `tools/list`. Closed inputs (no extra
/// properties) so a misnamed key is rejected at the boundary.
pub fn tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "top_bugs",
            "description": "List recurring failures ordered by occurrence count. Returns a JSON array of rows (hash, summary, count, first_seen_at, last_seen_at) that matches the driftwatch top data shape.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "default": 20, "description": "Maximum number of rows."},
                    "days": {"type": "integer", "default": 30, "description": "Restrict to fingerprints last seen within this many days."},
                    "tag": {"type": "string", "description": "Restrict to fingerprints whose occurrences include a run with this tag."}
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "show_bug",
            "description": "Render a single recurring bug by hash prefix (8+ hex chars preferred), numeric fingerprints.id, or `id:<n>`. Returns the same text output as `driftwatch show`.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Hash prefix, numeric id, or `id:<n>` form."}
                },
                "required": ["id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "ai_report",
            "description": "Render the AI-oriented Markdown context report. Returns bytes identical to `driftwatch report --ai`.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "default": 20, "description": "Maximum fingerprints to include."},
                    "days": {"type": "integer", "default": 30, "description": "Restrict to fingerprints last seen within this many days."},
                    "tag": {"type": "string", "description": "Restrict to fingerprints whose occurrences include a run with this tag."}
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "doctor_status",
            "description": "Run the diagnostic suite. Returns the same human-readable report as `driftwatch doctor`.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        }),
    ]
}

/// Dispatch one tool call. Returns the content blocks on success,
/// or a [`ToolError`] the JSON-RPC layer maps to the right shape.
pub fn call(
    name: &str,
    args: Value,
    proj: &ProjectRoot,
    db: &mut Db,
) -> Result<Vec<TextBlock>, ToolError> {
    match name {
        "top_bugs" => {
            let a: TopBugsArgs = parse_args(
                &args,
                "top_bugs",
                "{limit?: usize, days?: i64, tag?: string}",
            )?;
            top_bugs(a, db)
        }
        "show_bug" => {
            let a: ShowBugArgs = parse_args(&args, "show_bug", "{id: string}")?;
            show_bug(a, db)
        }
        "ai_report" => {
            let a: AiReportArgs = parse_args(
                &args,
                "ai_report",
                "{limit?: usize, days?: i64, tag?: string}",
            )?;
            ai_report(a, proj, db)
        }
        "doctor_status" => {
            let _a: DoctorStatusArgs = parse_args(&args, "doctor_status", "{}")?;
            doctor_status(proj.root())
        }
        _ => Err(ToolError::Unknown),
    }
}

fn top_bugs(a: TopBugsArgs, db: &mut Db) -> Result<Vec<TextBlock>, ToolError> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(a.days)).to_rfc3339();
    let tag_like = a.tag.as_deref().map(crate::repo::tag_like_pattern);
    let rows = Bugs::new(db)
        .top(a.limit, &cutoff, tag_like.as_deref())
        .map_err(ToolError::Run)?;
    let payload: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "hash": r.hash,
                "summary": r.summary,
                "count": r.count,
                "first_seen_at": r.first_seen_at,
                "last_seen_at": r.last_seen_at,
            })
        })
        .collect();
    let text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "[]".to_string());
    Ok(vec![TextBlock::new(text)])
}

fn show_bug(a: ShowBugArgs, db: &mut Db) -> Result<Vec<TextBlock>, ToolError> {
    let args = ShowArgs { bug_id: a.id };
    let out = show_render(db, &args.bug_id).map_err(ToolError::Run)?;
    Ok(vec![TextBlock::new(out)])
}

fn ai_report(
    a: AiReportArgs,
    proj: &ProjectRoot,
    db: &mut Db,
) -> Result<Vec<TextBlock>, ToolError> {
    let args = ReportArgs {
        limit: a.limit,
        days: a.days,
        tag: a.tag,
        ai: true,
    };
    let md = render_ai_for_mcp(&args, proj, db).map_err(ToolError::Run)?;
    Ok(vec![TextBlock::new(md)])
}

fn doctor_status(cwd: &Path) -> Result<Vec<TextBlock>, ToolError> {
    let report = doctor::run(cwd).map_err(ToolError::Run)?;
    Ok(vec![TextBlock::new(report.render())])
}
