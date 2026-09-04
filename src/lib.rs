//! Library entrypoint exposing the public modules for tests and future binaries.
//!
//! The CLI binary in `src/main.rs` is intentionally thin: it parses arguments
//! and dispatches to the library.

pub mod cli;
pub mod error;
pub mod project;
pub mod repo;
pub mod storage;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
