use super::*;

#[test]
fn unresolved_cxf_parameter_expression_is_rejected() {
    let graph: Block = serde_json::from_value(json!({
        "name": "gain", "block_type": "Buildings.Controls.OBC.CDL.Reals.MultiplyByParameter",
        "inputs": [{"name": "u", "type": "Real"}],
        "outputs": [{"name": "y", "type": "Real"}],
        "parameters": [{"name": "k", "value": "rootGain*2"}]
    }))
    .unwrap();
    assert!(validate(&graph)
        .iter()
        .any(|error| error.contains("unsupported parameter value")));
}

#[test]
fn all_examples_parse_and_validate() {
    for text in [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/obc_zone_status.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/obc_operation_mode.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/obc_group_status.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/obc_optimal_start_calculation.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/four_pipe_g36.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/east_controller.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/west_controller.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/north_controller.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/south_controller.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/corridor_controller.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/room_control_system.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/simple_temp_controller.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/basic_temp_control.json"
        )),
    ] {
        let graph: Block = serde_json::from_str(text).unwrap();
        assert!(
            validate(&graph).is_empty(),
            "{} {:?}",
            graph.name,
            validate(&graph)
        );
    }
}
