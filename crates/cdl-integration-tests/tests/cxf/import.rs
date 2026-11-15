use cdl_model::{DataType, Value};
#[test]
fn cooling_coil_import_resolves_classes_ports_bindings_and_wires() {
    let source = include_str!("fixtures/CoolingCoil.jsonld");
    let doc = cxf::from_json(source, &cxf::core_library()).unwrap();
    assert_eq!(doc.root.name, "CoolingCoil");
    let root = &doc.library.classes[&doc.root.class];
    assert_eq!(root.body.as_ref().unwrap().instances.len(), 6);
    assert_eq!(root.body.as_ref().unwrap().connections.len(), 11);
    let port = root.connector("TAirSup").unwrap();
    assert_eq!(
        port.units.unit.as_deref(),
        Some("http://qudt.org/vocab/unit#K")
    );
    assert_eq!(
        port.units.display_unit.as_deref(),
        Some("http://qudt.org/vocab/unit#DEG_C")
    );
    let child = root
        .body
        .as_ref()
        .unwrap()
        .instances
        .iter()
        .find(|i| i.name == "conCoi")
        .unwrap();
    assert_eq!(
        child
            .assignments
            .iter()
            .find(|p| p.name == "k")
            .unwrap()
            .value,
        Value::Expr("kCooCoi".into())
    );
    assert_eq!(
        doc.library.classes[&child.class]
            .connector("trigger")
            .unwrap()
            .data_type,
        DataType::Boolean
    );
    assert!(
        cdl_model::validate(&doc).is_empty(),
        "{:?}",
        cdl_model::validate(&doc)
    );
    let graph = cdl_compat::to_value(&doc).unwrap();
    assert_eq!(graph["name"], "CoolingCoil");
}
#[test]
fn official_add_is_a_standard_elementary_root() {
    let doc = cxf::from_json(
        include_str!("fixtures/Reals-Add.jsonld"),
        &cxf::core_library(),
    )
    .unwrap();
    assert_eq!(
        doc.root.class.0,
        "http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.Add"
    );
    assert_eq!(doc.library.classes[&doc.root.class].inputs.len(), 2);
}
#[test]
fn official_fixtures_reach_a_canonical_export_fixed_point() {
    for source in [
        include_str!("fixtures/Reals-Add.jsonld"),
        include_str!("fixtures/CoolingCoil.jsonld"),
    ] {
        let library = cxf::core_library();
        let first = cxf::from_json(source, &library).unwrap();
        let a = cxf::to_value(&first).unwrap();
        let second = cxf::from_value(&a, &library).unwrap();
        let b = cxf::to_value(&second).unwrap();
        assert_eq!(a, b);
        assert!(
            cdl_model::validate(&second).is_empty(),
            "{:?}",
            cdl_model::validate(&second)
        );
    }
}
#[test]
fn bundled_buildings_export_with_resolvable_links_and_reimport_stably() {
    for source in [
        include_str!("../../../../web/public/examples/four_pipe_building.json"),
        include_str!("../../../../web/public/examples/four_pipe_g36.json"),
    ] {
        let doc = cdl_compat::from_json(source, Default::default()).unwrap();
        let exported = cxf::to_value(&doc).unwrap();
        let graph = exported["@graph"].as_array().unwrap();
        let ids: std::collections::HashSet<_> =
            graph.iter().map(|n| n["@id"].as_str().unwrap()).collect();
        for node in graph {
            for key in [
                "S231:containsBlock",
                "S231:hasInput",
                "S231:hasOutput",
                "S231:hasParameter",
                "S231:hasInstance",
                "S231:isConnectedTo",
            ] {
                if let Some(refs) = node.get(key) {
                    let refs = refs
                        .as_array()
                        .map(|v| v.iter().collect::<Vec<_>>())
                        .unwrap_or_else(|| vec![refs]);
                    for r in refs {
                        assert!(
                            ids.contains(r["@id"].as_str().unwrap()),
                            "{} {key}",
                            node["@id"]
                        );
                    }
                }
            }
        }
        let imported = cxf::from_value(&exported, &cxf::core_library()).unwrap();
        assert!(
            cdl_model::validate(&imported).is_empty(),
            "{:?}",
            cdl_model::validate(&imported)
        );
        assert_eq!(
            doc.instances().unwrap().len(),
            imported.instances().unwrap().len()
        );
        let a = cxf::to_value(&imported).unwrap();
        let again = cxf::from_value(&a, &cxf::core_library()).unwrap();
        assert_eq!(a, cxf::to_value(&again).unwrap());
    }
}
#[test]
fn every_pinned_upstream_family_imports_and_reexports_at_a_fixed_point() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/manifest.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cxf/fixtures");
    for fixture in manifest["fixtures"].as_array().unwrap() {
        let source = std::fs::read_to_string(root.join(fixture["file"].as_str().unwrap())).unwrap();
        let doc = cxf::from_json(&source, &cxf::core_library())
            .unwrap_or_else(|e| panic!("{}: {e:?}", fixture["file"]));
        let a = cxf::to_value(&doc).unwrap();
        let second = cxf::from_value(&a, &cxf::core_library()).unwrap();
        assert_eq!(a, cxf::to_value(&second).unwrap(), "{}", fixture["file"]);
    }
}
