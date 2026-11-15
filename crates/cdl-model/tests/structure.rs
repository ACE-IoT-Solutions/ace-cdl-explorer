use cdl_model::*;

fn wire(a: Endpoint, b: Endpoint) -> Connection {
    Connection {
        a,
        b,
        meta: Meta::default(),
    }
}
fn fixture() -> Document {
    let mut leaf = BlockClass::new("urn:test:leaf", BlockKind::Elementary);
    leaf.inputs
        .push(Connector::new("u", Direction::Input, DataType::Real));
    leaf.outputs
        .push(Connector::new("y", Direction::Output, DataType::Real));
    let mut root = BlockClass::new("urn:test:root", BlockKind::Composite);
    root.inputs = leaf.inputs.clone();
    root.outputs = leaf.outputs.clone();
    root.body = Some(CompositeBody {
        instances: vec![
            Instance::new("b", leaf.id.clone()),
            Instance::new("a", leaf.id.clone()),
        ],
        connections: vec![
            wire(Endpoint::boundary("u"), Endpoint::child("a", "u")),
            // Deliberately reversed syntax: direction must come from connectors.
            wire(Endpoint::child("b", "u"), Endpoint::child("a", "y")),
            wire(Endpoint::child("b", "y"), Endpoint::boundary("y")),
        ],
    });
    let mut library = Library::default();
    library.classes.insert(leaf.id.clone(), leaf);
    library.classes.insert(root.id.clone(), root);
    Document {
        library,
        root: Instance::new("root", "urn:test:root"),
    }
}
fn root(d: &mut Document) -> &mut BlockClass {
    d.library.classes.get_mut("urn:test:root").unwrap()
}
fn has(d: &Document, code: DiagnosticCode) -> bool {
    validate(d).iter().any(|e| e.code == code)
}
#[test]
fn boundary_forwarding_and_reversed_wire_are_valid() {
    let d = fixture();
    assert_eq!(validate(&d), vec![]);
    assert_eq!(
        evaluation_order(&d.library.classes["urn:test:root"], &d.library).unwrap(),
        vec![1, 0]
    );
}
#[test]
fn traversal_expands_shared_class_independently_in_declaration_order() {
    let d = fixture();
    let flat = d.instances().unwrap();
    assert_eq!(
        flat.iter().map(|i| i.path.to_string()).collect::<Vec<_>>(),
        ["root", "root.b", "root.a"]
    );
    assert!(std::ptr::eq(flat[1].class, flat[2].class));
}
#[test]
fn path_helpers_reject_empty_segments() {
    let path = InstancePath::parse("a.b.c").unwrap();
    assert_eq!(path.parent().unwrap().to_string(), "a.b");
    assert_eq!(path.child("d").to_string(), "a.b.c.d");
    for bad in ["", "a..b", ".a", "a."] {
        assert!(InstancePath::parse(bad).is_err());
    }
}
#[test]
fn missing_child_or_port_is_diagnosed() {
    for endpoint in [
        Endpoint::child("missing", "y"),
        Endpoint::child("a", "missing"),
        Endpoint::boundary("missing"),
    ] {
        let mut d = fixture();
        root(&mut d).body.as_mut().unwrap().connections[0].a = endpoint;
        assert!(has(&d, DiagnosticCode::MissingEndpoint));
    }
}
#[test]
fn mismatched_data_types_and_dimensions_are_diagnosed() {
    for dimensions in [false, true] {
        let mut d = fixture();
        let leaf = d.library.classes.get_mut("urn:test:leaf").unwrap();
        if dimensions {
            leaf.inputs[0].dimensions = vec![Dimension::Size(2)];
        } else {
            leaf.inputs[0].data_type = DataType::Integer;
        }
        assert!(has(&d, DiagnosticCode::TypeMismatch));
    }
}
#[test]
fn sources_cannot_be_wired_together() {
    let mut d = fixture();
    root(&mut d).body.as_mut().unwrap().connections[0].b = Endpoint::child("a", "y");
    assert!(has(&d, DiagnosticCode::InvalidDirection));
}
#[test]
fn duplicate_drivers_are_diagnosed_regardless_of_wire_orientation() {
    let mut d = fixture();
    root(&mut d)
        .body
        .as_mut()
        .unwrap()
        .connections
        .push(wire(Endpoint::child("a", "u"), Endpoint::child("b", "y")));
    assert!(has(&d, DiagnosticCode::MultipleDrivers));
}
#[test]
fn missing_input_and_output_drivers_are_diagnosed() {
    for index in [0, 2] {
        let mut d = fixture();
        root(&mut d)
            .body
            .as_mut()
            .unwrap()
            .connections
            .remove(index);
        assert!(has(&d, DiagnosticCode::UndrivenInput));
    }
}
#[test]
fn optional_child_inputs_and_outputs_do_not_require_drivers() {
    let mut d = fixture();
    root(&mut d).body.as_mut().unwrap().connections.clear();
    root(&mut d).outputs[0].required = false;
    d.library.classes.get_mut("urn:test:leaf").unwrap().inputs[0].required = false;
    assert!(validate(&d).is_empty());
}
#[test]
fn algebraic_cycle_requires_an_explicit_loop_breaking_property() {
    let mut d = fixture();
    root(&mut d).body.as_mut().unwrap().connections[0].a = Endpoint::child("b", "y");
    assert!(has(&d, DiagnosticCode::AlgebraicCycle));
    d.library
        .classes
        .get_mut("urn:test:leaf")
        .unwrap()
        .breaks_algebraic_loop = true;
    assert!(validate(&d).is_empty());
}
#[test]
fn self_loop_is_algebraic_unless_the_class_breaks_it() {
    let mut d = fixture();
    root(&mut d).body.as_mut().unwrap().connections[0].a = Endpoint::child("a", "y");
    assert!(has(&d, DiagnosticCode::AlgebraicCycle));
}
#[test]
fn recursive_class_instantiation_terminates() {
    let mut d = fixture();
    root(&mut d)
        .body
        .as_mut()
        .unwrap()
        .instances
        .push(Instance::new("nested", "urn:test:root"));
    let errors = d.instances().unwrap_err();
    assert_eq!(errors[0].path, "root.nested");
    assert_eq!(errors[0].code, DiagnosticCode::RecursiveClass);
    assert!(has(&d, DiagnosticCode::RecursiveClass));
}
#[test]
fn unknown_root_and_child_classes_are_diagnosed() {
    let mut d = fixture();
    d.root.class = "urn:missing".into();
    assert!(has(&d, DiagnosticCode::UnknownClass));
    let mut d = fixture();
    root(&mut d).body.as_mut().unwrap().instances[0].class = "urn:missing".into();
    assert!(has(&d, DiagnosticCode::UnknownClass));
}
#[test]
fn duplicate_children_and_declarations_are_diagnosed() {
    let mut d = fixture();
    root(&mut d).body.as_mut().unwrap().instances[0].name = "a".into();
    assert!(has(&d, DiagnosticCode::DuplicateName));
    let mut d = fixture();
    let port = root(&mut d).inputs[0].clone();
    root(&mut d).inputs.push(port);
    assert!(has(&d, DiagnosticCode::DuplicateName));
}
#[test]
fn malformed_class_kind_and_map_key_are_diagnosed() {
    let mut d = fixture();
    root(&mut d).kind = BlockKind::Elementary;
    assert!(has(&d, DiagnosticCode::InvalidClass));
    let mut d = fixture();
    root(&mut d).id = "urn:other".into();
    assert!(has(&d, DiagnosticCode::InvalidClass));
}
#[test]
fn typed_bindings_validate_arrays_enumerations_and_expressions() {
    let mut d = fixture();
    d.library.enumerations.insert(
        "urn:mode".into(),
        Enumeration {
            id: "urn:mode".into(),
            members: vec!["P".into(), "PI".into()],
            meta: Meta::default(),
        },
    );
    let declaration = ParameterDecl {
        name: "modes".into(),
        data_type: DataType::Enumeration("urn:mode".into()),
        default: None,
        units: Units::default(),
        min: None,
        max: None,
        dimensions: vec![Dimension::Size(2)],
        meta: Meta::default(),
    };
    root(&mut d).parameters.push(declaration);
    d.root.assignments.push(Assignment {
        name: "modes".into(),
        value: Value::Array(vec![Value::Enum("P".into()), Value::Enum("PI".into())]),
        meta: Meta::default(),
    });
    assert!(validate(&d).is_empty());
    d.root.assignments[0].value = Value::Array(vec![Value::Enum("P".into())]);
    assert!(has(&d, DiagnosticCode::InvalidValue));
    d.root.assignments[0].value =
        Value::Array(vec![Value::Enum("P".into()), Value::Enum("bad".into())]);
    assert!(has(&d, DiagnosticCode::InvalidValue));
    d.root.assignments[0].value = Value::Expr("configuration".into());
    assert!(validate(&d).is_empty());
    d.root.assignments[0].name = "missing".into();
    assert!(has(&d, DiagnosticCode::UnknownParameter));
}
#[test]
fn serde_preserves_open_namespaced_annotation_values() {
    let mut d = fixture();
    let raw = serde_json::json!({"ace:notes":{"values":[null, true, -1, 18446744073709551615u64, 1.25,"x"]}});
    d.root.meta.annotations = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(&d.root.meta.annotations).unwrap(), raw);
    let encoded = serde_json::to_string(&d).unwrap();
    assert_eq!(serde_json::from_str::<Document>(&encoded).unwrap(), d);
}

#[test]
fn invalid_names_and_zero_dimensions_are_reported() {
    let mut d = fixture();
    d.root.name = "".into();
    assert!(has(&d, DiagnosticCode::InvalidName));
    let mut d = fixture();
    root(&mut d).inputs[0].name = "".into();
    assert!(has(&d, DiagnosticCode::InvalidName));
    let mut d = fixture();
    root(&mut d).inputs[0].dimensions = vec![Dimension::Size(0)];
    assert!(has(&d, DiagnosticCode::InvalidValue));
}
#[test]
fn malformed_unused_classes_and_instance_overrides_are_not_hidden() {
    let mut d = fixture();
    let mut unused = BlockClass::new("urn:unused", BlockKind::Composite);
    unused
        .body
        .as_mut()
        .unwrap()
        .instances
        .push(Instance::new("missing", "urn:missing"));
    d.library.classes.insert(unused.id.clone(), unused);
    assert!(has(&d, DiagnosticCode::UnknownClass));
    let mut d = fixture();
    d.root
        .connectors
        .push(Connector::new("u", Direction::Output, DataType::Real));
    assert!(has(&d, DiagnosticCode::InvalidDirection));
}
