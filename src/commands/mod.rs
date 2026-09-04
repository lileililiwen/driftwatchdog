//! CLI command orchestration. Each subcommand has a single public function
//! that returns a process exit code (or an `Error` for the binary to surface).

pub mod gc;
pub mod list;
pub mod report;
pub mod run;
pub mod show;
pub mod top;

pub use gc::gc as gc_cmd;
pub use list::list as list_cmd;
pub use report::report as report_cmd;
pub use run::run as run_cmd;
pub use show::show as show_cmd;
pub use top::top as top_cmd;
