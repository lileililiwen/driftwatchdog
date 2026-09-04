//! Project-level modules: root discovery, configuration, and `init` orchestration.

pub mod config;
pub mod git;
pub mod root;

mod init;

pub use init::init;
pub use root::ProjectRoot;
