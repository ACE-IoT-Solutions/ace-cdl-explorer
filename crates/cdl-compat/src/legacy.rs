//! Legacy JSON data transfer types retained for host API compatibility.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Legacy JSON Connector.
pub struct Connector {
    /// Legacy JSON name.
    pub name: String,
    #[serde(rename = "type")]
    /// Legacy JSON kind.
    pub kind: String,
    #[serde(default)]
    /// Legacy JSON unit.
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Legacy JSON Parameter.
pub struct Parameter {
    /// Legacy JSON name.
    pub name: String,
    /// Legacy JSON value.
    pub value: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Legacy JSON Equation.
pub struct Equation {
    /// Legacy JSON lhs.
    pub lhs: String,
    /// Legacy JSON rhs.
    pub rhs: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Legacy JSON Connection.
pub struct Connection {
    #[serde(default)]
    /// Legacy JSON from_block.
    pub from_block: String,
    #[serde(default)]
    /// Legacy JSON from_output.
    pub from_output: String,
    #[serde(default)]
    /// Legacy JSON to_block.
    pub to_block: String,
    #[serde(default)]
    /// Legacy JSON to_input.
    pub to_input: String,
    #[serde(default, rename = "from")]
    /// Legacy JSON from_path.
    pub from_path: String,
    #[serde(default, rename = "to")]
    /// Legacy JSON to_path.
    pub to_path: String,
}

impl Connection {
    /// Legacy JSON source.
    pub fn source(&self) -> (String, String) {
        if self.from_path.is_empty() {
            (self.from_block.clone(), self.from_output.clone())
        } else {
            split_path(&self.from_path)
        }
    }
    /// Legacy JSON target.
    pub fn target(&self) -> (String, String) {
        if self.to_path.is_empty() {
            (self.to_block.clone(), self.to_input.clone())
        } else {
            split_path(&self.to_path)
        }
    }
}

/// Split a simplified-JSON child connector path or boundary connector name.
pub fn split_path(path: &str) -> (String, String) {
    match path.split_once('.') {
        Some((a, b)) => (a.into(), b.into()),
        None => (path.into(), String::new()),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Legacy JSON Block.
pub struct Block {
    /// Legacy JSON name.
    pub name: String,
    #[serde(alias = "type")]
    /// Legacy JSON block_type.
    pub block_type: String,
    #[serde(default)]
    /// Legacy JSON category.
    pub category: Option<String>,
    #[serde(default)]
    /// Legacy JSON description.
    pub description: Option<String>,
    #[serde(default)]
    /// Legacy JSON inputs.
    pub inputs: Vec<Connector>,
    #[serde(default)]
    /// Legacy JSON outputs.
    pub outputs: Vec<Connector>,
    #[serde(default)]
    /// Legacy JSON parameters.
    pub parameters: Vec<Parameter>,
    #[serde(default)]
    /// Legacy JSON equations.
    pub equations: Vec<Equation>,
    #[serde(default)]
    /// Legacy JSON blocks.
    pub blocks: Vec<Block>,
    #[serde(default)]
    /// Legacy JSON connections.
    pub connections: Vec<Connection>,
}
