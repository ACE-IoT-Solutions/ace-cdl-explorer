use cdl_blocks::{implicit_assignment_unchanged, ConfiguredInstance, Registry, SlotAlloc, Step};
use cdl_compat::{from_value, to_value, ReadOptions};
use cdl_model::Value;
use serde_json::json;

#[test]
fn omitted_constant_binding_is_explicit_in_the_model_and_invisible_in_legacy_json() {
    let source = json!({"name":"c", "block_type":"Buildings.Controls.OBC.CDL.Reals.Sources.Constant",
        "outputs":[{"name":"y", "type":"Real"}]});
    let mut document = from_value(&source, ReadOptions::default()).unwrap();
    let binding = document
        .root
        .assignments
        .iter()
        .find(|p| p.name == "k")
        .unwrap();
    assert_eq!(binding.value, Value::Real(0.0));
    assert!(implicit_assignment_unchanged(binding));
    assert_eq!(to_value(&document).unwrap(), source);
    // Removing compatibility mode exercises the standard class with the explicit
    // legacy choice, rather than accidentally inheriting a different default.
    document
        .root
        .meta
        .annotations
        .shift_remove("ace:compat:fields");
    let registry = Registry::new(document.library.clone());
    let class = &document.library.classes[&document.root.class];
    let config = ConfiguredInstance::new(&document.root, class, "c");
    let mut io = SlotAlloc::default();
    io.bind_output("y", 0);
    let kernel = registry.build(&config, &io).unwrap();
    let mut slots = [99.0];
    kernel.initialize(&mut slots);
    assert_eq!(slots[0], 0.0);
    document
        .root
        .assignments
        .iter_mut()
        .find(|p| p.name == "k")
        .unwrap()
        .value = Value::Real(7.5);
    assert_eq!(to_value(&document).unwrap()["parameters"][0]["value"], 7.5);
}

#[test]
fn native_standard_required_bindings_are_diagnosed() {
    let library = cdl_compat::pinned_core_library();
    let id = cdl_model::ClassId::from(
        "http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.Sources.Constant",
    );
    let instance = cdl_model::Instance::new("c", id.clone());
    let config = ConfiguredInstance::new(&instance, &library.classes[&id], "c");
    let registry = Registry::new(library.clone());
    let mut io = SlotAlloc::default();
    io.bind_output("y", 0);
    assert!(registry
        .build(&config, &io)
        .err()
        .unwrap()
        .to_string()
        .contains("missing required parameter k"));
}

#[test]
fn dependent_hold_fallback_uses_the_supplied_duration() {
    let source = json!({"name":"hold", "block_type":"Buildings.Controls.OBC.CDL.Logical.TrueFalseHold",
        "inputs":[{"name":"u","type":"Boolean"}], "outputs":[{"name":"y","type":"Boolean"}],
        "parameters":[{"name":"trueHoldDuration","value":13.0}]});
    let document = from_value(&source, ReadOptions::default()).unwrap();
    assert_eq!(
        document
            .root
            .assignments
            .iter()
            .find(|p| p.name == "falseHoldDuration")
            .unwrap()
            .value,
        Value::Real(13.0)
    );
    assert_eq!(to_value(&document).unwrap(), source);
}

#[test]
fn invalid_sampled_and_timed_instances_do_not_gain_validating_defaults() {
    for (class, parameter) in [
        ("Discrete.UnitDelay", "samplePeriod"),
        ("Discrete.TriggeredMovingMean", "n"),
        ("Logical.TrueFalseHold", "trueHoldDuration"),
    ] {
        let source = json!({"name":"invalid", "block_type":format!("Buildings.Controls.OBC.CDL.{class}"),
            "inputs":[{"name":"u","type":"Real"}], "outputs":[{"name":"y","type":"Real"}]});
        let document = from_value(&source, ReadOptions::default()).unwrap();
        assert!(!document
            .root
            .assignments
            .iter()
            .any(|p| p.name == parameter));
        assert_eq!(to_value(&document).unwrap(), source);
    }
}

#[test]
fn compound_unit_tokens_are_not_shortened_to_a_known_denominator() {
    use cdl_model::{
        BlockClass, BlockKind, Connector, DataType, Direction, Document, Instance, Library,
    };
    let mut class = BlockClass::new("https://example.org/UnitExample", BlockKind::Extension);
    let mut port = Connector::new("u", Direction::Input, DataType::Real);
    port.units.unit = Some("http://qudt.org/vocab/unit#1/K".into());
    class.inputs.push(port);
    let mut library = Library::default();
    library.classes.insert(class.id.clone(), class.clone());
    let doc = Document {
        library,
        root: Instance::new("example", class.id),
    };
    assert_eq!(to_value(&doc).unwrap()["inputs"][0]["unit"], "1/K");
}
