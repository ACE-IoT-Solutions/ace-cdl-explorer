use cdl_compat::{from_value, to_value, ReadOptions};
use cdl_model::{Annotation, BlockKind, DiagnosticCode};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn graphs(value: &Value, path: &str, out: &mut Vec<(String, Value)>) {
    if let Some(object) = value.as_object() {
        if object.contains_key("name")
            && (object.contains_key("block_type") || object.contains_key("type"))
        {
            out.push((path.into(), value.clone()));
            return;
        }
        for (key, child) in object {
            graphs(child, &format!("{path}.{key}"), out);
        }
    } else if let Some(values) = value.as_array() {
        for (index, child) in values.iter().enumerate() {
            graphs(child, &format!("{path}[{index}]"), out);
        }
    }
}
fn compare(graph: &Value, path: &str) {
    let document = from_value(graph, ReadOptions::default())
        .unwrap_or_else(|errors| panic!("{path}: {errors:?}"));
    let out = to_value(&document).unwrap();
    assert!(
        out == *graph,
        "{path}: first difference {:?}",
        first_difference(graph, &out, "")
    );
    assert!(
        cdl_model::validate(&document).is_empty(),
        "{path}: {:?}",
        cdl_model::validate(&document)
    );
}
#[test]
fn every_bundled_and_verification_graph_roundtrips() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut count = 0;
    for directory in ["web/public/examples", "verification"] {
        for entry in fs::read_dir(root.join(directory)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            let mut found = Vec::new();
            graphs(&value, &path.display().to_string(), &mut found);
            for (path, graph) in found {
                compare(&graph, &path);
                count += 1;
            }
        }
    }
    let suite: Value =
        serde_json::from_slice(&fs::read(root.join("verification/suite.json")).unwrap()).unwrap();
    for case in suite["cases"].as_array().unwrap() {
        let graph: Value = if let Some(path) = case["graph_file"].as_str() {
            serde_json::from_slice(&fs::read(root.join("verification").join(path)).unwrap())
                .unwrap()
        } else {
            case["graph"].clone()
        };
        let mut selected = &graph;
        for name in case["graph_path"].as_array().into_iter().flatten() {
            selected = selected["blocks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|block| block["name"] == *name)
                .unwrap();
        }
        compare(selected, case["id"].as_str().unwrap());
        count += 1;
    }
    assert!(count > suite["cases"].as_array().unwrap().len());
    println!("{count} bundled/verification graphs round-trip and validate");
}
#[test]
fn all_g36_leaves_resolve_to_one_explicit_class() {
    let graph: Value = serde_json::from_str(include_str!(
        "../../../web/public/examples/four_pipe_g36.json"
    ))
    .unwrap();
    let document = from_value(&graph, ReadOptions::default()).unwrap();
    let instances = document.instances().unwrap();
    let leaves: Vec<_> = instances
        .iter()
        .filter(|i| i.class.body.is_none())
        .collect();
    assert_eq!(leaves.len(), 7077);
    assert!(leaves
        .iter()
        .all(|i| i.class.id.0.starts_with("http://data.ashrae.org/S231#")
            || i.class.kind == BlockKind::Extension));
    println!(
        "7077 G36 leaves: {} standard, {} extensions",
        leaves
            .iter()
            .filter(|i| i.class.kind == BlockKind::Elementary)
            .count(),
        leaves
            .iter()
            .filter(|i| i.class.kind == BlockKind::Extension)
            .count()
    );
}
#[test]
fn model_edits_are_written_instead_of_a_saved_graph() {
    let graph = json!({"name":"c","block_type":"Constant","notes":"keep me","outputs":[{"name":"y","type":"Real","unit":"°C","layout":{"x":1}}],"parameters":[{"name":"k","value":2.0}]});
    let mut document = from_value(&graph, ReadOptions::default()).unwrap();
    document.root.name = "changed".into();
    document.root.assignments[0].value = cdl_model::Value::Real(3.0);
    let out = to_value(&document).unwrap();
    assert_eq!(out["name"], "changed");
    assert_eq!(out["parameters"][0]["value"], 3.0);
    assert_eq!(out["notes"], "keep me");
    assert_eq!(out["outputs"][0], graph["outputs"][0]);
}
#[test]
fn untyped_ambiguous_name_is_diagnosed() {
    let errors = from_value(
        &json!({"name":"c","block_type":"Constant"}),
        ReadOptions::default(),
    )
    .unwrap_err();
    assert!(errors
        .iter()
        .any(|e| e.code == DiagnosticCode::AmbiguousClass));
}
#[test]
fn equations_are_explicit_extensions_and_metadata_survives() {
    let graph = json!({"name":"custom","block_type":"Custom","outputs":[{"name":"y","type":"Real"}],"equations":[{"lhs":"y","rhs":"sin(time)"}],"ace:notes":{"nested":[1,true,null]},"layout":{"x":2}});
    let document = from_value(&graph, ReadOptions::default()).unwrap();
    let class = &document.library.classes[&document.root.class];
    assert_eq!(class.kind, BlockKind::Extension);
    assert!(matches!(
        class.meta.annotations.get("ace:equations"),
        Some(Annotation::Array(_))
    ));
    assert_eq!(to_value(&document).unwrap(), graph);
}
#[test]
fn composite_deduplication_is_opt_in_and_edits_are_independent_by_default() {
    let child = json!({"name":"a","block_type":"Composite","category":"composite","blocks":[{"name":"c","block_type":"Constant","outputs":[{"name":"y","type":"Real"}],"parameters":[{"name":"k","value":1.0}]}]});
    let mut second = child.clone();
    second["name"] = json!("b");
    let graph = json!({"name":"root","block_type":"Composite","blocks":[child,second]});
    for deduplicate in [false, true] {
        let document = from_value(
            &graph,
            ReadOptions {
                deduplicate_composites: deduplicate,
            },
        )
        .unwrap();
        let root = &document.library.classes[&document.root.class];
        let children = &root.body.as_ref().unwrap().instances;
        assert_eq!(children[0].class == children[1].class, deduplicate);
        assert_eq!(to_value(&document).unwrap(), graph);
    }
}

fn first_difference(a: &Value, b: &Value, path: &str) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(difference) = first_difference(
                &a.get(key).cloned().unwrap_or(Value::Null),
                &b.get(key).cloned().unwrap_or(Value::Null),
                &format!("{path}.{key}"),
            ) {
                return Some(difference);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(difference) = first_difference(a, b, &format!("{path}[{i}]")) {
                return Some(difference);
            }
        }
    }
    Some(format!("{path}: {a:?} -> {b:?}"))
}

#[test]
fn mixed_boundary_spellings_and_aliases_are_preserved_per_wire() {
    let graph = json!({"name":"r","type":"Composite","description":null,"inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],"blocks":[{"name":"a","type":"Gain","inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],"parameters":[{"name":"k","value":2}]}],"connections":[{"from":"u.u","to":"a.u","id":42},{"from_block":"a","from_output":"y","to_block":"y","to_input":"y"}]});
    let mut document = from_value(&graph, ReadOptions::default()).unwrap();
    assert_eq!(to_value(&document).unwrap(), graph);
    let class = document
        .library
        .classes
        .get_mut(&document.root.class)
        .unwrap();
    class.body.as_mut().unwrap().connections[0]
        .meta
        .annotations
        .insert("ace:route".into(), Annotation::String("edited".into()));
    let out = to_value(&document).unwrap();
    assert_eq!(out["connections"][0]["ace:route"], "edited");
    assert_eq!(out["connections"][0]["from"], "u.u");
}
#[test]
fn field_metadata_units_dimensions_and_constants_reflect_model_edits() {
    let graph = json!({"name":"c","block_type":"Constant","notes":"old","outputs":[{"name":"y","type":"Real","unit":"°C","description":"before"}],"constants":[{"name":"fixed","value":2,"type":"Integer"}],"parameters":[{"name":"k","value":2.0}]});
    let mut document = from_value(&graph, ReadOptions::default()).unwrap();
    document
        .root
        .meta
        .annotations
        .insert("ace:notes".into(), Annotation::String("after".into()));
    let class = document
        .library
        .classes
        .get_mut(&document.root.class)
        .unwrap();
    class.constants[0].default = Some(cdl_model::Value::Integer(3));
    class.outputs[0].meta.description = Some("after".into());
    class.outputs[0].units.unit = Some("http://qudt.org/vocab/unit#K".into());
    class.outputs[0].dimensions = vec![cdl_model::Dimension::Size(2)];
    let out = to_value(&document).unwrap();
    assert_eq!(out["notes"], "after");
    assert_eq!(out["constants"][0]["value"], 3);
    assert_eq!(out["outputs"][0]["unit"], "K");
    assert_eq!(out["outputs"][0]["description"], "after");
    assert_eq!(out["outputs"][0]["dimensions"], json!([2]));
}
#[test]
fn malformed_container_and_record_types_return_diagnostics() {
    for graph in [
        json!(null),
        json!({"name":"a","block_type":"Gain","inputs":{}}),
        json!({"name":"a","block_type":"Gain","inputs":[7]}),
        json!({"name":"a","block_type":"Gain","parameters":[{"name":"k"}]}),
        json!({"name":"a","block_type":"Composite","connections":[{"from":3}]}),
    ] {
        assert!(
            from_value(&graph, ReadOptions::default()).is_err(),
            "{graph}"
        );
    }
}

#[test]
fn standard_definitions_are_shared_while_instance_metadata_and_type_hints_remain_independent() {
    let graph = json!({"name":"root","block_type":"Composite","blocks":[
        {"name":"a","block_type":"Constant","outputs":[{"name":"y","type":"Real","unit":"°C"}],"parameters":[{"name":"k","value":1,"type":"Integer"}]},
        {"name":"b","block_type":"Constant","outputs":[{"name":"y","type":"Real","unit":"K","label":"second"}],"parameters":[{"name":"k","value":2.0}]}
    ]});
    let document = from_value(&graph, ReadOptions::default()).unwrap();
    let children = &document.library.classes[&document.root.class]
        .body
        .as_ref()
        .unwrap()
        .instances;
    assert_eq!(children[0].class, children[1].class);
    let standard = &document.library.classes[&children[0].class];
    assert_eq!(standard.kind, BlockKind::Elementary);
    assert_eq!(standard.parameters[0].data_type, cdl_model::DataType::Real);
    assert_eq!(
        children[0].connectors[0].units.unit.as_deref(),
        Some("http://qudt.org/vocab/unit#DEG_C")
    );
    assert_eq!(
        children[1].connectors[0].units.unit.as_deref(),
        Some("http://qudt.org/vocab/unit#K")
    );
    assert_eq!(to_value(&document).unwrap(), graph);
}

#[test]
fn array_bindings_keep_their_shape_without_adding_json_fields() {
    let graph = json!({"name":"custom","block_type":"ArrayExtension","parameters":[{"name":"matrix","value":[[1,2],[3,4]],"type":"Integer"}]});
    let document = from_value(&graph, ReadOptions::default()).unwrap();
    assert!(cdl_model::validate(&document).is_empty());
    assert_eq!(
        document.library.classes[&document.root.class].parameters[0].dimensions,
        vec![cdl_model::Dimension::Size(2), cdl_model::Dimension::Size(2)]
    );
    assert_eq!(to_value(&document).unwrap(), graph);
    let ragged = json!({"name":"custom","block_type":"ArrayExtension","parameters":[{"name":"matrix","value":[[1,2],[3]],"type":"Integer"}]});
    let invalid = from_value(&ragged, ReadOptions::default()).unwrap();
    assert!(cdl_model::validate(&invalid)
        .iter()
        .any(|d| d.code == DiagnosticCode::InvalidValue));
}

#[test]
fn integral_float_bindings_and_large_integer_literals_roundtrip_without_losing_edits() {
    for (block_type, port_type, value) in [
        (
            "Buildings.Controls.OBC.CDL.Integers.Sources.Constant",
            "Integer",
            json!(2.0),
        ),
        ("Constant", "Real", json!(18446744073709551615u64)),
    ] {
        let graph = json!({"name":"c","block_type":block_type,"outputs":[{"name":"y","type":port_type}],"parameters":[{"name":"k","value":value}]});
        let mut document = from_value(&graph, ReadOptions::default()).unwrap();
        assert!(cdl_model::validate(&document).is_empty());
        assert_eq!(to_value(&document).unwrap(), graph);
        document.root.assignments[0].value = cdl_model::Value::Integer(3);
        assert_eq!(to_value(&document).unwrap()["parameters"][0]["value"], 3);
    }
}

#[test]
fn compact_unit_iris_expand_in_the_model_but_keep_their_original_json_spelling() {
    let graph = json!({"name":"c","block_type":"Constant","outputs":[{"name":"y","type":"Real","unit":"°C","cxf_unit":"unit:K","cxf_display_unit":"unit:DEG_C","quantity_kind":"q:Temperature"}],"parameters":[{"name":"k","value":300.0,"cxf_unit":"unit:K"}]});
    let document = from_value(&graph, ReadOptions::default()).unwrap();
    assert_eq!(
        document.root.connectors[0].units.unit.as_deref(),
        Some("http://qudt.org/vocab/unit#K")
    );
    assert_eq!(
        document.root.connectors[0].units.quantity_kind.as_deref(),
        Some("http://qudt.org/vocab/quantitykind#Temperature")
    );
    assert_eq!(to_value(&document).unwrap(), graph);
}

#[test]
fn custom_names_generate_iri_safe_extension_identifiers() {
    let graph = json!({"name":"c","block_type":"Custom block / variant","outputs":[{"name":"y","type":"Real"}]});
    let document = from_value(&graph, ReadOptions::default()).unwrap();
    assert!(!document.root.class.0.contains(' '));
    assert!(cdl_model::validate(&document).is_empty());
    assert_eq!(to_value(&document).unwrap(), graph);
}

#[test]
fn removing_metadata_and_editing_parameter_declarations_is_reflected_in_json() {
    let graph = json!({"name":"c","block_type":"Constant","notes":"remove","other:property":[1,2],"outputs":[{"name":"y","type":"Real","description":"remove"}],"parameters":[{"name":"k","value":1.0,"description":"before","other:property":42}]});
    let mut document = from_value(&graph, ReadOptions::default()).unwrap();
    document.root.meta.annotations.shift_remove("ace:notes");
    document
        .root
        .meta
        .annotations
        .shift_remove("other:property");
    document.root.connectors[0].meta.description = None;
    document.root.parameters[0].meta.description = Some("after".into());
    document.root.parameters[0]
        .meta
        .annotations
        .shift_remove("other:property");
    let out = to_value(&document).unwrap();
    assert!(out.get("notes").is_none());
    assert!(out.get("other:property").is_none());
    assert!(out["outputs"][0].get("description").is_none());
    assert_eq!(out["parameters"][0]["description"], "after");
    assert!(out["parameters"][0].get("other:property").is_none());
}
#[test]
fn namespaced_alias_collisions_and_reserved_properties_roundtrip_without_forging_adapter_state() {
    let graph = json!({"name":"c","block_type":"Constant","notes":"plain","ace:notes":"namespaced","ace:compat:fields":["name"],"ace:equations":"metadata only","outputs":[{"name":"y","type":"Real"}],"parameters":[{"name":"k","value":1.0}]});
    let document = from_value(&graph, ReadOptions::default()).unwrap();
    assert_eq!(to_value(&document).unwrap(), graph);
}

#[test]
fn retyping_a_binding_does_not_reuse_an_obsolete_literal_representation() {
    let graph = json!({"name":"c","block_type":"Constant","outputs":[{"name":"y","type":"Real"}],"parameters":[{"name":"k","value":2.0,"type":"Integer"}]});
    let mut document = from_value(&graph, ReadOptions::default()).unwrap();
    assert_eq!(to_value(&document).unwrap(), graph);
    document.root.assignments[0].value = cdl_model::Value::Integer(2);
    assert_eq!(
        to_value(&document).unwrap()["parameters"][0]["value"],
        json!(2)
    );
}

#[test]
fn array_extents_are_preserved_independently_of_host_pointer_width() {
    let graph = json!({"name":"large","block_type":"ArrayExtension","outputs":[{"name":"y","type":"Real","dimensions":[4294967297u64]}]});
    let document = from_value(&graph, ReadOptions::default()).unwrap();
    assert_eq!(
        document.library.classes[&document.root.class].outputs[0].dimensions,
        vec![cdl_model::Dimension::Size(4294967297u64)]
    );
    assert_eq!(to_value(&document).unwrap(), graph);
}
