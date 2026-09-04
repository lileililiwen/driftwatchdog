//! Project-level modules: root discovery, configuration, and `init` orchestration.

pub mod config;
pub mod git;

mod init;
mod root;

pub use init::init;
pub use root::ProjectRoot;
