//! CLI command orchestration. Each subcommand has a single public function
//! that returns a process exit code (or an `Error` for the binary to surface).

pub mod list;
pub mod run;
pub mod top;

pub use list::list as list_cmd;
pub use run::run as run_cmd;
pub use top::top as top_cmd;
