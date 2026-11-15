//! Editor presets derived from registry declarations and validated by its builders.
use cdl_blocks::{ConfiguredInstance, Registry, SlotAlloc};
use cdl_model::{
    Assignment, BlockClass, BlockKind, DataType, Diagnostic, DiagnosticCode, Document, Instance,
    Meta, Value,
};
use serde_json::{json, Value as Json};

pub(crate) fn json() -> Result<String, Vec<Diagnostic>> {
    let mut library = cxf::core_library();
    for class in cdl_blocks::legacy_catalog_classes(&library).map_err(|e| vec![e])? {
        library.classes.insert(class.id.clone(), class);
    }
    let presets: Json = serde_json::from_str(include_str!("../resources/editor-presets.json"))
        .expect("valid editor preset metadata");
    for (id, bindings) in presets.as_object().unwrap() {
        let class = library
            .classes
            .get(&cdl_model::ClassId::from(id.as_str()))
            .ok_or_else(|| {
                vec![Diagnostic::new(
                    id,
                    DiagnosticCode::UnknownClass,
                    "editor preset class is absent",
                )]
            })?;
        for name in bindings.as_object().unwrap().keys() {
            if !class.parameters.iter().any(|p| &p.name == name) {
                return Err(vec![Diagnostic::new(
                    format!("{id}.{name}"),
                    DiagnosticCode::UnknownParameter,
                    "editor preset parameter is absent",
                )]);
            }
        }
    }
    let registry = Registry::new(library);
    let mut result = serde_json::to_value(registry.catalog()).expect("serializable catalog");
    let mut templates = Vec::new();
    for class in registry.library().classes.values() {
        if registry.get(&class.id).is_none() {
            continue;
        }
        let instance = preset(class, &registry, presets.get(&class.id.0)).map_err(|e| vec![e])?;
        let graph = cdl_compat::to_value(&Document {
            root: instance,
            library: registry.library().clone(),
        })?;
        templates.push(json!({"id":class.id,"graph":graph,"creatable":true}));
    }
    // These app-specific entries create an empty composite or request the existing
    // VAV template loader. Other building composites are documented, not palette items.
    for (kind, creatable) in [
        ("Composite", true),
        ("VAVBoxController", true),
        ("AHUController", false),
        ("PlantDispatch", false),
        ("BuildingController", false),
    ] {
        templates.push(json!({"id":kind,"graph":{"name":kind,"block_type":kind,"category":"composite","inputs":[],"outputs":[],"parameters":[]},"creatable":creatable}));
    }
    result["editor_templates"] = Json::Array(templates);
    serde_json::to_string(&result).map_err(|e| {
        vec![Diagnostic::new(
            "",
            DiagnosticCode::InvalidDocument,
            e.to_string(),
        )]
    })
}
fn assignment(name: &str, value: Value) -> Assignment {
    Assignment {
        name: name.into(),
        value,
        meta: Meta::default(),
    }
}
fn preset(
    class: &BlockClass,
    registry: &Registry,
    overrides: Option<&Json>,
) -> Result<Instance, Diagnostic> {
    let mut instance = Instance::new("preset", class.id.clone());
    if class.kind == BlockKind::Extension {
        instance.assignments = class
            .parameters
            .iter()
            .filter_map(|p| p.default.clone().map(|v| assignment(&p.name, v)))
            .collect();
    }
    if let Some(overrides) = overrides {
        for p in &class.parameters {
            let Some(raw) = overrides.get(&p.name) else {
                continue;
            };
            let value = match &p.data_type {
                DataType::Boolean => raw.as_bool().map(Value::Boolean),
                DataType::Integer => raw.as_i64().map(Value::Integer),
                DataType::Enumeration(_) => raw.as_str().map(|v| Value::Enum(v.into())),
                DataType::String => raw.as_str().map(|v| Value::String(v.into())),
                DataType::Real => raw.as_f64().map(Value::Real),
            }
            .ok_or_else(|| {
                Diagnostic::new(
                    format!("{}.{}", class.id.0, p.name),
                    DiagnosticCode::TypeMismatch,
                    "invalid editor preset value",
                )
            })?;
            if let Some(existing) = instance.assignments.iter_mut().find(|a| a.name == p.name) {
                existing.value = value;
            } else {
                instance.assignments.push(assignment(&p.name, value));
            }
        }
    }
    let required: Vec<_> = class
        .parameters
        .iter()
        .filter(|p| p.default.is_none() && !instance.assignments.iter().any(|a| a.name == p.name))
        .collect();
    // Initial values are editor choices, not runtime fallback semantics. Try a
    // small deterministic set and accept only configurations the registry builds.
    // Time-valued required parameters start at one minute for building alignment.
    if required.len() > 4 {
        return Err(Diagnostic::new(
            &class.id.0,
            DiagnosticCode::InvalidValue,
            "editor preset needs explicit initial values for this interface",
        ));
    }
    let attempts = 4usize.pow(required.len() as u32);
    let mut last = None;
    for attempt in 0..attempts {
        let mut candidate = instance.clone();
        for (index, p) in required.iter().enumerate() {
            let selection = (attempt / 4usize.pow(index as u32)) % 4;
            let time = p.units.unit.as_deref() == Some("http://qudt.org/vocab/unit#SEC");
            let scalar = [if time { 60.0 } else { 1.0 }, 0.0, -1.0, 60.0][selection];
            let value = match p.data_type {
                DataType::Boolean => Value::Boolean(false),
                DataType::Integer => Value::Integer(scalar as i64),
                _ => Value::Real(scalar),
            };
            candidate.assignments.push(assignment(&p.name, value));
        }
        let mut io = SlotAlloc::default();
        for (slot, p) in class.inputs.iter().chain(&class.outputs).enumerate() {
            if p.direction == cdl_model::Direction::Input {
                io.bind_input(&p.name, slot);
            } else {
                io.bind_output(&p.name, slot);
            }
        }
        let config = ConfiguredInstance::new(&candidate, class, &class.id.0);
        match registry.build(&config, &io) {
            Ok(_) => {
                let values = config.effective_parameters();
                for (name, mut value) in values {
                    if class
                        .parameters
                        .iter()
                        .any(|p| p.name == name && matches!(p.data_type, DataType::Enumeration(_)))
                    {
                        if let Value::Expr(text) | Value::String(text) = &value {
                            value = Value::Enum(text.rsplit('.').next().unwrap_or(text).into());
                        }
                    }
                    if !candidate.assignments.iter().any(|a| a.name == name) {
                        candidate.assignments.push(assignment(&name, value));
                    }
                }
                return Ok(candidate);
            }
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| {
        Diagnostic::new(
            &class.id.0,
            DiagnosticCode::InvalidValue,
            "no valid editor preset",
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdl_exec::{Program, Scalar};
    #[test]
    fn every_elementary_editor_preset_compiles_and_steps_with_its_declared_types() {
        let catalog: Json = serde_json::from_str(&json().unwrap()).unwrap();
        let overrides: Json =
            serde_json::from_str(include_str!("../resources/editor-presets.json")).unwrap();
        for (id, values) in overrides.as_object().unwrap() {
            let graph = &catalog["editor_templates"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == id.as_str())
                .unwrap()["graph"];
            for (name, value) in values.as_object().unwrap() {
                let actual = &graph["parameters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["name"] == name.as_str())
                    .unwrap()["value"];
                if value.is_number() {
                    assert_eq!(actual.as_f64(), value.as_f64(), "{id}.{name}");
                } else {
                    assert_eq!(actual, value, "{id}.{name}");
                }
            }
        }
        let mut count = 0;
        for template in catalog["editor_templates"].as_array().unwrap() {
            let graph = &template["graph"];
            if graph["category"] == "composite" {
                continue;
            }
            let document = cdl_compat::from_value(graph, Default::default()).unwrap();
            let program = Program::compile(&document, &Registry::new(document.library.clone()))
                .unwrap_or_else(|e| panic!("{}: {e}", template["id"]));
            let mut state = program.instantiate();
            for (index, p) in graph["inputs"].as_array().unwrap().iter().enumerate() {
                let name = p["name"].as_str().unwrap();
                let handle = program.input(name).unwrap();
                let value = match handle.data_type() {
                    DataType::Boolean => Scalar::Boolean(true),
                    DataType::Integer => Scalar::Integer(index as i64 + 1),
                    _ => Scalar::Real(index as f64 + 1.0),
                };
                state.set(&handle, value).unwrap();
            }
            state
                .step(15.0)
                .unwrap_or_else(|e| panic!("{}: {e}", template["id"]));
            count += 1;
        }
        assert_eq!(count, 59);
    }
}
