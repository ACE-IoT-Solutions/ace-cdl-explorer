use ace_building_sim::{Block, Executor};
use cdl_model::{Diagnostic, DiagnosticCode};
use serde_json::Value;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn validate_json(graph_json: &str) -> String {
    let errors = match crate::format_api::document(graph_json) {
        Ok(document) => {
            let errors = cdl_model::validate(&document);
            if !errors.is_empty() {
                errors
            } else {
                match cdl_exec::Program::compile(
                    &document,
                    &cdl_blocks::Registry::new(document.library.clone()),
                ) {
                    Ok(_) => Vec::new(),
                    Err(cdl_exec::Error::Diagnostics(errors)) => errors,
                    Err(error) => vec![Diagnostic::new(
                        "",
                        DiagnosticCode::InvalidValue,
                        error.to_string(),
                    )],
                }
            }
        }
        Err(errors) => errors,
    };
    serde_json::to_string(&errors).expect("serializable diagnostics")
}

#[wasm_bindgen]
pub fn execute_json(graph_json: &str, inputs_json: &str, dt: f64) -> Result<String, JsValue> {
    let graph: Block = ace_building_sim::parse_legacy_json(graph_json)
        .map_err(|errors| JsValue::from_str(&errors.join("; ")))?;
    let inputs: HashMap<String, Value> =
        serde_json::from_str(inputs_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let result = Executor::default()
        .execute(&graph, &inputs, dt, "")
        .map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}
