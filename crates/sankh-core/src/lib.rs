//! Sankh core: everything both `sankh run` and `sankh serve` share, so the
//! CLI and the UI cannot drift apart.

pub mod assert;
pub mod capture;
pub mod collection;
pub mod diag;
pub mod env;
pub mod format;
pub mod http;
pub mod import;
pub mod jq;
pub mod parse;
pub use parse as parser;
pub mod paths;
pub mod redact;
pub mod report;
pub mod request;
pub mod runner;
pub mod scratch;
pub mod select;
pub mod shell;
pub mod workspace;

pub use collection::{Collection, Node};
pub use env::Env;
pub use request::Request;
pub use runner::{Outcome, RequestResult, RunContext, RunOptions};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
