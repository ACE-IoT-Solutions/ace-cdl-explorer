use cdl_blocks::{
    BlockImpl, ConfiguredInstance, ParameterAccess, Registry, SlotAlloc, LEGACY_NAMES,
};
use cdl_model::{Annotation, Assignment, DataType, Instance, Meta, Value};
use std::collections::HashSet;
#[test]
fn all_legacy_names_resolve_uniquely() {
    let registry = Registry::new(cdl_compat::pinned_core_library());
    let mut ids = HashSet::new();
    assert_eq!(LEGACY_NAMES.len(), 43);
    for name in LEGACY_NAMES {
        let definition = registry.legacy(name).unwrap();
        assert_eq!(definition.name(), *name);
        assert!(ids.insert(definition.class()));
    }
}
#[test]
fn every_registered_standard_builder_conforms_to_the_pinned_core() {
    let library = cdl_compat::pinned_core_library();
    assert_eq!(library.classes.len(), 132);
    let registry = Registry::new(library);
    let mut implemented = 0;
    for (class, supported) in registry.standard_classes() {
        if !supported {
            continue;
        }
        implemented += 1;
        let definition = registry.get(&class.id).unwrap();
        assert_eq!(definition.class(), &class.id);
        let mut instance = Instance::new("probe", class.id.clone());
        for parameter in &class.parameters {
            if parameter.default.is_none() {
                instance.assignments.push(Assignment {
                    name: parameter.name.clone(),
                    value: match parameter.data_type {
                        DataType::Boolean => Value::Boolean(false),
                        DataType::Integer => Value::Integer(1),
                        _ => Value::Real(1.0),
                    },
                    meta: Meta::default(),
                });
            }
        }
        let config = ConfiguredInstance::new(&instance, class, "probe");
        let mut io = SlotAlloc::default();
        let mut index = 0;
        for port in &class.inputs {
            io.bind_input(&port.name, index);
            index += 1;
        }
        for port in &class.outputs {
            io.bind_output(&port.name, index);
            index += 1;
        }
        definition
            .build(&config, &io)
            .unwrap_or_else(|e| panic!("{}: {e}", class.id));
        for read in config.parameter_reads() {
            let parameter = class
                .parameters
                .iter()
                .find(|p| p.name == read.name)
                .unwrap_or_else(|| panic!("{} reads undeclared {}", class.id, read.name));
            assert_eq!(read.data_type.as_ref(), Some(&parameter.data_type));
            let compatible = match read.access {
                ParameterAccess::Inspect => true,
                ParameterAccess::Number => {
                    matches!(parameter.data_type, DataType::Real | DataType::Integer)
                }
                ParameterAccess::Boolean => parameter.data_type == DataType::Boolean,
                ParameterAccess::Text => matches!(
                    parameter.data_type,
                    DataType::String | DataType::Enumeration(_)
                ),
            };
            assert!(
                compatible,
                "{} reads {} using {:?}, declared {:?}",
                class.id, parameter.name, read.access, parameter.data_type
            );
        }
    }
    assert!(implemented >= 50);
    println!("standard implementation coverage: {implemented}/132");
    let catalog = registry.catalog();
    assert_eq!(catalog.classes.len(), 132);
    assert_eq!(
        catalog
            .classes
            .values()
            .filter(
                |c| c.meta.annotations.get("ace:implemented") == Some(&Annotation::Boolean(true))
            )
            .count(),
        implemented
    );
    serde_json::to_string(&catalog).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&cdl_compat::catalog_json(&registry).unwrap()).unwrap();
    assert_eq!(json["classes"].as_object().unwrap().len(), 132);
    assert!(catalog
        .classes
        .values()
        .all(|c| c.meta.label.is_some() && c.meta.annotations.contains_key("ace:group")));
}

#[test]
fn foreign_or_incompatible_interfaces_are_not_registered_by_suffix() {
    let mut library = cdl_compat::pinned_core_library();
    let id = cdl_model::ClassId::from(
        "http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.Add",
    );
    let mut foreign = library.classes[&id].clone();
    foreign.id = "https://example.org/Add".into();
    let foreign_id = foreign.id.clone();
    library.classes.insert(foreign_id.clone(), foreign);
    library.classes.get_mut(&id).unwrap().inputs[0].data_type = DataType::Boolean;
    let registry = Registry::new(library);
    assert!(registry.get(&foreign_id).is_none());
    assert!(registry.get(&id).is_none());
}

#[test]
fn step_duration_rejects_negative_and_nonfinite_values() {
    for dt in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(cdl_blocks::StepCtx::new(dt).is_err());
    }
    assert_eq!(cdl_blocks::StepCtx::new(0.0).unwrap().dt(), 0.0);
}

#[test]
fn standard_parameter_declaration_drift_is_not_hidden_by_implementation_fallbacks() {
    let mut library = cdl_compat::pinned_core_library();
    let id = cdl_model::ClassId::from(
        "http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.AddParameter",
    );
    library.classes.get_mut(&id).unwrap().parameters.clear();
    let class = &library.classes[&id];
    let instance = Instance::new("drift", id.clone());
    let config = ConfiguredInstance::new(&instance, class, "drift");
    let mut io = SlotAlloc::default();
    io.bind_input("u", 0);
    io.bind_output("y", 1);
    let registry = Registry::new(library.clone());
    assert!(registry
        .build(&config, &io)
        .err()
        .unwrap()
        .message
        .contains("undeclared parameter p"));
}
