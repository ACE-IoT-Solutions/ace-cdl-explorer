//! Native fixed-topology building simulator over the canonical CDL executor.
#![doc = include_str!("../README.md")]
mod exec;
mod model;
mod sim;
mod validate;
pub use exec::{CompiledExecutor, Executor};
pub use model::{Block, Connection, Connector, Equation, Parameter};
pub use sim::*;
pub use validate::validate;
/// Parse a legacy project graph through the canonical compatibility adapter.
pub fn parse_legacy_json(json: &str) -> Result<Block, Vec<String>> {
    model::parse_json(json)
}
