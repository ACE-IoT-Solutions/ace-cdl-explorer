use super::*;

#[test]
fn timed_cdl_transitions_refine_building_step() {
    let mut graph: Block = serde_json::from_value(json!({
        "name": "timed", "block_type": "Composite", "blocks": [
            {"name": "delay", "block_type": "TrueDelay", "parameters": [
                {"name": "delayTime", "value": 15.0}
            ]},
            {"name": "hold", "block_type": "TrueFalseHold", "parameters": [
                {"name": "trueHoldDuration", "value": 300.0},
                {"name": "falseHoldDuration", "value": 0.0}
            ]}
        ]
    }))
    .unwrap();
    assert_eq!(sampled_step(&graph, 900).unwrap(), 15);
    graph.blocks[0].parameters[0].value = json!(30.0);
    assert_eq!(sampled_step(&graph, 900).unwrap(), 30);
    graph.blocks[1].parameters[0].value = json!(75.0);
    assert_eq!(sampled_step(&graph, 900).unwrap(), 15);
    graph.blocks[1].parameters[0].value = json!(17.0);
    assert!(sampled_step(&graph, 900).is_err());
}
