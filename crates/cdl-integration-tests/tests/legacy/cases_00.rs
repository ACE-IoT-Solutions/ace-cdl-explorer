use super::*;

#[test]
fn pid_with_reset_uses_rising_fan_proof_in_both_executors() {
    let graph: Block = serde_json::from_value(json!({
        "name":"DamperLoop","block_type":"Composite","category":"composite",
        "inputs":[{"name":"u_s","type":"Real"},{"name":"u_m","type":"Real"},{"name":"fan","type":"Boolean"}],
        "outputs":[{"name":"y","type":"Real"}],
        "blocks":[{"name":"pi","block_type":"Buildings.Controls.OBC.CDL.Reals.PIDWithReset","category":"elementary",
            "inputs":[{"name":"u_s","type":"Real"},{"name":"u_m","type":"Real"},{"name":"trigger","type":"Boolean"}],
            "outputs":[{"name":"y","type":"Real"}],
            "parameters":[{"name":"k","value":0.5},{"name":"Ti","value":300.0},{"name":"r","value":1.0},
                {"name":"Ni","value":0.9},{"name":"yMin","value":0.0},{"name":"yMax","value":1.0},
                {"name":"y_reset","value":0.01},{"name":"reverseActing","value":true,"type":"Boolean"}]}],
        "connections":[{"from":"u_s","to":"pi.u_s"},{"from":"u_m","to":"pi.u_m"},
            {"from":"fan","to":"pi.trigger"},{"from":"pi.y","to":"y"}]
    })).unwrap();
    assert!(validate(&graph).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    let samples = [
        (false, 0.0, 0.0),
        (true, 0.0, 0.01),
        (true, 0.2, 0.13),
        (true, 0.2, 0.15),
        (false, 0.2, 0.17),
        (true, 0.2, 0.01),
    ];
    for (fan, setpoint, expected) in samples {
        let inputs = HashMap::from([
            ("u_s".into(), json!(setpoint)),
            ("u_m".into(), json!(0.0)),
            ("fan".into(), json!(fan)),
        ]);
        let actual = dynamic.execute(&graph, &inputs, 60.0, "").unwrap()["y"]
            .as_f64()
            .unwrap();
        let faster = compiled.execute(&inputs, 60.0, false).unwrap().0["y"]
            .as_f64()
            .unwrap();
        assert!((actual - faster).abs() < 1e-12);
        assert!(
            (actual - expected).abs() < 1e-12,
            "fan={fan} setpoint={setpoint}: {actual}"
        );
    }
}

#[test]
fn pid_with_reset_controller_types_match_dynamic_and_compiled_execution() {
    for kind in ["P", "PI", "PD", "PID"] {
        let graph: Block = serde_json::from_value(json!({
            "name":"Controller","block_type":"Composite","category":"composite",
            "inputs":[{"name":"u_s","type":"Real"},{"name":"u_m","type":"Real"},{"name":"trigger","type":"Boolean"}],
            "outputs":[{"name":"y","type":"Real"}],
            "blocks":[{"name":"controller","block_type":"Buildings.Controls.OBC.CDL.Reals.PIDWithReset","category":"elementary",
                "inputs":[{"name":"u_s","type":"Real"},{"name":"u_m","type":"Real"},{"name":"trigger","type":"Boolean"}],
                "outputs":[{"name":"y","type":"Real"}],
                "parameters":[{"name":"controllerType","value":kind},{"name":"k","value":0.5},
                    {"name":"Ti","value":20.0},{"name":"Td","value":2.0},{"name":"Nd","value":4.0},
                    {"name":"r","value":1.0},{"name":"Ni","value":0.9},
                    {"name":"yMin","value":-10.0},{"name":"yMax","value":10.0},
                    {"name":"y_reset","value":0.1},{"name":"reverseActing","value":true}]}],
            "connections":[{"from":"u_s","to":"controller.u_s"},{"from":"u_m","to":"controller.u_m"},
                {"from":"trigger","to":"controller.trigger"},{"from":"controller.y","to":"y"}]
        })).unwrap();
        assert!(validate(&graph).is_empty(), "{kind}");
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        let mut values = Vec::new();
        for (setpoint, trigger) in [
            (0.0, false),
            (1.0, false),
            (1.0, true),
            (1.0, true),
            (0.0, false),
        ] {
            let inputs = HashMap::from([
                ("u_s".into(), json!(setpoint)),
                ("u_m".into(), json!(0.0)),
                ("trigger".into(), json!(trigger)),
            ]);
            let ordinary = dynamic.execute(&graph, &inputs, 0.5, "").unwrap()["y"]
                .as_f64()
                .unwrap();
            let fast = compiled.execute(&inputs, 0.5, false).unwrap().0["y"]
                .as_f64()
                .unwrap();
            assert!(
                (ordinary - fast).abs() < 1e-12,
                "{kind}: {ordinary} vs {fast}"
            );
            values.push(ordinary);
        }
        if kind == "P" {
            assert_eq!(values, vec![0.0, 0.5, 0.5, 0.5, 0.0]);
        }
        if kind == "PD" {
            let d = 2.0 * (-1.0_f64).exp();
            assert!((values[1] - (0.5 + d)).abs() < 1e-12);
            assert!(values[2] > 0.5);
        }
        if matches!(kind, "PI" | "PID") {
            assert!(
                (values[2] - 0.1).abs() < 1e-12,
                "{kind}: reset={}",
                values[2]
            );
        }
    }
    let invalid: Block = serde_json::from_value(json!({
        "name":"bad","block_type":"Buildings.Controls.OBC.CDL.Reals.PIDWithReset",
        "inputs":[{"name":"u_s","type":"Real"},{"name":"u_m","type":"Real"},{"name":"trigger","type":"Boolean"}],
        "outputs":[{"name":"y","type":"Real"}],
        "parameters":[{"name":"controllerType","value":"PX"}]
    })).unwrap();
    assert!(validate(&invalid)
        .iter()
        .any(|error| error.contains("controllerType")));
}

#[test]
fn pid_with_reset_pi_and_pid_match_pinned_obc_trace() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_pid_with_reset_dymola.txt"
    ));
    let series = |name: &str| -> Vec<f64> {
        let line = source.lines().find(|line| line.starts_with(name)).unwrap();
        line.split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|field| field.trim().parse::<f64>().unwrap())
            .collect()
    };
    for (name, reset, kind) in [
        ("limPIPar", 0.5, "PI"),
        ("limPIInp", 0.75, "PI"),
        ("limPIDPar", 0.5, "PID"),
        ("limPIDInp", 0.75, "PID"),
    ] {
        let measured = series(&format!("{name}.u_m="));
        let reference = series(&format!("{name}.y="));
        assert_eq!((measured.len(), reference.len()), (101, 101));
        let block: Block = serde_json::from_value(json!({
            "name": "pi", "block_type": "Buildings.Controls.OBC.CDL.Reals.PIDWithReset",
            "inputs": [{"name":"u_s","type":"Real"},{"name":"u_m","type":"Real"},
                {"name":"trigger","type":"Boolean"}],
            "outputs": [{"name":"y","type":"Real"}],
            "parameters": [{"name":"controllerType","value":kind},
                {"name":"k","value":1.0},{"name":"Ti","value":1.0},
                {"name":"Td","value":1.0},{"name":"Nd","value":10.0},
                {"name":"Ni","value":0.9},{"name":"yMin","value":-1.0},
                {"name":"yMax","value":1.0},{"name":"y_reset","value":reset},
                {"name":"reverseActing","type":"Boolean","value":true}]
        }))
        .unwrap();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&block).unwrap().unwrap();
        let mut max_error = 0.0_f64;
        let mut max_step = 0;
        for step in 0..=10_000 {
            let sample = (step / 100).min(100);
            let next = (sample + 1).min(100);
            let fraction = (step % 100) as f64 / 100.0;
            let measurement = measured[sample] * (1.0 - fraction) + measured[next] * fraction;
            let triggered = step >= 1_000;
            let inputs = HashMap::from([
                ("u_s".into(), json!(if triggered { 1.0 } else { 0.0 })),
                ("u_m".into(), json!(measurement)),
                ("trigger".into(), json!(triggered)),
            ]);
            let regular = dynamic.execute(&block, &inputs, 0.001, "").unwrap()["y"]
                .as_f64()
                .unwrap();
            let faster = compiled.execute(&inputs, 0.001, false).unwrap().0["y"]
                .as_f64()
                .unwrap();
            assert!((regular - faster).abs() < 1e-12, "{name} step {step}");
            // Dymola records t=1 immediately before the rising-edge reset.
            if step % 100 == 0 && step != 1_000 {
                let error = (regular - reference[step / 100]).abs();
                if error > max_error {
                    max_error = error;
                    max_step = step;
                }
            }
        }
        let tolerance = if kind == "PID" { 0.05 } else { 5e-4 };
        assert!(
            max_error < tolerance,
            "{name}: max error {max_error} at {max_step}"
        );
    }
}

#[test]
fn g36_vav_damper_loops_reset_from_physical_fan_proof() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    assert!(validate(&graph).is_empty());
    for zone in graph
        .blocks
        .iter()
        .filter(|block| block.block_type == "VAVBoxController")
    {
        let pi = zone
            .blocks
            .iter()
            .find(|block| block.name == "damper_flow_pi")
            .unwrap();
        assert_eq!(
            pi.block_type,
            "Buildings.Controls.OBC.CDL.Reals.PIDWithReset"
        );
        assert_eq!(param(pi, "y_reset", -1.0), 0.01);
        assert!(zone.connections.iter().any(|edge| {
            edge.source() == ("supply_fan_proven".into(), String::new())
                && edge.target() == ("damper_flow_pi".into(), "trigger".into())
        }));
        assert!(CompiledExecutor::try_new(zone).unwrap().is_some());
    }
}

#[test]
fn boolean_to_real_matches_obc_parameters_in_both_executors() {
    let graph: Block = serde_json::from_value(json!({
        "name": "bool_to_real",
        "block_type": "Buildings.Controls.OBC.CDL.Conversions.BooleanToReal",
        "inputs": [{"name": "u", "type": "Boolean"}],
        "outputs": [{"name": "y", "type": "Real"}],
        "parameters": [{"name": "realTrue", "value": 2.5}, {"name": "realFalse", "value": -0.5}]
    }))
    .unwrap();
    assert!(validate(&graph).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (input, expected) in [(false, -0.5), (true, 2.5), (false, -0.5)] {
        let values = HashMap::from([("u".into(), json!(input))]);
        assert_eq!(
            dynamic.execute(&graph, &values, 60.0, "").unwrap()["y"],
            json!(expected)
        );
        assert_eq!(
            compiled.execute(&values, 60.0, false).unwrap().0["y"],
            json!(expected)
        );
    }
}

#[test]
fn vav_shortcut_requires_expansion_into_cdl_blocks() {
    let legacy: Block = serde_json::from_value(json!({
        "name": "legacy_vav",
        "block_type": "VAVBoxController",
        "inputs": [{"name": "room_temp", "type": "Real"}],
        "outputs": [{"name": "damper_position", "type": "Real"}]
    }))
    .unwrap();
    assert!(validate(&legacy)
        .iter()
        .any(|error| error.contains("unsupported class") && error.contains("VAVBoxController")));
    assert!(CompiledExecutor::try_new(&legacy).is_err());
    assert!(Executor::default()
        .execute(&legacy, &HashMap::new(), 900.0, "")
        .is_err());
}

#[test]
fn true_false_hold_respects_both_minimum_durations_in_both_executors() {
    let graph: Block = serde_json::from_value(json!({
        "name":"hold", "block_type":"Buildings.Controls.OBC.CDL.Logical.TrueFalseHold",
        "inputs":[{"name":"u","type":"Boolean"}],
        "outputs":[{"name":"y","type":"Boolean"}],
        "parameters":[{"name":"trueHoldDuration","value":120.0},
                      {"name":"falseHoldDuration","value":60.0}]
    }))
    .unwrap();
    assert!(validate(&graph).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (input, expected) in [
        (false, false),
        (true, false),
        (true, true),
        (false, true),
        (false, true),
        (false, true),
        (false, false),
        (true, false),
        (true, true),
    ] {
        let inputs = HashMap::from([("u".into(), json!(input))]);
        assert_eq!(
            dynamic.execute(&graph, &inputs, 30.0, "").unwrap()["y"],
            json!(expected)
        );
        assert_eq!(
            compiled.execute(&inputs, 30.0, false).unwrap().0["y"],
            json!(expected)
        );
    }
    let mut invalid = graph.clone();
    invalid.parameters[0].value = json!(-1.0);
    assert!(!validate(&invalid).is_empty());
}

#[test]
fn triggered_cdl_blocks_match_in_dynamic_and_compiled_execution() {
    let cases = [
        (
            "TriggeredSampler",
            json!([{"name":"y_start","value":-1}]),
            vec![
                (1.0, false, -1.0),
                (2.0, true, 2.0),
                (3.0, true, 2.0),
                (4.0, false, 2.0),
                (5.0, true, 5.0),
            ],
        ),
        (
            "TriggeredMovingMean",
            json!([{"name":"n","value":2}]),
            vec![
                (1.0, false, 1.0),
                (2.0, true, 1.5),
                (3.0, true, 1.5),
                (4.0, false, 1.5),
                (5.0, true, 3.5),
            ],
        ),
    ];
    for (kind, parameters, sequence) in cases {
        let graph: Block = serde_json::from_value(json!({
            "name":"sampler", "block_type":format!("Buildings.Controls.OBC.CDL.Discrete.{kind}"),
            "inputs":[{"name":"u","type":"Real"},{"name":"trigger","type":"Boolean"}],
            "outputs":[{"name":"y","type":"Real"}], "parameters":parameters,
        }))
        .unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (value, trigger, expected) in sequence {
            let inputs = HashMap::from([
                ("u".into(), json!(value)),
                ("trigger".into(), json!(trigger)),
            ]);
            assert_eq!(
                dynamic.execute(&graph, &inputs, 15.0, "").unwrap()["y"],
                json!(expected)
            );
            assert_eq!(
                compiled.execute(&inputs, 15.0, false).unwrap().0["y"],
                json!(expected)
            );
        }
    }
}

#[test]
fn cdl_edges_and_timer_match_in_dynamic_and_compiled_execution() {
    for (kind, expected) in [
        ("Edge", vec![false, true, false, false, true]),
        ("FallingEdge", vec![false, false, false, true, false]),
    ] {
        let graph: Block = serde_json::from_value(json!({
            "name":"edge", "block_type":format!("Buildings.Controls.OBC.CDL.Logical.{kind}"),
            "inputs":[{"name":"u","type":"Boolean"}],
            "outputs":[{"name":"y","type":"Boolean"}],
        }))
        .unwrap();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (active, wanted) in [false, true, true, false, true].into_iter().zip(expected) {
            let inputs = HashMap::from([("u".into(), json!(active))]);
            assert_eq!(
                dynamic.execute(&graph, &inputs, 15.0, "").unwrap()["y"],
                json!(wanted)
            );
            assert_eq!(
                compiled.execute(&inputs, 15.0, false).unwrap().0["y"],
                json!(wanted)
            );
        }
    }
    let timer: Block = serde_json::from_value(json!({
        "name":"timer", "block_type":"Buildings.Controls.OBC.CDL.Logical.Timer",
        "inputs":[{"name":"u","type":"Boolean"}],
        "outputs":[{"name":"y","type":"Real"},{"name":"passed","type":"Boolean"}],
        "parameters":[{"name":"t","value":30}],
    }))
    .unwrap();
    assert!(validate(&timer).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&timer).unwrap().unwrap();
    for (active, seconds, passed) in [
        (false, 0.0, false),
        (true, 0.0, false),
        (true, 15.0, false),
        (true, 30.0, true),
        (false, 0.0, false),
        (true, 0.0, false),
    ] {
        let inputs = HashMap::from([("u".into(), json!(active))]);
        let reference = dynamic.execute(&timer, &inputs, 15.0, "").unwrap();
        let optimized = compiled.execute(&inputs, 15.0, false).unwrap().0;
        assert_eq!(reference["y"], json!(seconds));
        assert_eq!(reference["passed"], json!(passed));
        assert_eq!(optimized["y"], json!(seconds));
        assert_eq!(optimized["passed"], json!(passed));
    }
}

#[test]
fn g36_damper_zero_flow_override_uses_pi_until_unoccupied() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = building
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let mut inputs: HashMap<String, Value> = zone
        .inputs
        .iter()
        .map(|port| {
            (
                port.name.clone(),
                if port.kind == "Boolean" {
                    json!(false)
                } else if port.kind == "Integer" {
                    json!(0)
                } else {
                    json!(22.0)
                },
            )
        })
        .collect();
    inputs.insert("scheduled_occupied".into(), json!(true));
    inputs.insert("supply_fan_proven".into(), json!(true));
    inputs.insert("measured_airflow".into(), json!(0.01));
    inputs.insert("flow_override_code".into(), json!(2));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
    let mut previous = 0.0;
    for _ in 0..10 {
        let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow["damper_position"], fast["damper_position"]);
        previous = slow["damper_position"].as_f64().unwrap();
    }
    assert!(previous > 0.5, "failed to prime damper PI: {previous}");
    inputs.insert("flow_override_code".into(), json!(1));
    let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
    let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(slow["airflow_request"], json!(0.0));
    assert_eq!(slow["damper_position"], fast["damper_position"]);
    assert!(
        slow["damper_position"].as_f64().unwrap() > 0.0,
        "zero-flow override must not bypass the CDL PI while occupied"
    );
    inputs.insert("scheduled_occupied".into(), json!(false));
    let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
    let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(slow["damper_position"], json!(0.0));
    assert_eq!(slow["damper_position"], fast["damper_position"]);
}

#[test]
fn g36_vav_flow_overrides_replay_pinned_damper_valves_setpoint_trace() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut zone = building
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap()
        .clone();
    for (name, value) in [
        ("flow_override_cooling_max", 0.09),
        ("flow_override_design_min", 0.01),
        ("flow_override_heating_max", 0.08),
    ] {
        zone.blocks
            .iter_mut()
            .find(|block| block.name == name)
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(value);
    }
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_damper_valves_dymola.txt"
    ));
    let series = |name: &str| -> Vec<f64> {
        archive
            .lines()
            .find(|line| line.starts_with(name))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|field| field.trim().parse::<f64>().unwrap())
            .collect()
    };
    let override_code = series("damValFan1.oveFloSet=");
    let reference_flow = series("damValFan1.VSet_flow=");
    assert_eq!((override_code.len(), reference_flow.len()), (101, 101));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&zone).unwrap().unwrap();
    let mut coverage = [0usize; 4];
    for (index, (&code, &expected)) in override_code.iter().zip(reference_flow.iter()).enumerate() {
        if !(1.0..=3.0).contains(&code) {
            continue;
        }
        coverage[code as usize] += 1;
        let mut inputs: HashMap<String, Value> = zone
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else if port.kind == "Integer" {
                        json!(0)
                    } else {
                        json!(22.0)
                    },
                )
            })
            .collect();
        inputs.insert("flow_override_code".into(), json!(code as i64));
        let slow = dynamic.execute(&zone, &inputs, 72.0, "").unwrap();
        let fast = compiled.execute(&inputs, 72.0, false).unwrap().0;
        let actual = slow["airflow_request"].as_f64().unwrap();
        assert!(
            (actual - expected).abs() < 1e-6,
            "sample {index} code {code}: {actual} vs {expected}"
        );
        assert_eq!(slow["airflow_request"], fast["airflow_request"]);
    }
    assert!(
        coverage[1] > 0 && coverage[2] > 0 && coverage[3] > 0,
        "{coverage:?}"
    );
    let mut inputs: HashMap<String, Value> = zone
        .inputs
        .iter()
        .map(|port| {
            (
                port.name.clone(),
                if port.kind == "Boolean" {
                    json!(false)
                } else if port.kind == "Integer" {
                    json!(0)
                } else {
                    json!(22.0)
                },
            )
        })
        .collect();
    inputs.insert("flow_override_code".into(), json!(4));
    assert_eq!(
        dynamic.execute(&zone, &inputs, 72.0, "").unwrap()["airflow_request"],
        json!(0.08)
    );
}

#[test]
fn g36_normal_vav_airflow_replays_unforced_obc_trace() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = building
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let normal = zone
        .blocks
        .iter()
        .find(|block| block.name == "normal_airflow_sequence")
        .unwrap();
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_damper_valves_dymola.txt"
    ));
    let series = |name: &str| -> Vec<f64> {
        archive
            .lines()
            .find(|line| line.starts_with(name))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|field| field.trim().parse::<f64>().unwrap())
            .collect()
    };
    let code = series("damValFan1.oveFloSet=");
    let expected = series("damValFan1.VSet_flow=");
    let cooling = series("damValFan1.uCoo=");
    let heating = series("damValFan1.uHea=");
    let supply = series("damValFan1.TSup=");
    let zone_temp = series("damValFan1.TZon=")[0] - 273.15;
    let discharge = series("damValFan1.TDis=");
    let cooling_min = series("damValFan1.VActCooMin_flow=")[0];
    let cooling_max = series("damValFan1.VActCooMax_flow=")[0];
    let minimum = series("damValFan1.VActMin_flow=")[0];
    let heating_min = series("damValFan1.VActHeaMin_flow=")[0];
    let heating_max = series("damValFan1.VActHeaMax_flow=")[0];
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(normal).unwrap().unwrap();
    let mut checked = 0;
    for index in 0..101 {
        let inputs = HashMap::from([
            ("cooling_signal".into(), json!(cooling[index])),
            ("heating_signal".into(), json!(heating[index])),
            ("cooling_min".into(), json!(cooling_min)),
            ("cooling_max".into(), json!(cooling_max)),
            ("minimum_flow".into(), json!(minimum)),
            ("heating_min".into(), json!(heating_min)),
            ("heating_max".into(), json!(heating_max)),
            ("supply_temp".into(), json!(supply[index] - 273.15)),
            ("zone_temp".into(), json!(zone_temp)),
            ("discharge_temp".into(), json!(discharge[index] - 273.15)),
            ("heating_warm_margin".into(), json!(3.0)),
        ]);
        let slow = dynamic.execute(normal, &inputs, 72.0, "").unwrap()["y"]
            .as_f64()
            .unwrap();
        let fast = compiled.execute(&inputs, 72.0, false).unwrap().0["y"]
            .as_f64()
            .unwrap();
        assert!(
            (slow - fast).abs() < 1e-10,
            "sample {index}: dynamic {slow}, compiled {fast}"
        );
        if code[index] == 0.0 {
            checked += 1;
            assert!(
                (slow - expected[index]).abs() < 1e-5,
                "sample {index}: {slow} vs {}",
                expected[index]
            );
        }
    }
    assert!(checked >= 15, "only {checked} unforced samples checked");
}

#[test]
fn g36_normal_vav_airflow_covers_deadband_and_heating_equations() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = building
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let normal = zone
        .blocks
        .iter()
        .find(|block| block.name == "normal_airflow_sequence")
        .unwrap();
    for (cool, heat, supply, discharge, expected) in [
        (0.0, 0.0, 14.0, 22.0, 0.15),
        (0.5, 0.0, 14.0, 22.0, 0.5),
        (0.5, 0.0, 25.0, 22.0, 0.15),
        (0.0, 0.8, 14.0, 22.0, 0.2),
        (0.0, 0.8, 14.0, 26.0, 0.62),
    ] {
        let inputs = HashMap::from([
            ("cooling_signal".into(), json!(cool)),
            ("heating_signal".into(), json!(heat)),
            ("cooling_min".into(), json!(0.1)),
            ("cooling_max".into(), json!(0.9)),
            ("minimum_flow".into(), json!(0.15)),
            ("heating_min".into(), json!(0.2)),
            ("heating_max".into(), json!(0.9)),
            ("supply_temp".into(), json!(supply)),
            ("zone_temp".into(), json!(22.0)),
            ("discharge_temp".into(), json!(discharge)),
            ("heating_warm_margin".into(), json!(3.0)),
        ]);
        let slow = Executor::default()
            .execute(normal, &inputs, 60.0, "")
            .unwrap()["y"]
            .as_f64()
            .unwrap();
        let fast = CompiledExecutor::try_new(normal)
            .unwrap()
            .unwrap()
            .execute(&inputs, 60.0, false)
            .unwrap()
            .0["y"]
            .as_f64()
            .unwrap();
        assert!(
            (slow - expected).abs() < 1e-10,
            "cool={cool}, heat={heat}, supply={supply}, discharge={discharge}: {slow}"
        );
        assert!(
            (fast - expected).abs() < 1e-10,
            "compiled cool={cool}, heat={heat}: {fast}"
        );
    }
}

#[test]
fn g36_discharge_setpoint_replays_pinned_damper_valves_trace() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = building
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let names = [
        "discharge_upper",
        "discharge_max_rise",
        "discharge_span",
        "discharge_positive_span",
        "discharge_zero_rise",
        "discharge_demand_double",
        "discharge_demand_limit",
        "discharge_reset_rise",
        "discharge_reset_target",
        "discharge_target_cap",
        "maximum_discharge_temp",
        "discharge_target",
        "discharge_minimum",
        "discharge_baseline",
        "discharge_heat_detected",
        "discharge_heat_hold",
    ];
    let mut branch = serde_json::to_value(zone).unwrap();
    branch["name"] = json!("PinnedDamperValvesDischargeTarget");
    branch["inputs"] = json!([
        {"name":"heating_signal","type":"Real","unit":"fraction (0–1)"},
        {"name":"heating_setpoint","type":"Real","unit":"°C"},
        {"name":"supply_air_setpoint","type":"Real","unit":"°C"}
    ]);
    branch["outputs"] = json!([{"name":"y","type":"Real","unit":"°C"}]);
    branch["blocks"]
        .as_array_mut()
        .unwrap()
        .retain(|block| names.contains(&block["name"].as_str().unwrap()));
    let original_edges = branch["connections"].as_array().unwrap().clone();
    let mut edges: Vec<Value> = original_edges
        .iter()
        .filter_map(|edge| {
            let target = edge["to"].as_str().unwrap().split('.').next().unwrap();
            if !names.contains(&target) {
                return None;
            }
            let source = match edge["from"].as_str().unwrap() {
                "heat_pi.y" => "heating_signal",
                "window_heat_target.y" => "heating_setpoint",
                other => other,
            };
            assert!(
                names.contains(&source.split('.').next().unwrap())
                    || ["heating_signal", "heating_setpoint", "supply_air_setpoint"]
                        .contains(&source),
                "unexpected discharge target source {source}"
            );
            Some(json!({"from":source,"to":edge["to"]}))
        })
        .collect();
    edges.push(json!({"from":"discharge_target.y","to":"y"}));
    branch["connections"] = json!(edges);
    let branch: Block = serde_json::from_value(branch).unwrap();
    assert!(validate(&branch).is_empty());
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_damper_valves_dymola.txt"
    ));
    let series = |name: &str| -> Vec<f64> {
        archive
            .lines()
            .find(|line| line.starts_with(name))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|field| field.trim().parse::<f64>().unwrap())
            .collect()
    };
    let heating = series("damValFan1.uHea=");
    let expected = series("damValFan1.TDisSet=");
    let supply = series("damValFan1.TSupSet=")[0] - 273.15;
    let heating_setpoint = series("damValFan1.THeaSet=")[0] - 273.15;
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    for index in 0..101 {
        let inputs = HashMap::from([
            ("heating_signal".into(), json!(heating[index])),
            ("heating_setpoint".into(), json!(heating_setpoint)),
            ("supply_air_setpoint".into(), json!(supply)),
        ]);
        let slow = dynamic.execute(&branch, &inputs, 72.0, "").unwrap()["y"]
            .as_f64()
            .unwrap();
        let fast = compiled.execute(&inputs, 72.0, false).unwrap().0["y"]
            .as_f64()
            .unwrap();
        let reference = expected[index] - 273.15;
        assert!(
            (slow - reference).abs() < 0.001,
            "sample {index}: dynamic {slow}, reference {reference}"
        );
        assert!(
            (fast - reference).abs() < 0.001,
            "sample {index}: compiled {fast}, reference {reference}"
        );
    }
}

#[test]
fn g36_reheat_pi_resets_when_cdl_enables_the_valve() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = building
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let pi = zone
        .blocks
        .iter()
        .find(|block| block.name == "discharge_valve_pi")
        .unwrap();
    assert_eq!(
        pi.block_type,
        "Buildings.Controls.OBC.CDL.Reals.PIDWithReset"
    );
    assert!(zone.connections.iter().any(|edge| {
        let (source, source_port) = edge.source();
        let (target, target_port) = edge.target();
        source == "discharge_valve_enable"
            && source_port == "y"
            && target == "discharge_valve_pi"
            && target_port == "trigger"
    }));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(pi).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("u_s".into(), json!(30.0)),
        ("u_m".into(), json!(15.0)),
        ("trigger".into(), json!(false)),
    ]);
    for _ in 0..30 {
        let slow = dynamic.execute(pi, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow["y"], fast["y"]);
    }
    inputs.insert("trigger".into(), json!(true));
    let slow = dynamic.execute(pi, &inputs, 60.0, "").unwrap();
    let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(slow["y"], json!(0.0));
    assert_eq!(fast["y"], json!(0.0));
    let slow = dynamic.execute(pi, &inputs, 60.0, "").unwrap();
    let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert!(slow["y"].as_f64().unwrap() > 0.0);
    assert_eq!(slow["y"], fast["y"]);
}

#[test]
fn g36_damper_pi_replays_pinned_damper_valves_trace() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = building
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let mut pi = zone
        .blocks
        .iter()
        .find(|block| block.name == "damper_flow_pi")
        .unwrap()
        .clone();
    for parameter in &mut pi.parameters {
        if parameter.name == "k" {
            parameter.value = json!(1.0);
        }
        if parameter.name == "r" {
            parameter.value = json!(1.0);
        }
    }
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_damper_valves_dymola.txt"
    ));
    let expected: Vec<f64> = archive
        .lines()
        .find(|line| line.starts_with("damValFan1.yDam="))
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .trim_matches(['[', ']'])
        .split(',')
        .map(|value| value.trim().parse().unwrap())
        .collect();
    assert_eq!(expected.len(), 101);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&pi).unwrap().unwrap();
    let mut largest = (0usize, 0.0_f64);
    // The validation's rounded ramp selects overrides 1, 2, and 3 at
    // 1334, 2000, and 2667 s. The 90%-width fan pulse starts at 180 s.
    for second in 0..=7200 {
        let t = second as f64;
        let setpoint = if second < 1334 {
            0.075
        } else if second < 2000 {
            0.0
        } else if second < 2667 {
            0.09
        } else {
            0.01
        };
        let fan = (180..4500).contains(&second) || second >= 4980;
        let inputs = HashMap::from([
            ("u_s".into(), json!(setpoint / 0.09)),
            ("u_m".into(), json!((0.01 + 0.06 * t / 7200.0) / 0.09)),
            ("trigger".into(), json!(fan)),
        ]);
        let slow = dynamic.execute(&pi, &inputs, 1.0, "").unwrap()["y"]
            .as_f64()
            .unwrap();
        let fast = compiled.execute(&inputs, 1.0, false).unwrap().0["y"]
            .as_f64()
            .unwrap();
        assert!(
            (slow - fast).abs() < 1e-9,
            "second {second}: {slow} vs {fast}"
        );
        if second % 72 == 0 {
            let index = second / 72;
            let excess = (slow - expected[index]).abs();
            if excess > largest.1 {
                largest = (index, excess);
            }
        }
    }
    assert!(
        largest.1 < 0.03,
        "damper excess {} at sample {}",
        largest.1,
        largest.0
    );
}

#[test]
fn g36_vav_damper_valves_branches_replay_all_pinned_outputs() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = building
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let mut branch = serde_json::to_value(zone).unwrap();
    branch["name"] = json!("PinnedDamperValvesAirflowBranch");
    branch["inputs"] = json!([
        {"name":"cooling_signal","type":"Real"},
        {"name":"heating_signal","type":"Real"},
        {"name":"supply_temp","type":"Real"},
        {"name":"zone_temp","type":"Real"},
        {"name":"discharge_temp","type":"Real"},
        {"name":"cooling_min","type":"Real"},
        {"name":"cooling_max","type":"Real"},
        {"name":"minimum_flow","type":"Real"},
        {"name":"heating_min","type":"Real"},
        {"name":"heating_max","type":"Real"},
        {"name":"heating_warm_margin","type":"Real"},
        {"name":"flow_override_code","type":"Integer"},
        {"name":"measured_airflow","type":"Real"},
        {"name":"supply_fan_proven","type":"Boolean"},
        {"name":"mode","type":"Integer"},
        {"name":"heating_setpoint","type":"Real","unit":"°C"},
        {"name":"supply_air_setpoint","type":"Real","unit":"°C"},
        {"name":"occupied","type":"Boolean"}
    ]);
    branch["outputs"] = json!([
        {"name":"airflow_request","type":"Real"},
        {"name":"damper_position","type":"Real"},
        {"name":"discharge_setpoint","type":"Real","unit":"°C"},
        {"name":"reheat_valve_position","type":"Real"}
    ]);
    let reheat_names = [
        "discharge_upper",
        "discharge_max_rise",
        "discharge_span",
        "discharge_positive_span",
        "discharge_zero_rise",
        "discharge_demand_double",
        "discharge_demand_limit",
        "discharge_reset_rise",
        "discharge_reset_target",
        "discharge_target_cap",
        "maximum_discharge_temp",
        "discharge_target",
        "discharge_minimum",
        "discharge_baseline",
        "discharge_heat_detected",
        "discharge_heat_hold",
        "discharge_valve_pi",
        "discharge_below_minimum",
        "discharge_minimum_occupied",
        "discharge_valve_enable",
        "discharge_valve_closed",
        "discharge_valve_select",
    ];
    branch["blocks"].as_array_mut().unwrap().retain(|block| {
        let name = block["name"].as_str().unwrap();
        name.starts_with("flow_override_")
            || reheat_names.contains(&name)
            || [
                "normal_airflow_sequence",
                "damper_flow_pi",
                "damper_closed",
                "damper_feedback_output",
                "damper_is_unoccupied",
                "damper_operation_enabled",
                "mode_unoccupied",
            ]
            .contains(&name)
    });
    for (name, value) in [
        ("flow_override_cooling_max", 0.09),
        ("flow_override_design_min", 0.01),
        ("flow_override_heating_max", 0.08),
    ] {
        branch["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == name)
            .unwrap()["parameters"][0]["value"] = json!(value);
    }
    let pi = branch["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "damper_flow_pi")
        .unwrap();
    for parameter in pi["parameters"].as_array_mut().unwrap() {
        if parameter["name"] == "k" {
            parameter["value"] = json!(1.0);
        }
        if parameter["name"] == "r" {
            parameter["value"] = json!(0.09);
        }
    }
    let valve_pi = branch["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "discharge_valve_pi")
        .unwrap();
    for parameter in valve_pi["parameters"].as_array_mut().unwrap() {
        if parameter["name"] == "k" {
            parameter["value"] = json!(0.5);
        }
        if parameter["name"] == "Ti" {
            parameter["value"] = json!(300.0);
        }
    }
    let names: std::collections::HashSet<String> = branch["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| block["name"].as_str().unwrap().to_string())
        .collect();
    let mut edges: Vec<Value> = branch["connections"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|edge| {
            let source = match edge["from"].as_str().unwrap() {
                "heat_pi.y" => "heating_signal",
                "window_heat_target.y" => "heating_setpoint",
                "local_occupied.y" => "occupied",
                other => other,
            };
            let target = edge["to"].as_str().unwrap();
            if target.starts_with("normal_airflow_sequence.")
                || !names.contains(target.split('.').next().unwrap())
                || !(names.contains(source.split('.').next().unwrap())
                    || [
                        "heating_signal",
                        "heating_setpoint",
                        "supply_air_setpoint",
                        "discharge_temp",
                        "occupied",
                    ]
                    .contains(&source))
            {
                return None;
            }
            Some(json!({"from":source,"to":target}))
        })
        .collect();
    edges.extend([
        json!({"from":"normal_airflow_sequence.y","to":"flow_override_select.u2"}),
        json!({"from":"flow_override_code","to":"flow_override_is_zero.u1"}),
        json!({"from":"flow_override_code","to":"flow_override_is_cooling.u1"}),
        json!({"from":"flow_override_code","to":"flow_override_is_minimum.u1"}),
        json!({"from":"flow_override_code","to":"flow_override_is_heating.u1"}),
        json!({"from":"measured_airflow","to":"damper_flow_pi.u_m"}),
        json!({"from":"supply_fan_proven","to":"damper_flow_pi.trigger"}),
        json!({"from":"mode","to":"damper_is_unoccupied.u1"}),
        json!({"from":"flow_override_select.y","to":"airflow_request"}),
        json!({"from":"damper_feedback_output.y","to":"damper_position"}),
        json!({"from":"discharge_target.y","to":"discharge_setpoint"}),
        json!({"from":"discharge_valve_select.y","to":"reheat_valve_position"}),
    ]);
    for input in [
        "cooling_signal",
        "heating_signal",
        "supply_temp",
        "zone_temp",
        "discharge_temp",
        "cooling_min",
        "cooling_max",
        "minimum_flow",
        "heating_min",
        "heating_max",
        "heating_warm_margin",
    ] {
        edges.push(json!({"from":input,"to":format!("normal_airflow_sequence.{input}")}));
    }
    branch["connections"] = json!(edges);
    let branch: Block = serde_json::from_value(branch).unwrap();
    assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_damper_valves_dymola.txt"
    ));
    let series = |name: &str| -> Vec<f64> {
        archive
            .lines()
            .find(|line| line.starts_with(name))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|field| field.trim().parse().unwrap())
            .collect()
    };
    let expected_flow = series("damValFan1.VSet_flow=");
    let expected_damper = series("damValFan1.yDam=");
    let expected_discharge = series("damValFan1.TDisSet=");
    let expected_valve = series("damValFan1.yVal=");
    assert_eq!(
        (
            expected_flow.len(),
            expected_damper.len(),
            expected_discharge.len(),
            expected_valve.len()
        ),
        (101, 101, 101, 101)
    );
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    let mut largest_flow = (0usize, 0.0_f64);
    let mut largest_damper = (0usize, 0.0_f64);
    let mut largest_discharge = (0usize, 0.0_f64);
    let mut largest_valve = (0usize, 0.0_f64);
    for second in 0..=7200 {
        let t = second as f64;
        let inputs = HashMap::from([
            (
                "cooling_signal".into(),
                json!((1.0 - (t - 900.0) / 3600.0).clamp(0.0, 1.0)),
            ),
            (
                "heating_signal".into(),
                json!(((t - 5500.0) / 3600.0).clamp(0.0, 1.0)),
            ),
            (
                "supply_temp".into(),
                json!(13.0 + (std::f64::consts::TAU * t / 3600.0).sin()),
            ),
            ("zone_temp".into(), json!(22.0)),
            (
                "discharge_temp".into(),
                json!(13.0 + 1.2 * (std::f64::consts::TAU * t / 3600.0).sin()),
            ),
            ("cooling_min".into(), json!(0.015)),
            ("cooling_max".into(), json!(0.075)),
            ("minimum_flow".into(), json!(0.01)),
            ("heating_min".into(), json!(0.015)),
            ("heating_max".into(), json!(0.06)),
            ("heating_warm_margin".into(), json!(3.0)),
            (
                "flow_override_code".into(),
                json!((((t - 1000.0) * 3.0 / 2000.0).clamp(0.0, 3.0).round()) as i64),
            ),
            ("measured_airflow".into(), json!(0.01 + 0.06 * t / 7200.0)),
            (
                "supply_fan_proven".into(),
                json!((180..4500).contains(&second) || second >= 4980),
            ),
            ("mode".into(), json!(1)),
            ("heating_setpoint".into(), json!(20.0)),
            ("supply_air_setpoint".into(), json!(13.0)),
            ("occupied".into(), json!(true)),
        ]);
        let slow = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
        let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
        for output in [
            "airflow_request",
            "damper_position",
            "discharge_setpoint",
            "reheat_valve_position",
        ] {
            assert!(
                (slow[output].as_f64().unwrap() - fast[output].as_f64().unwrap()).abs() < 1e-9,
                "second {second} output {output}"
            );
        }
        if second % 72 == 0 {
            let index = second / 72;
            let flow_excess =
                (slow["airflow_request"].as_f64().unwrap() - expected_flow[index]).abs();
            let damper_excess =
                (slow["damper_position"].as_f64().unwrap() - expected_damper[index]).abs();
            let discharge_excess = (slow["discharge_setpoint"].as_f64().unwrap()
                - (expected_discharge[index] - 273.15))
                .abs();
            let valve_excess =
                (slow["reheat_valve_position"].as_f64().unwrap() - expected_valve[index]).abs();
            if flow_excess > largest_flow.1 {
                largest_flow = (index, flow_excess);
            }
            if damper_excess > largest_damper.1 {
                largest_damper = (index, damper_excess);
            }
            if discharge_excess > largest_discharge.1 {
                largest_discharge = (index, discharge_excess);
            }
            if valve_excess > largest_valve.1 {
                largest_valve = (index, valve_excess);
            }
        }
    }
    assert!(
        largest_flow.1 < 1e-5,
        "flow excess {} at sample {}",
        largest_flow.1,
        largest_flow.0
    );
    assert!(
        largest_damper.1 < 0.003,
        "damper excess {} at sample {}",
        largest_damper.1,
        largest_damper.0
    );
    assert!(
        largest_discharge.1 < 0.00002,
        "discharge target excess {} at sample {}",
        largest_discharge.1,
        largest_discharge.0
    );
    assert!(
        largest_valve.1 < 0.008,
        "reheat valve excess {} at sample {}",
        largest_valve.1,
        largest_valve.0
    );
}

#[test]
fn g36_reheat_valve_core_replays_pinned_damper_valves_trace() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = building
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let names = [
        "discharge_upper",
        "discharge_max_rise",
        "discharge_span",
        "discharge_positive_span",
        "discharge_zero_rise",
        "discharge_demand_double",
        "discharge_demand_limit",
        "discharge_reset_rise",
        "discharge_reset_target",
        "discharge_target_cap",
        "maximum_discharge_temp",
        "discharge_target",
        "discharge_minimum",
        "discharge_baseline",
        "discharge_heat_detected",
        "discharge_heat_hold",
        "discharge_valve_pi",
        "discharge_below_minimum",
        "discharge_minimum_occupied",
        "discharge_valve_enable",
        "discharge_valve_closed",
        "discharge_valve_select",
    ];
    let mut branch = serde_json::to_value(zone).unwrap();
    branch["name"] = json!("PinnedDamperValvesReheatCore");
    branch["inputs"] = json!([
        {"name":"heating_signal","type":"Real","unit":"fraction (0–1)"},
        {"name":"heating_setpoint","type":"Real","unit":"°C"},
        {"name":"supply_air_setpoint","type":"Real","unit":"°C"},
        {"name":"discharge_temp","type":"Real","unit":"°C"},
        {"name":"occupied","type":"Boolean"}
    ]);
    branch["outputs"] = json!([{"name":"y","type":"Real","unit":"fraction (0–1)"}]);
    branch["blocks"]
        .as_array_mut()
        .unwrap()
        .retain(|block| names.contains(&block["name"].as_str().unwrap()));
    let pi = branch["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "discharge_valve_pi")
        .unwrap();
    for parameter in pi["parameters"].as_array_mut().unwrap() {
        if parameter["name"] == "k" {
            parameter["value"] = json!(0.5);
        }
        if parameter["name"] == "Ti" {
            parameter["value"] = json!(300.0);
        }
    }
    let original = branch["connections"].as_array().unwrap().clone();
    let mut edges: Vec<Value> = original
        .iter()
        .filter_map(|edge| {
            let target = edge["to"].as_str().unwrap().split('.').next().unwrap();
            if !names.contains(&target) {
                return None;
            }
            let source = match edge["from"].as_str().unwrap() {
                "heat_pi.y" => "heating_signal",
                "window_heat_target.y" => "heating_setpoint",
                "local_occupied.y" => "occupied",
                other => other,
            };
            assert!(
                names.contains(&source.split('.').next().unwrap())
                    || [
                        "heating_signal",
                        "heating_setpoint",
                        "supply_air_setpoint",
                        "discharge_temp",
                        "occupied"
                    ]
                    .contains(&source),
                "unexpected reheat source {source}"
            );
            Some(json!({"from":source,"to":edge["to"]}))
        })
        .collect();
    edges.push(json!({"from":"discharge_valve_select.y","to":"y"}));
    branch["connections"] = json!(edges);
    let branch: Block = serde_json::from_value(branch).unwrap();
    assert!(validate(&branch).is_empty());
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_damper_valves_dymola.txt"
    ));
    let expected: Vec<f64> = archive
        .lines()
        .find(|line| line.starts_with("damValFan1.yVal="))
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .trim_matches(['[', ']'])
        .split(',')
        .map(|field| field.trim().parse().unwrap())
        .collect();
    assert_eq!(expected.len(), 101);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    let mut largest = (0usize, 0.0_f64);
    for second in 0..=7200 {
        let t = second as f64;
        let inputs = HashMap::from([
            (
                "heating_signal".into(),
                json!(((t - 5500.0) / 3600.0).clamp(0.0, 1.0)),
            ),
            ("heating_setpoint".into(), json!(20.0)),
            ("supply_air_setpoint".into(), json!(13.0)),
            (
                "discharge_temp".into(),
                json!(13.0 + 1.2 * (std::f64::consts::TAU * t / 3600.0).sin()),
            ),
            ("occupied".into(), json!(true)),
        ]);
        let slow = dynamic.execute(&branch, &inputs, 1.0, "").unwrap()["y"]
            .as_f64()
            .unwrap();
        let fast = compiled.execute(&inputs, 1.0, false).unwrap().0["y"]
            .as_f64()
            .unwrap();
        assert!(
            (slow - fast).abs() < 1e-9,
            "second {second}: {slow} vs {fast}"
        );
        if second % 72 == 0 {
            let index = second / 72;
            let excess = (slow - expected[index]).abs();
            if excess > largest.1 {
                largest = (index, excess);
            }
        }
    }
    assert!(
        largest.1 < 0.03,
        "largest valve trace excess {} at sample {}",
        largest.1,
        largest.0
    );
}

#[test]
fn conventional_source_fault_status_comes_from_cdl() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let inputs: HashMap<String, Value> = building
        .inputs
        .iter()
        .map(|port| {
            (
                port.name.clone(),
                if port.kind == "Boolean" {
                    json!(false)
                } else {
                    json!(0.0)
                },
            )
        })
        .collect();
    let mut dynamic = Executor::default();
    let normal = dynamic.execute(&building, &inputs, 900.0, "").unwrap();
    assert_eq!(normal["plant_heat_pump_start_fault"], json!(false));
    let mut edited = building.clone();
    let plant = edited
        .blocks
        .iter_mut()
        .find(|block| block.name == "plant")
        .unwrap();
    plant
        .blocks
        .iter_mut()
        .find(|block| block.name == "conventional_heat_pump_start_fault_off")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "k")
        .unwrap()
        .value = json!(true);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&edited).unwrap().unwrap();
    let slow = dynamic.execute(&edited, &inputs, 900.0, "").unwrap();
    let fast = compiled.execute(&inputs, 900.0, false).unwrap().0;
    assert_eq!(slow["plant_heat_pump_start_fault"], json!(true));
    assert_eq!(fast["plant_heat_pump_start_fault"], json!(true));
}

#[test]
fn integer_requests_match_in_compiled_and_dynamic_execution() {
    let graph: Block = serde_json::from_value(json!({
        "name":"requests","block_type":"Composite","category":"composite",
        "inputs":[{"name":"a","type":"Boolean"},{"name":"b","type":"Boolean"}],
        "outputs":[{"name":"count","type":"Integer"}],
        "blocks":[
            {"name":"a_int","block_type":"Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger","inputs":[{"name":"u","type":"Boolean"}],"outputs":[{"name":"y","type":"Integer"}]},
            {"name":"b_int","block_type":"Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger","inputs":[{"name":"u","type":"Boolean"}],"outputs":[{"name":"y","type":"Integer"}]},
            {"name":"sum","block_type":"Buildings.Controls.OBC.CDL.Integers.Add","inputs":[{"name":"u1","type":"Integer"},{"name":"u2","type":"Integer"}],"outputs":[{"name":"y","type":"Integer"}]}
        ],
        "connections":[{"from":"a","to":"a_int.u"},{"from":"b","to":"b_int.u"},{"from":"a_int.y","to":"sum.u1"},{"from":"b_int.y","to":"sum.u2"},{"from":"sum.y","to":"count"}]
    })).unwrap();
    assert!(validate(&graph).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (a, b, expected) in [(false, false, 0), (true, false, 1), (true, true, 2)] {
        let inputs = HashMap::from([("a".into(), json!(a)), ("b".into(), json!(b))]);
        let reference = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
        let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(reference["count"], json!(expected));
        assert_eq!(optimized["count"], json!(expected));
    }
}

#[test]
fn obc_integer_conversions_and_comparisons_match_both_executors() {
    let graph: Block = serde_json::from_value(json!({
        "name":"integer_reference","block_type":"Composite","category":"composite",
        "inputs":[{"name":"condition","type":"Boolean"},{"name":"real","type":"Real"},{"name":"match_value","type":"Integer"}],
        "outputs":[{"name":"converted","type":"Integer"},{"name":"rounded","type":"Integer"},{"name":"above","type":"Boolean"},{"name":"equal","type":"Boolean"}],
        "blocks":[
            {"name":"boolean_to_integer","block_type":"Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger","inputs":[{"name":"u","type":"Boolean"}],"outputs":[{"name":"y","type":"Integer"}],"parameters":[{"name":"integerTrue","value":7},{"name":"integerFalse","value":-3}]},
            {"name":"real_to_integer","block_type":"Buildings.Controls.OBC.CDL.Conversions.RealToInteger","inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Integer"}]},
            {"name":"greater_threshold","block_type":"Buildings.Controls.OBC.CDL.Integers.GreaterThreshold","inputs":[{"name":"u","type":"Integer"}],"outputs":[{"name":"y","type":"Boolean"}],"parameters":[{"name":"t","value":2}]},
            {"name":"integer_equal","block_type":"Buildings.Controls.OBC.CDL.Integers.Equal","inputs":[{"name":"u1","type":"Integer"},{"name":"u2","type":"Integer"}],"outputs":[{"name":"y","type":"Boolean"}]}
        ],
        "connections":[
            {"from":"condition","to":"boolean_to_integer.u"},
            {"from":"real","to":"real_to_integer.u"},
            {"from":"real_to_integer.y","to":"greater_threshold.u"},
            {"from":"real_to_integer.y","to":"integer_equal.u1"},
            {"from":"match_value","to":"integer_equal.u2"},
            {"from":"boolean_to_integer.y","to":"converted"},
            {"from":"real_to_integer.y","to":"rounded"},
            {"from":"greater_threshold.y","to":"above"},
            {"from":"integer_equal.y","to":"equal"}
        ]
    })).unwrap();
    assert!(validate(&graph).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (condition, real, match_value, expected_integer, above, equal) in [
        (false, -2.5, -3, -3, false, true),
        (true, -0.5, 0, -1, false, false),
        (false, 2.5, 3, 3, true, true),
        (true, 1.5, 2, 2, false, true),
    ] {
        let inputs = HashMap::from([
            ("condition".into(), json!(condition)),
            ("real".into(), json!(real)),
            ("match_value".into(), json!(match_value)),
        ]);
        let reference = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
        let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for actual in [&reference, &optimized] {
            assert_eq!(actual["converted"], json!(if condition { 7 } else { -3 }));
            assert_eq!(actual["rounded"], json!(expected_integer));
            assert_eq!(actual["above"], json!(above));
            assert_eq!(actual["equal"], json!(equal));
        }
    }
}

#[test]
fn boolean_pre_converges_feedback_within_event_in_both_executors() {
    let graph: Block = serde_json::from_value(json!({
        "name":"previous_boolean", "block_type":"composite",
        "inputs":[{"name":"request","type":"Boolean"}],
        "outputs":[{"name":"latched","type":"Boolean"},{"name":"previous","type":"Boolean"}],
        "blocks":[
            {"name":"pre","block_type":"Buildings.Controls.OBC.CDL.Logical.Pre",
             "inputs":[{"name":"u","type":"Boolean"}],
             "outputs":[{"name":"y","type":"Boolean"}],
             "parameters":[{"name":"pre_u_start","type":"Boolean","value":false}]},
            {"name":"feedback","block_type":"Buildings.Controls.OBC.CDL.Logical.Or",
             "inputs":[{"name":"u1","type":"Boolean"},{"name":"u2","type":"Boolean"}],
             "outputs":[{"name":"y","type":"Boolean"}]}
        ],
        "connections":[
            {"from":"request","to":"feedback.u1"},
            {"from":"pre.y","to":"feedback.u2"},
            {"from":"feedback.y","to":"pre.u"},
            {"from":"feedback.y","to":"latched"},
            {"from":"pre.y","to":"previous"}
        ]
    }))
    .unwrap();
    assert!(validate(&graph).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (request, expected_latched, expected_previous) in [
        (false, false, false),
        (true, true, true),
        (false, true, true),
        (false, true, true),
    ] {
        let inputs = HashMap::from([("request".into(), json!(request))]);
        let ordinary = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for result in [&ordinary, &fast] {
            assert_eq!(result["latched"], json!(expected_latched));
            assert_eq!(result["previous"], json!(expected_previous));
        }
    }
}

#[test]
fn boolean_pre_rejects_oscillating_event_loop() {
    let graph: Block = serde_json::from_value(json!({
        "name":"oscillating_boolean", "block_type":"composite",
        "inputs":[],
        "outputs":[{"name":"y","type":"Boolean"}],
        "blocks":[
            {"name":"pre","block_type":"Buildings.Controls.OBC.CDL.Logical.Pre",
             "inputs":[{"name":"u","type":"Boolean"}],
             "outputs":[{"name":"y","type":"Boolean"}]},
            {"name":"invert","block_type":"Buildings.Controls.OBC.CDL.Logical.Not",
             "inputs":[{"name":"u","type":"Boolean"}],
             "outputs":[{"name":"y","type":"Boolean"}]}
        ],
        "connections":[
            {"from":"pre.y","to":"invert.u"},
            {"from":"invert.y","to":"pre.u"},
            {"from":"invert.y","to":"y"}
        ]
    }))
    .unwrap();
    assert!(validate(&graph).is_empty());
    assert!(Executor::default()
        .execute(&graph, &HashMap::new(), 60.0, "")
        .unwrap_err()
        .contains("did not converge"));
    assert!(CompiledExecutor::try_new(&graph)
        .unwrap()
        .unwrap()
        .execute(&HashMap::new(), 60.0, false)
        .unwrap_err()
        .contains("did not converge"));
}

#[test]
fn real_greater_threshold_uses_obc_hysteresis_in_both_executors() {
    let graph: Block = serde_json::from_value(json!({
        "name":"hysteretic_threshold",
        "block_type":"Buildings.Controls.OBC.CDL.Reals.GreaterThreshold",
        "inputs":[{"name":"u","type":"Real"}],
        "outputs":[{"name":"y","type":"Boolean"}],
        "parameters":[{"name":"t","value":10.0},{"name":"h","value":2.0}]
    }))
    .unwrap();
    assert!(validate(&graph).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (value, expected) in [
        (9.0, false),
        (11.0, true),
        (9.0, true),
        (8.0, false),
        (11.0, true),
        (8.0, false),
    ] {
        let inputs = HashMap::from([("u".into(), json!(value))]);
        assert_eq!(
            dynamic.execute(&graph, &inputs, 60.0, "").unwrap()["y"],
            json!(expected)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["y"],
            json!(expected)
        );
    }
    let mut invalid = graph.clone();
    invalid
        .parameters
        .iter_mut()
        .find(|p| p.name == "h")
        .unwrap()
        .value = json!(-1.0);
    assert!(validate(&invalid)
        .iter()
        .any(|issue| issue.contains("nonnegative")));
}
