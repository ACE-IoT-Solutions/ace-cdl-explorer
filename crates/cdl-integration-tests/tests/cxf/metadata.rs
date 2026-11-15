use cdl_model::*;
use serde_json::json;
fn imported(value: serde_json::Value) -> Document {
    cxf::from_value(&value, &cxf::core_library()).unwrap()
}
#[test]
fn instance_only_ports_and_parameters_survive_without_polluting_the_standard_class() {
    let source = json!({"@context":{"S231":"http://data.ashrae.org/S231#","ex":"https://example.org/#"},"@graph":[
    {"@id":"ex:root","@type":"http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.MultiplyByParameter","S231:label":"gain","S231:hasInstance":[{"@id":"ex:root.extra"},{"@id":"ex:root.factor"}]},
    {"@id":"ex:root.extra","@type":"S231:RealInput","ex:tag":{"@id":"ex:point"}},
    {"@id":"ex:root.factor","S231:value":4,"ex:quality":"custom"}
    ]});
    let doc = imported(source);
    assert!(doc.root.connectors.iter().any(|p| p.name == "extra"));
    assert!(doc
        .root
        .assignments
        .iter()
        .any(|p| p.name == "factor" && p.value == Value::Integer(4)));
    assert!(doc.library.classes[&ClassId::from(
        "http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.MultiplyByParameter"
    )]
        .connector("extra")
        .is_none());
    let exported = cxf::to_value(&doc).unwrap();
    let second = cxf::from_value(&exported, &cxf::core_library()).unwrap();
    assert_eq!(doc.root.connectors, second.root.connectors);
    assert_eq!(doc.root.assignments, second.root.assignments);
}
#[test]
fn typed_string_expression_array_enum_bounds_and_labels_remain_distinct() {
    let id = ClassId::from("https://example.org/#ArrayBlock");
    let enumeration = Enumeration {
        id: "https://example.org/#Mode".into(),
        members: vec!["Off".into(), "On".into()],
        meta: Meta::default(),
    };
    let mut class = BlockClass::new(id.clone(), BlockKind::Extension);
    let mut port = Connector::new("u space", Direction::Input, DataType::Real);
    port.dimensions = vec![Dimension::Size(2)];
    port.meta.label = Some("Display label".into());
    class.inputs.push(port);
    let mut p = ParameterDecl {
        name: "text".into(),
        data_type: DataType::String,
        default: Some(Value::String("literal".into())),
        units: Units::default(),
        min: None,
        max: None,
        dimensions: vec![],
        meta: Meta::default(),
    };
    class.parameters.push(p.clone());
    p.name = "formula".into();
    p.default = None;
    class.parameters.push(p.clone());
    p.name = "array".into();
    p.data_type = DataType::Real;
    p.dimensions = vec![Dimension::Size(2)];
    p.min = Some(Value::Real(-2.0));
    p.max = Some(Value::Expr("upper".into()));
    class.parameters.push(p.clone());
    p.name = "mode".into();
    p.data_type = DataType::Enumeration(enumeration.id.clone());
    p.dimensions = vec![];
    p.min = None;
    p.max = None;
    class.parameters.push(p);
    let mut library = Library::default();
    library.classes.insert(id.clone(), class);
    library
        .enumerations
        .insert(enumeration.id.clone(), enumeration);
    library.meta.annotations.insert(
        "https://example.org/#owner".into(),
        Annotation::String("research".into()),
    );
    let mut root = Instance::new("array_root", id);
    root.meta.label = Some("User label".into());
    root.assignments = vec![
        Assignment {
            name: "formula".into(),
            value: Value::Expr("quotedVariable".into()),
            meta: Meta::default(),
        },
        Assignment {
            name: "array".into(),
            value: Value::Array(vec![Value::Real(1.0), Value::Expr("x".into())]),
            meta: Meta::default(),
        },
        Assignment {
            name: "mode".into(),
            value: Value::Enum("On".into()),
            meta: Meta::default(),
        },
    ];
    let doc = Document { library, root };
    let exported = cxf::to_value(&doc).unwrap();
    let second = cxf::from_value(&exported, &Library::default()).unwrap();
    assert_eq!(second.root.name, "array_root");
    assert_eq!(second.root.meta.label.as_deref(), Some("User label"));
    assert_eq!(second.root.assignments, doc.root.assignments);
    let class = &second.library.classes[&second.root.class];
    assert_eq!(class.inputs[0].name, "u space");
    assert_eq!(class.inputs[0].meta.label.as_deref(), Some("Display label"));
    assert_eq!(class.inputs[0].dimensions, vec![Dimension::Size(2)]);
    assert_eq!(class.parameters[2].min, Some(Value::Real(-2.0)));
    assert_eq!(class.parameters[2].max, Some(Value::Expr("upper".into())));
    assert_eq!(second.library.meta, doc.library.meta);
    assert!(
        cdl_model::validate(&second).is_empty(),
        "{:?}",
        cdl_model::validate(&second)
    );
}
#[test]
fn final_binding_edits_do_not_reappear_from_declaration_metadata() {
    let mut source: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/CoolingCoil.jsonld")).unwrap();
    let mut doc = cxf::from_value(&source, &cxf::core_library()).unwrap();
    let root = doc.library.classes.get_mut(&doc.root.class).unwrap();
    let child = root
        .body
        .as_mut()
        .unwrap()
        .instances
        .iter_mut()
        .find(|i| i.name == "conCoi")
        .unwrap();
    let binding = child
        .assignments
        .iter_mut()
        .find(|p| p.name == "k")
        .unwrap();
    assert_eq!(
        binding.meta.annotations.shift_remove("S231:isFinal"),
        Some(Annotation::Boolean(true))
    );
    source = cxf::to_value(&doc).unwrap();
    let parameter = source["@graph"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| {
            n["@id"]
                .as_str()
                .is_some_and(|id| id.ends_with(".conCoi.k"))
        })
        .unwrap();
    assert!(parameter.get("S231:isFinal").is_none());
}
#[test]
fn qudt_context_variants_and_unknown_iri_properties_normalize_without_unit_conversion() {
    for namespace in [
        "http://qudt.org/schema/qudt/",
        "http://qudt.org/schema/qudt#",
    ] {
        let source = json!({"@context":{"B":"http://data.ashrae.org/S231#","Q":namespace,"U":"http://qudt.org/vocab/unit#","X":"https://example.org/#"},"@graph":[
        {"@id":"X:root","@type":"B:ExtensionBlock","B:label":"root","B:hasInput":{"@id":"X:root.u"},"X:payload":{"@id":"X:external"}},
        {"@id":"X:root.u","@type":"B:RealInput","Q:hasUnit":{"@id":"U:K"},"B:hasDisplayUnit":{"@id":"U:DEG_C"},"B:isArray":true,"B:numberDimensions":2}
        ]});
        let doc = imported(source);
        let p = &doc.library.classes[&doc.root.class].inputs[0];
        assert_eq!(p.dimensions, vec![Dimension::Expr(":".into()); 2]);
        assert_eq!(
            p.units.unit.as_deref(),
            Some("http://qudt.org/vocab/unit#K")
        );
        assert!(doc
            .root
            .meta
            .annotations
            .contains_key("https://example.org/#payload"));
        let a = cxf::to_value(&doc).unwrap();
        let b = cxf::from_value(&a, &cxf::core_library()).unwrap();
        assert_eq!(a, cxf::to_value(&b).unwrap());
    }
}
#[test]
fn wire_metadata_can_be_edited_and_removed_without_stale_source_properties() {
    let source = json!({"name":"annotated","block_type":"Composite","category":"composite","inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],"blocks":[{"name":"gain","block_type":"Gain","inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}]}],"connections":[{"from":"u","to":"gain.u","ace:notes":"wire note","https://example.org/#tag":3},{"from":"gain.y","to":"y"}]});
    let doc = cdl_compat::from_value(&source, Default::default()).unwrap();
    let a = cxf::to_value(&doc).unwrap();
    let mut second = cxf::from_value(&a, &cxf::core_library()).unwrap();
    let body = second
        .library
        .classes
        .get_mut(&second.root.class)
        .unwrap()
        .body
        .as_mut()
        .unwrap();
    let wire = body
        .connections
        .iter_mut()
        .find(|w| w.meta.annotations.contains_key("ace:notes"))
        .unwrap();
    assert_eq!(
        wire.meta.annotations.shift_remove("ace:notes"),
        Some(Annotation::String("wire note".into()))
    );
    wire.meta.annotations.clear();
    let b = cxf::to_value(&second).unwrap();
    assert!(b["@graph"]
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["@type"] != "ace:Connection"));
}
#[test]
fn metadata_only_parameter_overrides_and_unbound_defaults_are_not_materialized_as_bindings() {
    let mut doc=cdl_compat::from_value(&json!({"name":"gain","block_type":"Buildings.Controls.OBC.CDL.Reals.MultiplyByParameter","inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],"parameters":[{"name":"k","value":2}]}),Default::default()).unwrap();
    doc.root.assignments.clear();
    doc.root.meta = Meta::default();
    doc.root.parameters = doc.library.classes[&doc.root.class].parameters.clone();
    let p = &mut doc.root.parameters[0];
    p.meta.description = Some("instance documentation".into());
    p.default = Some(Value::Real(7.0));
    p.units.quantity_kind = Some("http://qudt.org/vocab/quantitykind#DimensionlessRatio".into());
    let quantity_kind = p.units.quantity_kind.clone();
    let exported = cxf::to_value(&doc).unwrap();
    let second = cxf::from_value(&exported, &cxf::core_library()).unwrap();
    assert!(second.root.assignments.is_empty());
    assert_eq!(second.root.parameters[0].default, Some(Value::Real(7.0)));
    assert_eq!(
        second.root.parameters[0].meta.description.as_deref(),
        Some("instance documentation")
    );
    assert_eq!(second.root.parameters[0].units.quantity_kind, quantity_kind);
}
#[test]
fn value_annotations_belong_to_the_binding_and_can_be_removed_independently() {
    let source = json!({"@graph":[{"@id":"urn:gain","@type":"http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.MultiplyByParameter","S231:label":"gain","S231:hasInstance":[{"@id":"urn:gain.k"}]},{"@id":"urn:gain.k","S231:value":2,"https://example.org/#quality":"verified"}],"ace:rootBlock":{"@id":"urn:gain"}});
    let mut doc = imported(source);
    let binding = &mut doc.root.assignments[0];
    assert_eq!(
        binding
            .meta
            .annotations
            .shift_remove("https://example.org/#quality"),
        Some(Annotation::String("verified".into()))
    );
    assert!(!doc.root.parameters.iter().any(|p| p
        .meta
        .annotations
        .contains_key("https://example.org/#quality")));
    let exported = cxf::to_value(&doc).unwrap();
    let second = cxf::from_value(&exported, &cxf::core_library()).unwrap();
    assert!(second.root.assignments[0].meta.annotations.is_empty());
}
#[test]
fn complete_connector_overrides_can_clear_inherited_labels_and_units() {
    let mut class = BlockClass::new("https://example.org/#Override", BlockKind::Extension);
    let mut p = Connector::new("u", Direction::Input, DataType::Real);
    p.meta.label = Some("inherited".into());
    p.units.unit = Some("http://qudt.org/vocab/unit#K".into());
    class.inputs.push(p.clone());
    let mut library = Library::default();
    library.classes.insert(class.id.clone(), class.clone());
    let mut root = Instance::new("override", class.id);
    p.meta = Meta::default();
    p.units = Units::default();
    root.connectors.push(p.clone());
    let doc = Document { library, root };
    let second = cxf::from_value(&cxf::to_value(&doc).unwrap(), &Library::default()).unwrap();
    assert_eq!(second.root.connectors, vec![p]);
}
#[test]
fn array_extent_expressions_keep_function_argument_commas_and_typed_rank() {
    let source = json!({"@graph":[{"@id":"urn:array","@type":"S231:ExtensionBlock","S231:hasInput":{"@id":"urn:array.u"}},{"@id":"urn:array.u","@type":"S231:RealInput","S231:isArray":{"@value":"true","@type":"http://www.w3.org/2001/XMLSchema#boolean"},"S231:numberDimensions":{"@value":"2","@type":"http://www.w3.org/2001/XMLSchema#integer"},"S231:sizeOfDimensions":"(size(a[1,2], 1), n)"}]});
    let doc = imported(source);
    assert_eq!(
        doc.library.classes[&doc.root.class].inputs[0].dimensions,
        vec![
            Dimension::Expr("size(a[1,2], 1)".into()),
            Dimension::Expr("n".into())
        ]
    );
    let core = cxf::core_library();
    let id =
        ClassId::from("http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.MatrixMax");
    assert_eq!(
        core.classes[&id].outputs[0].dimensions,
        vec![Dimension::Expr(
            "if rowMax then size(u,1) else size(u,2)".into()
        )]
    );
}

#[test]
fn constants_and_cleared_instance_defaults_keep_their_ownership() {
    let source = json!({"@graph":[
        {"@id":"urn:class","@type":"S231:ExtensionBlock","S231:hasParameter":{"@id":"urn:class.p"},"S231:hasConstant":{"@id":"urn:class.c"}},
        {"@id":"urn:class.p","@type":"S231:Parameter","S231:isOfDataType":{"@id":"S231:Real"},"S231:value":1},
        {"@id":"urn:class.c","@type":"S231:Constant","S231:isOfDataType":{"@id":"S231:Real"},"S231:value":2}
    ]});
    let mut doc = imported(source);
    doc.root.assignments.clear();
    let mut declaration = doc.library.classes[&doc.root.class].parameters[0].clone();
    declaration.default = None;
    doc.root.parameters.push(declaration.clone());
    doc.root.meta.annotations.insert(
        "ace:cxf:source".into(),
        Annotation::String("user annotation".into()),
    );
    let exported = cxf::to_value(&doc).unwrap();
    assert!(exported["@graph"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["@type"] == "S231:Constant"));
    let second = cxf::from_value(&exported, &Library::default()).unwrap();
    assert_eq!(second.root.parameters, vec![declaration]);
    assert!(second.root.assignments.is_empty());
    assert_eq!(second.root.meta, doc.root.meta);
    assert_eq!(
        second.library.classes[&second.root.class].constants,
        doc.library.classes[&doc.root.class].constants
    );
}

#[test]
fn generic_block_with_only_boundary_wiring_is_a_composite() {
    let source = json!({"@graph":[
        {"@id":"urn:identity","@type":"S231:Block","S231:hasInput":{"@id":"urn:identity.u"},"S231:hasOutput":{"@id":"urn:identity.y"}},
        {"@id":"urn:identity.u","@type":"S231:RealInput","S231:connectedTo":{"@id":"urn:identity.y"}},
        {"@id":"urn:identity.y","@type":"S231:RealOutput"}
    ]});
    let doc = imported(source);
    let class = &doc.library.classes[&doc.root.class];
    assert_eq!(class.kind, BlockKind::Composite);
    assert_eq!(class.body.as_ref().unwrap().connections.len(), 1);
    assert!(cdl_model::validate(&doc).is_empty());
    let exported = cxf::to_value(&doc).unwrap();
    let second = imported(exported.clone());
    assert_eq!(cxf::to_value(&second).unwrap(), exported);
}
