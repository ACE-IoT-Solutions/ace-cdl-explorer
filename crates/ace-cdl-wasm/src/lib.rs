//! Browser boundary: wasm-bindgen exports and Arrow IPC encoding.
mod arrow_out;
mod editor_catalog;
mod format_api;
pub use format_api::{aligned_step, block_catalog, cxf_export, cxf_import};
mod wasm_api;
use ace_building_sim::{SimError, Simulation as NativeSimulation};
pub use wasm_api::{execute_json, validate_json};
use wasm_bindgen::prelude::*;
fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[wasm_bindgen]
pub struct Simulation {
    inner: NativeSimulation,
}
#[wasm_bindgen]
impl Simulation {
    #[wasm_bindgen(constructor)]
    pub fn new(graph_json: &str, weather_json: &str, scenario_json: &str) -> Result<Self, JsValue> {
        Self::native(graph_json, weather_json, scenario_json).map_err(js_error)
    }
    pub fn step_chunk(&mut self, count: usize) -> Result<String, JsValue> {
        self.chunk_json(count).map_err(js_error)
    }
    pub fn step_chunk_arrow(&mut self, count: usize) -> Result<Vec<u8>, JsValue> {
        self.chunk_arrow(count).map_err(js_error)
    }
    pub fn progress(&self) -> usize {
        self.inner.progress()
    }
    pub fn total(&self) -> usize {
        self.inner.total()
    }
    pub fn done(&self) -> bool {
        self.inner.done()
    }
    pub fn effective_step_seconds(&self) -> usize {
        self.inner.effective_step_seconds()
    }
    pub fn summary_json(&self) -> String {
        serde_json::to_string(&self.inner.summary()).expect("finite totals")
    }
    pub fn set_traces(&mut self, targets_json: &str) -> Result<(), JsValue> {
        let targets = serde_json::from_str(targets_json).map_err(js_error)?;
        self.inner.set_traces(targets).map_err(js_error)
    }
}
impl Simulation {
    fn native(graph: &str, weather: &str, scenario: &str) -> Result<Self, SimError> {
        let document = cdl_compat::from_json(graph, Default::default())
            .map_err(|e| SimError(format!("{e:?}")))?;
        Ok(Self {
            inner: NativeSimulation::new(
                document,
                serde_json::from_str(weather).map_err(|e| SimError(e.to_string()))?,
                serde_json::from_str(scenario).map_err(|e| SimError(e.to_string()))?,
            )?,
        })
    }
    fn chunk_json(&mut self, count: usize) -> Result<String, SimError> {
        serde_json::to_string(&self.inner.step_chunk(count)?).map_err(|e| SimError(e.to_string()))
    }
    fn chunk_arrow(&mut self, count: usize) -> Result<Vec<u8>, SimError> {
        let rows = self.inner.step_chunk(count)?.rows;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        arrow_out::arrow_batch(&rows, self.inner.trace_targets()).map_err(SimError)
    }
}
#[cfg(test)]
mod tests;
