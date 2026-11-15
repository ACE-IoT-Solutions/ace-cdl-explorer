//! Canonical format, catalog and timing boundary shared by wasm exports and tests.
use cdl_blocks::{ConfiguredInstance, Registry, SlotAlloc};
#[cfg(test)]
use cdl_exec::Program;
use cdl_model::{Assignment, Diagnostic, DiagnosticCode, Document, Instance, Meta};
use wasm_bindgen::prelude::*;

fn invalid(error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::new(
        "",
        DiagnosticCode::InvalidDocument,
        error.to_string(),
    )]
}
fn js_error(errors: Vec<Diagnostic>) -> JsValue {
    JsValue::from_str(&serde_json::to_string(&errors).expect("serializable diagnostics"))
}
pub(crate) fn document(json: &str) -> Result<Document, Vec<Diagnostic>> {
    cdl_compat::from_json(json, Default::default())
}

/// Convert compact CXF to the editor's simplified graph shape.
#[wasm_bindgen]
pub fn cxf_import(json: &str) -> Result<String, JsValue> {
    import_project(json).map_err(js_error)
}
/// Convert an editor graph to compact CXF using authoritative standard interfaces.
#[wasm_bindgen]
pub fn cxf_export(json: &str) -> Result<String, JsValue> {
    export_project(json).map_err(js_error)
}
/// Encode the registry catalog, including implementation coverage and declarations.
#[wasm_bindgen]
pub fn block_catalog() -> String {
    crate::editor_catalog::json().expect("valid registry editor presets")
}
/// Resolve the same event alignment used by the native building simulator.
#[wasm_bindgen]
pub fn aligned_step(json: &str, requested: u32) -> Result<u32, JsValue> {
    alignment(json, requested).map_err(js_error)
}
pub(crate) fn alignment(json: &str, requested: u32) -> Result<u32, Vec<Diagnostic>> {
    let document = document(json)?;
    cdl_exec::aligned_step(
        &document,
        &Registry::new(document.library.clone()),
        requested.into(),
    )
    .map(|v| v as u32)
    .map_err(|error| match error {
        cdl_exec::Error::Diagnostics(errors) => errors,
        error => invalid(error),
    })
}
pub(crate) fn export_project(json: &str) -> Result<String, Vec<Diagnostic>> {
    cxf::to_json(&document(json)?)
}
pub(crate) fn import_project(json: &str) -> Result<String, Vec<Diagnostic>> {
    let mut document = cxf::from_json(json, &cxf::core_library())?;
    // The simplified editor format has bindings rather than inherited defaults.
    // Reify supported defaults through the actual kernel builder, so aliases such
    // as falseHoldDuration=trueHoldDuration do not become unsupported string
    // bindings. Canonical CXF I/O still retains the declaration expressions.
    // Unsupported classes/configurations remain inspectable, with expressions intact.
    let registry = Registry::new(document.library.clone());
    effective_defaults(&mut document.root, &registry);
    for class in document.library.classes.values_mut() {
        if let Some(body) = &mut class.body {
            for instance in &mut body.instances {
                effective_defaults(instance, &registry);
            }
        }
    }
    cdl_compat::to_json(&document)
}
fn effective_defaults(instance: &mut Instance, registry: &Registry) {
    let Some(class) = registry.library().classes.get(&instance.class) else {
        return;
    };
    let config = ConfiguredInstance::new(instance, class, &instance.name);
    let mut io = SlotAlloc::default();
    for (slot, port) in class.inputs.iter().chain(&class.outputs).enumerate() {
        if port.direction == cdl_model::Direction::Input {
            io.bind_input(&port.name, slot);
        } else {
            io.bind_output(&port.name, slot);
        }
    }
    if registry.build(&config, &io).is_err() {
        return;
    }
    let values = config.effective_parameters();
    for (name, value) in values {
        if !instance.assignments.iter().any(|a| a.name == name) {
            instance.assignments.push(Assignment {
                name,
                value,
                meta: Meta::default(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdl_exec::Scalar;
    use serde_json::json;

    const G36: &str = include_str!("../../../web/public/examples/four_pipe_g36.json");
    const CONVENTIONAL: &str = include_str!("../../../web/public/examples/four_pipe_building.json");
    #[test]
    fn diagnostics_are_structured_and_catalog_uses_the_registry() {
        let errors: Vec<Diagnostic> = serde_json::from_str(&crate::validate_json("{}")).unwrap();
        assert!(!errors.is_empty());
        assert!(!errors[0].message.is_empty());
        let value: serde_json::Value = serde_json::from_str(&block_catalog()).unwrap();
        let classes = value["classes"].as_object().unwrap();
        let standard: Vec<_> = classes
            .iter()
            .filter(|(id, _)| {
                id.starts_with("http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.")
            })
            .collect();
        assert_eq!(standard.len(), 132);
        assert_eq!(
            standard
                .iter()
                .filter(|(_, class)| class["meta"]["annotations"]["ace:implemented"] == true)
                .count(),
            52
        );
    }
    #[test]
    fn alignment_agrees_with_simulator_for_both_buildings_and_rejects_invalid_timing() {
        assert_eq!(alignment(CONVENTIONAL, 900).unwrap(), 900);
        assert_eq!(alignment(G36, 900).unwrap(), 15);
        assert!(alignment(G36, 0).is_err());
        let mut graph: serde_json::Value = serde_json::from_str(G36).unwrap();
        fn invalid_period(value: &mut serde_json::Value) -> bool {
            if value["block_type"]
                .as_str()
                .is_some_and(|s| s.ends_with("SampleTrigger"))
            {
                for p in value["parameters"].as_array_mut().unwrap() {
                    if p["name"] == "period" {
                        p["value"] = json!(7.5);
                        return true;
                    }
                }
            }
            value["blocks"]
                .as_array_mut()
                .is_some_and(|blocks| blocks.iter_mut().any(invalid_period))
        }
        assert!(invalid_period(&mut graph));
        assert!(alignment(&graph.to_string(), 900).is_err());
    }
    #[test]
    fn cxf_projection_reifies_supported_default_aliases_without_evaluating_parent_expressions() {
        let source = json!({"@graph":[
            {"@id":"urn:hold","@type":"http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Logical.TrueFalseHold","S231:hasInstance":[{"@id":"urn:hold.trueHoldDuration"}]},
            {"@id":"urn:hold.trueHoldDuration","S231:value":120}
        ]}).to_string();
        let native = cxf::from_json(&source, &cxf::core_library()).unwrap();
        let projection = import_project(&source).unwrap();
        assert_eq!(crate::validate_json(&projection), "[]");
        let projected = document(&projection).unwrap();
        let a = Program::compile(&native, &Registry::new(native.library.clone())).unwrap();
        let b = Program::compile(&projected, &Registry::new(projected.library.clone())).unwrap();
        let mut a_state = a.instantiate();
        let mut b_state = b.instantiate();
        for flag in [false, true, false, false, false, false, true, false] {
            a_state
                .set(&a.input("u").unwrap(), Scalar::Boolean(flag))
                .unwrap();
            b_state
                .set(&b.input("u").unwrap(), Scalar::Boolean(flag))
                .unwrap();
            a_state.step(30.0).unwrap();
            b_state.step(30.0).unwrap();
            assert_eq!(a_state.output("y").unwrap(), b_state.output("y").unwrap());
        }
        let expression = json!({"@graph":[
            {"@id":"urn:gain","@type":"http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.MultiplyByParameter","S231:hasInstance":[{"@id":"urn:gain.k"}]},
            {"@id":"urn:gain.k","S231:value":"parentGain"}
        ]}).to_string();
        let projection: serde_json::Value =
            serde_json::from_str(&import_project(&expression).unwrap()).unwrap();
        assert!(projection["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "k" && p["value"] == "parentGain"));
    }
    #[test]
    fn browser_format_boundary_preserves_old_export_execution_and_metadata() {
        let original = include_str!("../../cdl-integration-tests/tests/cxf/legacy/graph.json");
        let exported = export_project(original).unwrap();
        let projected = import_project(&exported).unwrap();
        assert_eq!(crate::validate_json(&projected), "[]");
        let first: serde_json::Value = serde_json::from_str(&projected).unwrap();
        let second: serde_json::Value =
            serde_json::from_str(&import_project(&export_project(&projected).unwrap()).unwrap())
                .unwrap();
        assert_eq!(first, second);
        for graph in [original, projected.as_str()] {
            let doc = document(graph).unwrap();
            let program = Program::compile(&doc, &Registry::new(doc.library.clone())).unwrap();
            let mut state = program.instantiate();
            state.set_input("a", Scalar::Real(4.0)).unwrap();
            state.set_input("b", Scalar::Real(1.0)).unwrap();
            for (choice, expected) in [(true, 7.0), (false, 1.0)] {
                state.set_input("choice", Scalar::Boolean(choice)).unwrap();
                state.step(1.0).unwrap();
                assert_eq!(state.output("y").unwrap(), Scalar::Real(expected));
            }
        }
    }
}
