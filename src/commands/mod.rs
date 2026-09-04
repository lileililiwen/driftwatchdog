//! CLI command orchestration. Each subcommand has a single public function
//! that returns a process exit code (or an `Error` for the binary to surface).

pub mod check;
pub mod doctor;
pub mod export;
pub mod gc;
pub mod link;
pub mod list;
pub mod report;
pub mod report_ai;
pub mod run;
pub mod show;
pub mod top;
pub mod unlink;

pub use check::check as check_cmd;
pub use doctor::doctor as doctor_cmd;
pub use export::export as export_cmd;
pub use gc::gc as gc_cmd;
pub use link::link as link_cmd;
pub use list::list as list_cmd;
pub use report::report as report_cmd;
pub use report_ai::render_ai as report_ai_cmd;
pub use run::run as run_cmd;
pub use show::show as show_cmd;
pub use top::top as top_cmd;
pub use unlink::unlink as unlink_cmd;
