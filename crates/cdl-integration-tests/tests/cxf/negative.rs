use serde_json::json;
#[test]
fn malformed_contexts_ids_and_references_are_explicit_errors() {
    for value in [
        json!({"@graph":[{"@id":"urn:root","@type":"S231:ExtensionBlock","ace:compat:fields":["name"]}]}),
        json!({}),
        json!({"@context":{"ex":{"@id":"https://example.org/#","@prefix":false}},"@graph":[{"@id":"urn:root","@type":"S231:ExtensionBlock"}]}),
        json!({"@context":"https://example.org/remote","@graph":[]}),
        json!({"@context":{"@base":"https://example.org/"},"@graph":[]}),
        json!({"@graph":[{"@id":"urn:root","@type":"S231:Block","S231:hasInput":[{}]}]}),
        json!({"@graph":[{"@id":"urn:root"},{"@id":"urn:root"}]}),
        json!({"@graph":[{"@id":"urn:root","@type":"S231:Block","S231:hasInput":{"@id":"urn:missing"}}]}),
        json!({"@graph":[{"@id":"urn:root","@type":"urn:missing"}],"ace:rootBlock":{"@id":"urn:root"}}),
        json!({"@graph":[{"@id":"urn:root","@type":"S231:Block","S231:hasInput":{"@id":"urn:root.u"}},{"@id":"urn:root.u","@type":"S231:RealInput","S231:isArray":true,"S231:numberDimensions":u64::MAX}]}),
    ] {
        assert!(
            cxf::from_value(&value, &cxf::core_library()).is_err(),
            "{value}"
        );
    }
}
#[test]
fn ambiguous_wire_direction_is_rejected_instead_of_guessing() {
    let v = json!({"@graph":[{"@id":"urn:root","@type":"S231:CompositeBlock","S231:hasInput":[{"@id":"urn:root.a"},{"@id":"urn:root.b"}]},
 {"@id":"urn:root.a","@type":"S231:RealInput","S231:isConnectedTo":{"@id":"urn:root.b"}},{"@id":"urn:root.b","@type":"S231:RealInput"}]});
    let errors = cxf::from_value(&v, &cxf::core_library()).unwrap_err();
    assert_eq!(errors[0].code, cdl_model::DiagnosticCode::InvalidDirection);
}
#[test]
fn multiple_roots_require_selection_and_malformed_percent_names_do_not_panic() {
    let v = json!({"@graph":[{"@id":"urn:first","@type":"S231:ExtensionBlock"},{"@id":"urn:second","@type":"S231:ExtensionBlock","S231:hasInput":{"@id":"urn:second.%😀"}},{"@id":"urn:second.%😀","@type":"S231:RealInput"}]});
    assert!(cxf::from_value(&v, &cxf::core_library()).is_err());
    let doc = cxf::from_value_with_root(&v, &cxf::core_library(), "urn:second").unwrap();
    assert_eq!(doc.library.classes[&doc.root.class].inputs[0].name, "%😀");
}
