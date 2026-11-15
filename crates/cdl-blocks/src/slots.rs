use cdl_model::{Diagnostic, DiagnosticCode};
use std::collections::BTreeMap;
/// Numeric slot index allocated once during compilation.
pub type Slot = usize;
/// Inputs/outputs of one instance, bound to a host program's shared slots.
#[derive(Clone, Debug, Default)]
pub struct SlotAlloc {
    inputs: BTreeMap<String, Slot>,
    outputs: BTreeMap<String, Slot>,
}
impl SlotAlloc {
    /// Bind an input connector.
    pub fn bind_input(&mut self, name: impl Into<String>, slot: Slot) {
        self.inputs.insert(name.into(), slot);
    }
    /// Bind an output connector.
    pub fn bind_output(&mut self, name: impl Into<String>, slot: Slot) {
        self.outputs.insert(name.into(), slot);
    }
    /// Resolve an input; missing bindings are compile diagnostics.
    pub fn input(&self, name: &str, path: &str) -> Result<Slot, Diagnostic> {
        self.inputs.get(name).copied().ok_or_else(|| {
            Diagnostic::new(
                path,
                DiagnosticCode::MissingEndpoint,
                format!("missing input {name}"),
            )
        })
    }
    /// Resolve an output; missing bindings are compile diagnostics.
    pub fn output(&self, name: &str, path: &str) -> Result<Slot, Diagnostic> {
        self.outputs.get(name).copied().ok_or_else(|| {
            Diagnostic::new(
                path,
                DiagnosticCode::MissingEndpoint,
                format!("missing output {name}"),
            )
        })
    }
}
