//! Ported elementary/composite corpus. The Explorer enables equations explicitly.
#![cfg(feature = "equations")]

// Include the actual legacy host adapter so these tests exercise Program/State
// through the former call shape without a dependency on wasm or the simulator.
// This unpublished harness owns the repository-level host boundary checks.
#[path = "../../ace-building-sim/src/exec/canonical.rs"]
#[allow(dead_code)] // The included host adapter also exposes unrelated legacy API methods.
mod host_adapter;
mod model {
    pub use cdl_compat::legacy::Block;
}
use cdl_compat::legacy::{Block, Connection, Connector, Parameter};
use cdl_runtime_oracle::Executor;
use host_adapter::CompiledExecutor;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

fn param(block: &Block, name: &str, fallback: f64) -> f64 {
    block
        .parameters
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.value.as_f64().unwrap_or(0.0))
        .unwrap_or(fallback)
}
fn validate(block: &Block) -> Vec<String> {
    let source = serde_json::to_value(block).unwrap();
    let document = match cdl_compat::from_value(&source, Default::default()) {
        Ok(document) => document,
        Err(errors) => return errors.into_iter().map(|e| e.to_string()).collect(),
    };
    match cdl_exec::Program::compile(
        &document,
        &cdl_blocks::Registry::new(document.library.clone()),
    ) {
        Ok(_) => vec![],
        Err(cdl_exec::Error::Diagnostics(errors)) => {
            errors.into_iter().map(|e| e.to_string()).collect()
        }
        Err(error) => vec![error.to_string()],
    }
}
#[path = "legacy/cases_00.rs"]
mod cases_00;
#[path = "legacy/cases_01.rs"]
mod cases_01;
#[path = "legacy/cases_02.rs"]
mod cases_02;
#[path = "legacy/cases_03.rs"]
mod cases_03;
