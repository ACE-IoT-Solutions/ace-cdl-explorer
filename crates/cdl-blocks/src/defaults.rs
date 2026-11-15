use crate::{ConfiguredInstance, Registry, SlotAlloc};
use cdl_model::{Annotation, Assignment, Diagnostic, IndexMap, Meta, Value};
const ORIGINAL: &str = "ace:compat:implicit_value";
fn fingerprint(value: &Value) -> Annotation {
    let (kind, value) = match value {
        Value::Real(v) => ("real", Annotation::Real(*v)),
        Value::Integer(v) => ("integer", Annotation::Integer(*v)),
        Value::Boolean(v) => ("boolean", Annotation::Boolean(*v)),
        Value::String(v) => ("string", Annotation::String(v.clone())),
        Value::Enum(v) => ("enum", Annotation::String(v.clone())),
        Value::Expr(v) => ("expr", Annotation::String(v.clone())),
        Value::Array(v) => (
            "array",
            Annotation::Array(v.iter().map(fingerprint).collect()),
        ),
    };
    Annotation::Object(IndexMap::from_iter([
        ("type".into(), Annotation::String(kind.into())),
        ("value".into(), value),
    ]))
}
/// Whether a compatibility-only implicit assignment is unchanged and may be omitted
/// when writing the original simplified JSON format.
pub fn implicit_assignment_unchanged(assignment: &Assignment) -> bool {
    assignment.meta.annotations.get(ORIGINAL) == Some(&fingerprint(&assignment.value))
}
impl Registry {
    /// Materialize missing legacy fallback choices as typed parameter assignments.
    ///
    /// The values are obtained from the same builder that supplies executable
    /// behavior. Invalid instances return diagnostics instead of gaining defaults
    /// that would silently make a previously invalid graph valid.
    pub fn legacy_defaults(
        &self,
        config: &ConfiguredInstance<'_>,
    ) -> Result<Vec<Assignment>, Diagnostic> {
        if !config.is_legacy() {
            return Ok(Vec::new());
        }
        let mut io = SlotAlloc::default();
        let mut index = 0;
        for port in &config.class.inputs {
            io.bind_input(&port.name, index);
            index += 1;
        }
        for port in &config.class.outputs {
            io.bind_output(&port.name, index);
            index += 1;
        }
        self.build(config, &io)?;
        Ok(config
            .effective_parameters()
            .into_iter()
            .filter(|(name, _)| !config.instance.assignments.iter().any(|a| &a.name == name))
            .map(|(name, value)| {
                let mut meta = Meta::default();
                meta.annotations
                    .insert(ORIGINAL.into(), fingerprint(&value));
                Assignment { name, value, meta }
            })
            .collect())
    }
}
