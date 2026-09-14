//! MCP (Model Context Protocol) server over stdio.
//!
//! Exposes a read-only tool layer (`top_bugs`, `show_bug`, `ai_report`,
//! `doctor_status`) so external agents can query local Driftwatch
//! state through a typed contract instead of parsing CLI output.
//! Transport: newline-delimited JSON-RPC 2.0 on stdin/stdout. The
//! SQLite database is opened read-only so a write attempt fails at
//! the driver level.

pub mod server;
pub mod tools;
