use cdl_exec::{Program, Scalar};
use cdl_model::{Annotation, DataType};
#[test]
fn old_ts_composite_headers_and_interface_variants_retain_executable_behavior() {
    let direct =
        cdl_compat::from_json(include_str!("legacy/graph.json"), Default::default()).unwrap();
    let imported = cxf::from_json(
        include_str!("legacy/old-export.jsonld"),
        &cxf::core_library(),
    )
    .unwrap();
    assert!(
        cdl_model::validate(&imported).is_empty(),
        "{:?}",
        cdl_model::validate(&imported)
    );
    assert!(imported
        .library
        .classes
        .values()
        .any(|c| c.meta.annotations.get("ace:interfaceOnly") == Some(&Annotation::Boolean(true))));
    assert!(imported
        .library
        .classes
        .values()
        .any(|c| c.meta.annotations.contains_key("ace:baseClass")));
    for document in [direct, imported] {
        let program = Program::compile(
            &document,
            &cdl_blocks::Registry::new(document.library.clone()),
        )
        .unwrap();
        let mut state = program.instantiate();
        state.set_input("a", Scalar::Real(4.0)).unwrap();
        state.set_input("b", Scalar::Real(1.0)).unwrap();
        state.set_input("choice", Scalar::Boolean(true)).unwrap();
        state.step(1.0).unwrap();
        assert_eq!(state.output("y").unwrap(), Scalar::Real(7.0));
        state.set_input("choice", Scalar::Boolean(false)).unwrap();
        state.step(1.0).unwrap();
        assert_eq!(state.output("y").unwrap(), Scalar::Real(1.0));
        assert_eq!(program.output("y").unwrap().data_type(), &DataType::Real);
    }
}
