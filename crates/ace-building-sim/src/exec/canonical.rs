//! Legacy host API over the canonical engine. No behavior or fallback lives here.
use crate::model::Block;
use cdl_blocks::Registry;
use cdl_exec::{PortHandle, Program, Scalar, State};
use cdl_model::DataType;
use serde_json::{json, Value};
use std::collections::HashMap;

type ExecutionResult = (HashMap<String, Value>, serde_json::Map<String, Value>);

#[derive(Clone)]
pub struct CompiledExecutor {
    #[cfg(test)]
    program: Program,
    state: State,
    inputs: Vec<(String, PortHandle)>,
    outputs: Vec<(String, PortHandle)>,
    traces: Vec<(String, PortHandle)>,
}
fn json_scalar(value: Scalar) -> Value {
    match value {
        Scalar::Real(v) => json!(v),
        Scalar::Integer(v) => json!(v),
        Scalar::Boolean(v) => json!(v),
    }
}
impl CompiledExecutor {
    pub fn try_new(graph: &Block) -> Result<Option<Self>, String> {
        Self::new(graph).map(Some)
    }
    pub(crate) fn new(graph: &Block) -> Result<Self, String> {
        let source = serde_json::to_value(graph).map_err(|e| e.to_string())?;
        let document = cdl_compat::from_value(&source, Default::default()).map_err(|e| {
            e.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        Self::from_document(&document)
    }
    pub(crate) fn from_document(document: &cdl_model::Document) -> Result<Self, String> {
        let graph: Block =
            serde_json::from_value(cdl_compat::to_value(document).map_err(|e| format!("{e:?}"))?)
                .map_err(|e| e.to_string())?;
        let program = Program::compile(document, &Registry::new(document.library.clone()))
            .map_err(|e| e.to_string())?;
        let inputs = graph
            .inputs
            .iter()
            .map(|p| {
                Ok((
                    p.name.clone(),
                    program.input(&p.name).map_err(|e| e.to_string())?,
                ))
            })
            .collect::<Result<_, String>>()?;
        let outputs = graph
            .outputs
            .iter()
            .map(|p| {
                Ok((
                    p.name.clone(),
                    program.output(&p.name).map_err(|e| e.to_string())?,
                ))
            })
            .collect::<Result<_, String>>()?;
        let state = program.instantiate();
        Ok(Self {
            #[cfg(test)]
            program,
            state,
            inputs,
            outputs,
            traces: vec![],
        })
    }
    #[cfg(test)]
    pub(crate) fn set_traces(&mut self, targets: &[String]) -> Result<(), String> {
        self.traces = targets
            .iter()
            .map(|name| {
                self.program
                    .trace(name)
                    .map(|h| (name.clone(), h))
                    .map_err(|_| format!("Selected graph signal {name} has no value"))
            })
            .collect::<Result<_, _>>()?;
        Ok(())
    }
    pub fn execute(
        &mut self,
        inputs: &HashMap<String, Value>,
        dt: f64,
        record_traces: bool,
    ) -> Result<ExecutionResult, String> {
        for (name, handle) in &self.inputs {
            let value = inputs
                .get(name)
                .ok_or_else(|| format!("Missing input {name}"))?;
            let value = match handle.data_type() {
                DataType::Boolean => Scalar::Boolean(
                    value
                        .as_bool()
                        .or_else(|| value.as_f64().map(|v| v != 0.0))
                        .ok_or_else(|| format!("Invalid input {name}"))?,
                ),
                DataType::Integer => {
                    let number = value
                        .as_f64()
                        .ok_or_else(|| format!("Invalid input {name}"))?;
                    if !number.is_finite() || number.fract() != 0.0 {
                        return Err(format!("Invalid Integer input {name}"));
                    }
                    Scalar::Integer(number as i64)
                }
                _ => Scalar::Real(
                    value
                        .as_f64()
                        .or_else(|| value.as_bool().map(|v| if v { 1.0 } else { 0.0 }))
                        .ok_or_else(|| format!("Invalid input {name}"))?,
                ),
            };
            self.state.set(handle, value).map_err(|e| e.to_string())?;
        }
        self.state.step(dt).map_err(|e| e.to_string())?;
        let outputs = self
            .outputs
            .iter()
            .map(|(name, handle)| {
                (
                    name.clone(),
                    json_scalar(self.state.get(handle).expect("compiled handle")),
                )
            })
            .collect();
        let traces = if record_traces {
            self.traces
                .iter()
                .map(|(name, handle)| {
                    let number = match self.state.get(handle).expect("compiled handle") {
                        Scalar::Real(v) => v,
                        Scalar::Integer(v) => v as f64,
                        Scalar::Boolean(v) => {
                            if v {
                                1.0
                            } else {
                                0.0
                            }
                        }
                    };
                    (name.clone(), json!(number))
                })
                .collect()
        } else {
            serde_json::Map::new()
        };
        Ok((outputs, traces))
    }
}

/// Backward-compatible call shape using the same canonical engine.
#[derive(Default)]
pub struct Executor {
    programs: HashMap<String, CompiledExecutor>,
}
impl Executor {
    pub fn execute(
        &mut self,
        graph: &Block,
        inputs: &HashMap<String, Value>,
        dt: f64,
        path: &str,
    ) -> Result<HashMap<String, Value>, String> {
        let key = format!("{path}{}", graph.name);
        if !self.programs.contains_key(&key) {
            self.programs
                .insert(key.clone(), CompiledExecutor::new(graph)?);
        }
        self.programs
            .get_mut(&key)
            .unwrap()
            .execute(inputs, dt, false)
            .map(|(outputs, _)| outputs)
    }
}
