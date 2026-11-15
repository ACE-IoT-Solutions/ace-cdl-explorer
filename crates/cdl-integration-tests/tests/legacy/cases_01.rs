use super::*;

#[test]
fn real_less_threshold_uses_obc_hysteresis_in_both_executors() {
    let graph: Block = serde_json::from_value(json!({
        "name":"low_discharge_threshold",
        "block_type":"Buildings.Controls.OBC.CDL.Reals.LessThreshold",
        "inputs":[{"name":"u","type":"Real"}],
        "outputs":[{"name":"y","type":"Boolean"}],
        "parameters":[{"name":"t","value":10.0},{"name":"h","value":0.25}]
    }))
    .unwrap();
    assert!(validate(&graph).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (value, expected) in [
        (10.1, false),
        (9.9, true),
        (10.1, true),
        (10.24, true),
        (10.25, false),
        (9.9, true),
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
        .find(|parameter| parameter.name == "h")
        .unwrap()
        .value = json!(-0.1);
    assert!(validate(&invalid)
        .iter()
        .any(|error| error.contains("hysteresis")));
}

#[test]
fn real_pair_comparators_use_obc_hysteresis_in_both_executors() {
    let graph: Block = serde_json::from_value(json!({
        "name":"pair_comparators", "block_type":"Composite", "category":"composite",
        "inputs":[{"name":"a","type":"Real"},{"name":"b","type":"Real"}],
        "outputs":[{"name":"greater","type":"Boolean"},{"name":"less","type":"Boolean"}],
        "blocks":[
            {"name":"g","block_type":"Buildings.Controls.OBC.CDL.Reals.Greater",
             "inputs":[{"name":"u1","type":"Real"},{"name":"u2","type":"Real"}],
             "outputs":[{"name":"y","type":"Boolean"}],
             "parameters":[{"name":"h","value":0.2},{"name":"pre_y_start","type":"Boolean","value":false}]},
            {"name":"l","block_type":"Buildings.Controls.OBC.CDL.Reals.Less",
             "inputs":[{"name":"u1","type":"Real"},{"name":"u2","type":"Real"}],
             "outputs":[{"name":"y","type":"Boolean"}],
             "parameters":[{"name":"h","value":0.2},{"name":"pre_y_start","type":"Boolean","value":false}]}
        ],
        "connections":[
            {"from":"a","to":"g.u1"},{"from":"b","to":"g.u2"},
            {"from":"a","to":"l.u1"},{"from":"b","to":"l.u2"},
            {"from":"g.y","to":"greater"},{"from":"l.y","to":"less"}
        ]
    })).unwrap();
    assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (a, greater, less) in [
        (1.0, false, false),
        (1.1, true, false),
        (0.9, true, true),
        (0.75, false, true),
        (1.15, true, true),
        (1.25, true, false),
    ] {
        let inputs = HashMap::from([("a".into(), json!(a)), ("b".into(), json!(1.0))]);
        for result in [
            dynamic.execute(&graph, &inputs, 60.0, "").unwrap(),
            compiled.execute(&inputs, 60.0, false).unwrap().0,
        ] {
            assert_eq!(result["greater"], json!(greater), "a={a}");
            assert_eq!(result["less"], json!(less), "a={a}");
        }
    }
    let mut initially_true = graph.clone();
    for block in &mut initially_true.blocks {
        block
            .parameters
            .iter_mut()
            .find(|p| p.name == "pre_y_start")
            .unwrap()
            .value = json!(true);
    }
    let inputs = HashMap::from([("a".into(), json!(1.1)), ("b".into(), json!(1.0))]);
    for result in [
        Executor::default()
            .execute(&initially_true, &inputs, 60.0, "")
            .unwrap(),
        CompiledExecutor::try_new(&initially_true)
            .unwrap()
            .unwrap()
            .execute(&inputs, 60.0, false)
            .unwrap()
            .0,
    ] {
        assert_eq!(result["greater"], json!(true));
        assert_eq!(result["less"], json!(true));
    }
    for block in &mut initially_true.blocks {
        block
            .parameters
            .iter_mut()
            .find(|p| p.name == "h")
            .unwrap()
            .value = json!(-0.1);
    }
    assert!(validate(&initially_true)
        .iter()
        .any(|issue| issue.contains("nonnegative")));
}

#[test]
fn pinned_optimal_start_calculation_executes_in_both_engines() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/obc_optimal_start_calculation.json"
    )))
    .unwrap();
    assert!(validate(&graph).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (error, start_calculation, next_occupancy) in [
        (2.0, false, 20_000.0),
        (2.0, true, 10_800.0),
        (2.0, false, 6_000.0),
        (0.0, false, 1_000.0),
    ] {
        let inputs = HashMap::from([
            ("TDif".into(), json!(error)),
            ("staCal".into(), json!(start_calculation)),
            ("tNexOcc".into(), json!(next_occupancy)),
        ]);
        let ordinary = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(ordinary["optOn"], fast["optOn"]);
        let a = ordinary["tOpt"].as_f64().unwrap();
        let b = fast["tOpt"].as_f64().unwrap();
        assert!(a.is_finite() && (0.0..=10_800.0).contains(&a));
        assert!((a - b).abs() < 1e-9);
    }
}

#[test]
fn pinned_optimal_start_learns_a_completed_recovery_across_days() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/obc_optimal_start_calculation.json"
    )))
    .unwrap();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for minute in 0..=1440 {
        let seconds = minute * 60;
        let day_second = seconds % 86_400;
        // Three kelvin of error clears after 1,740 seconds of measured
        // recovery. OBC samples its timer at the falling edge of the
        // recovery latch, then samples that slope at the next midnight.
        let error = if day_second == 0 {
            3.0
        } else if day_second <= 1_860 {
            3.0 - 3.2 * (day_second - 60) as f64 / 1_800.0
        } else {
            -0.2
        };
        let next_occupancy = if day_second < 10_800 {
            10_800 - day_second
        } else {
            97_200 - day_second
        };
        let inputs = HashMap::from([
            ("TDif".into(), json!(error)),
            ("staCal".into(), json!(day_second == 0)),
            ("tNexOcc".into(), json!(next_occupancy)),
        ]);
        let ordinary = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(ordinary["optOn"], fast["optOn"], "minute {minute}");
        assert_eq!(ordinary["tOpt"], fast["tOpt"], "minute {minute}");
        if minute == 0 {
            assert_eq!(ordinary["tOpt"], json!(10_800.0));
        }
        if minute == 1440 {
            // First midnight samples an initial zero slope. The next
            // samples 3 K / 1740 s, so the two-sample mean is 3/3480 K/s.
            // A fresh 3 K error therefore predicts 3 / (3/3480) s.
            assert_eq!(ordinary["tOpt"], json!(3_480.0));
        }
    }
}

#[test]
#[ignore = "Manual cost check for nested OBC optimal-start event iteration"]
fn benchmark_obc_optimal_start_group() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("seconds_until_occupied".into(), json!(12_000.0)),
        ("lookahead_allowed".into(), json!(true)),
        ("scheduled_occupied".into(), json!(false)),
    ]);
    for zone in 1..=5 {
        inputs.insert(format!("room_{zone}"), json!(19.0));
        inputs.insert(format!("heat_target_{zone}"), json!(21.0));
        inputs.insert(format!("cool_target_{zone}"), json!(24.0));
        inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
        inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
        inputs.insert(format!("window_closed_{zone}"), json!(true));
        inputs.insert(format!("override_{zone}"), json!(false));
    }
    let start = std::time::Instant::now();
    for index in 0..1_000 {
        inputs.insert(
            "seconds_until_occupied".into(),
            json!(12_000.0 - index as f64 * 60.0),
        );
        compiled.execute(&inputs, 60.0, false).unwrap();
    }
    println!("1000 group steps: {:?}", start.elapsed());
}

#[test]
fn pinned_optimal_start_drives_g36_group_from_editable_cdl() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let mut inputs = HashMap::from([
        ("seconds_until_occupied".into(), json!(24_000.0)),
        ("lookahead_allowed".into(), json!(true)),
        ("scheduled_occupied".into(), json!(false)),
    ]);
    for zone in 1..=5 {
        inputs.insert(
            format!("room_{zone}"),
            json!(if zone == 1 { 19.0 } else { 21.0 }),
        );
        inputs.insert(format!("heat_target_{zone}"), json!(21.0));
        inputs.insert(format!("cool_target_{zone}"), json!(24.0));
        inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
        inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
        inputs.insert(format!("window_closed_{zone}"), json!(true));
        inputs.insert(format!("override_{zone}"), json!(false));
    }
    let mut dynamic = Executor::default();
    dynamic.trace_targets = vec![
        "zone_group_1|optimal_warmup_1_obc.tOpt".into(),
        "zone_group_1|optimal_start_sample_event.y".into(),
    ];
    let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
    let mut warmup_seen = false;
    for minute in 0..=340 {
        inputs.insert(
            "seconds_until_occupied".into(),
            json!(24_000.0 - minute as f64 * 60.0),
        );
        let ordinary = dynamic.execute(group, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(ordinary["warmup"], fast["warmup"]);
        assert_eq!(ordinary["preconditioning"], fast["preconditioning"]);
        if minute < 220 {
            assert_eq!(ordinary["warmup"], false);
        }
        if minute == 220 {
            assert_eq!(
                dynamic.trace_values["zone_group_1|optimal_start_sample_event.y"],
                true
            );
            assert_eq!(
                dynamic.trace_values["zone_group_1|optimal_warmup_1_obc.tOpt"],
                7200.0
            );
        }
        warmup_seen |= ordinary["warmup"] == true;
    }
    assert!(warmup_seen, "the sampled OBC prediction must start warm-up");
    inputs.insert("window_closed_1".into(), json!(false));
    let closed = dynamic.execute(group, &inputs, 60.0, "").unwrap();
    assert_eq!(closed["warmup"], false, "open-window zone must be excluded");
    assert_eq!(
        closed["warmup"],
        compiled.execute(&inputs, 60.0, false).unwrap().0["warmup"]
    );
    let mut tuned = group.clone();
    let learner = tuned
        .blocks
        .iter_mut()
        .find(|block| block.name == "optimal_warmup_1_obc")
        .unwrap();
    learner
        .blocks
        .iter_mut()
        .find(|block| block.name == "defTemSlo")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "k")
        .unwrap()
        .value = json!(1.0 / 600.0);
    inputs.insert("window_closed_1".into(), json!(true));
    let mut faster = CompiledExecutor::try_new(&tuned).unwrap().unwrap();
    let mut result = HashMap::new();
    for minute in 0..=340 {
        inputs.insert(
            "seconds_until_occupied".into(),
            json!(24_000.0 - minute as f64 * 60.0),
        );
        result = faster.execute(&inputs, 60.0, false).unwrap().0;
    }
    assert_eq!(
        result["warmup"], false,
        "editing the OBC slope floor must delay warm-up"
    );
}

#[test]
fn hysteresis_rises_above_high_and_clears_below_low_in_both_executors() {
    let graph: Block = serde_json::from_value(json!({
        "name":"valve_request", "block_type":"Buildings.Controls.OBC.CDL.Reals.Hysteresis",
        "inputs":[{"name":"u","type":"Real"}],
        "outputs":[{"name":"y","type":"Boolean"}],
        "parameters":[{"name":"uLow","value":0.4},{"name":"uHigh","value":0.8}]
    }))
    .unwrap();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (value, expected) in [
        (0.0, false),
        (0.9, true),
        (0.7, true),
        (0.2, false),
        (0.7, false),
        (0.9, true),
        (0.3, false),
    ] {
        let inputs = HashMap::from([("u".into(), json!(value))]);
        let reference = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
        let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(reference["y"], json!(expected));
        assert_eq!(optimized["y"], json!(expected));
    }
    let mut initialized = graph.clone();
    initialized.parameters.push(Parameter {
        name: "pre_y_start".into(),
        value: json!(true),
    });
    let input = HashMap::from([("u".into(), json!(0.6))]);
    assert_eq!(
        Executor::default()
            .execute(&initialized, &input, 60.0, "")
            .unwrap()["y"],
        json!(true)
    );
    assert_eq!(
        CompiledExecutor::try_new(&initialized)
            .unwrap()
            .unwrap()
            .execute(&input, 60.0, false)
            .unwrap()
            .0["y"],
        json!(true)
    );
}

#[test]
fn unit_delay_breaks_feedback_cycle_and_samples_at_its_period() {
    let graph: Block=serde_json::from_value(json!({
        "name":"sampled_feedback","block_type":"Composite","category":"composite",
        "inputs":[{"name":"increment","type":"Real"}],"outputs":[{"name":"result","type":"Real"}],
        "blocks":[
            {"name":"previous","block_type":"Buildings.Controls.OBC.CDL.Discrete.UnitDelay",
             "inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],
             "parameters":[{"name":"samplePeriod","value":120.0},{"name":"y_start","value":1.0}]},
            {"name":"next","block_type":"Add","inputs":[{"name":"u1","type":"Real"},{"name":"u2","type":"Real"}],"outputs":[{"name":"y","type":"Real"}]}
        ],
        "connections":[{"from":"previous.y","to":"next.u1"},{"from":"increment","to":"next.u2"},
            {"from":"next.y","to":"previous.u"},{"from":"next.y","to":"result"}]
    })).unwrap();
    assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
    let inputs = HashMap::from([("increment".into(), json!(1.0))]);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    // Calls occur at t=0, 60, 120, 180, and 240 seconds.
    for expected in [2.0, 2.0, 3.0, 3.0, 4.0] {
        let reference = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
        let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(reference["result"], json!(expected));
        assert_eq!(optimized["result"], json!(expected));
    }
}

#[test]
fn g36_ahu_freeze_thresholds_hold_through_pinned_hysteresis_band() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let ahu = building
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1")
        .unwrap();
    let cold = "ahu_1|freeze_stage_one_cold.y".to_string();
    let warm = "ahu_1|freeze_stage_one_warm.y".to_string();
    let mut dynamic = Executor::default();
    dynamic.trace_targets = vec![cold.clone(), warm.clone()];
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    compiled.set_traces(&[cold.clone(), warm.clone()]).unwrap();
    let mut inputs: HashMap<String, Value> = ahu
        .inputs
        .iter()
        .map(|port| {
            let value = match port.kind.as_str() {
                "Boolean" => json!(false),
                "Integer" => json!(0),
                _ => json!(0.0),
            };
            (port.name.clone(), value)
        })
        .collect();
    inputs.insert("occupied_mode".into(), json!(true));
    inputs.insert("supply_fan_proven".into(), json!(true));
    inputs.insert("operation_mode".into(), json!(1));
    for (temperature, expect_cold, expect_warm) in [
        (5.0, false, false),
        (4.3, true, false),
        (4.5, true, false),
        (4.65, false, false),
        (7.1, false, true),
        (6.8, false, true),
        (6.75, false, false),
    ] {
        inputs.insert("actual_supply_air_temp".into(), json!(temperature));
        let slow = dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
        let (fast, traces) = compiled.execute(&inputs, 15.0, true).unwrap();
        assert_eq!(slow, fast);
        for (name, expected) in [(&cold, expect_cold), (&warm, expect_warm)] {
            assert_eq!(
                dynamic.trace_values[name],
                json!(expected),
                "{name} at {temperature}"
            );
            assert_eq!(
                traces[name],
                json!(if expected { 1.0 } else { 0.0 }),
                "compiled {name} at {temperature}"
            );
        }
    }
}

#[test]
fn g36_supply_air_mode_selection_matches_pinned_obc_priority() {
    let building: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let ahu = building["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["name"] == "ahu_1")
        .unwrap();
    let mut branch = json!({
        "name":"SupplyModeReference","block_type":"Composite","category":"composite",
        "inputs":[{"name":"operation_mode","type":"Integer"},
                  {"name":"supply_fan_proven","type":"Boolean"},
                  {"name":"reset_max","type":"Real"},
                  {"name":"outdoor_c","type":"Real"}],
        "outputs":[{"name":"supply_air_temp","type":"Real"}],
        "blocks":ahu["blocks"].as_array().unwrap().iter()
            .filter(|block| (block["name"].as_str().unwrap().starts_with("sat_mode_")
                && !block["name"].as_str().unwrap().ends_with("_permitted"))
                || ["sat_minimum", "sat_outdoor_low", "sat_outdoor_high", "sat_weather_line"]
                    .contains(&block["name"].as_str().unwrap()))
            .cloned().collect::<Vec<_>>(),
        "connections":ahu["connections"].as_array().unwrap().iter()
            .filter(|edge| {
                let target=edge["to"].as_str().unwrap();
                (target.starts_with("sat_mode_")
                    && !target.split('.').next().unwrap().ends_with("_permitted"))
                    || target.starts_with("sat_weather_line.")
            })
            .map(|edge| {
                let mut copy=edge.clone();
                if copy["from"] == "sat_output.y" { copy["from"] = json!("reset_max"); }
                copy
            }).collect::<Vec<_>>()
    });
    branch["connections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"from":"sat_mode_fan_select.y","to":"supply_air_temp"}));
    let branch: Block = serde_json::from_value(branch).unwrap();
    assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    for (mode, expected) in [
        (0, 26.0),
        (1, 17.5),
        (2, 17.5),
        (3, 12.0),
        (4, 35.0),
        (5, 35.0),
        (6, 26.0),
        (7, 26.0),
    ] {
        for fan in [false, true] {
            let inputs = HashMap::from([
                ("operation_mode".into(), json!(mode)),
                ("supply_fan_proven".into(), json!(fan)),
                ("reset_max".into(), json!(17.5)),
                ("outdoor_c".into(), json!(16.0)),
            ]);
            let expected = if fan { expected } else { 26.0 };
            assert_eq!(
                dynamic.execute(&branch, &inputs, 60.0, "").unwrap()["supply_air_temp"],
                json!(expected),
                "mode {mode}, fan {fan}"
            );
            assert_eq!(
                compiled.execute(&inputs, 60.0, false).unwrap().0["supply_air_temp"],
                json!(expected),
                "mode {mode}, fan {fan}"
            );
        }
    }
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_supply_temperature_dymola.txt"
    ));
    let series = |name: &str| -> Vec<f64> {
        source
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
    let modes = series("conTSupSet.uOpeMod=");
    let fans = series("conTSupSet.u1SupFan=");
    let outdoor = series("conTSupSet.TOut=");
    let reset_max = series("conTSupSet.maxSupTemRes.y=");
    let expected = series("conTSupSet.TAirSupSet=");
    assert_eq!(expected.len(), 101);
    for sample in 0..expected.len() {
        // Feed the archived trim output to the actual editable Line block.
        let inputs = HashMap::from([
            ("operation_mode".into(), json!(modes[sample] as i64)),
            ("supply_fan_proven".into(), json!(fans[sample] > 0.5)),
            ("reset_max".into(), json!(reset_max[sample] - 273.15)),
            ("outdoor_c".into(), json!(outdoor[sample] - 273.15)),
        ]);
        for output in [
            dynamic.execute(&branch, &inputs, 864.0, "").unwrap(),
            compiled.execute(&inputs, 864.0, false).unwrap().0,
        ] {
            let actual = output["supply_air_temp"].as_f64().unwrap();
            assert!(
                (actual - (expected[sample] - 273.15)).abs() < 5e-4,
                "OBC supply mode sample {sample}: {actual} vs {}",
                expected[sample] - 273.15
            );
        }
    }
}

#[test]
fn g36_sampled_supply_reset_tracks_pinned_obc_day() {
    let building: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let ahu = building["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["name"] == "ahu_1")
        .unwrap();
    let branch_blocks: Vec<Value> = ahu["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|block| {
            let name = block["name"].as_str().unwrap();
            name.starts_with("sat_")
                && !name.starts_with("sat_request_sum_")
                && name != "sat_request_real"
                && !name.ends_with("_permitted")
        })
        .cloned()
        .collect();
    let names: std::collections::HashSet<&str> = branch_blocks
        .iter()
        .map(|block| block["name"].as_str().unwrap())
        .collect();
    let mut connections: Vec<Value> = ahu["connections"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|edge| {
            let from = edge["from"].as_str().unwrap();
            let to = edge["to"].as_str().unwrap();
            let source = from.split('.').next().unwrap();
            let target = to.split('.').next().unwrap();
            if !names.contains(target)
                || (!names.contains(source)
                    && !["operation_mode", "supply_fan_proven", "outdoor_c"].contains(&source))
            {
                return None;
            }
            Some(edge.clone())
        })
        .collect();
    connections.push(json!({"from":"request_count","to":"sat_request_sample.u"}));
    connections.push(json!({"from":"sat_output.y","to":"reset_max"}));
    connections.push(json!({"from":"sat_mode_fan_select.y","to":"supply_air_temp"}));
    let branch: Block = serde_json::from_value(json!({
        "name":"SupplyResetReference","block_type":"Composite","category":"composite",
        "inputs":[{"name":"request_count","type":"Real"},
                  {"name":"operation_mode","type":"Integer"},
                  {"name":"supply_fan_proven","type":"Boolean"},
                  {"name":"outdoor_c","type":"Real"}],
        "outputs":[{"name":"reset_max","type":"Real"},
                   {"name":"supply_air_temp","type":"Real"}],
        "blocks":branch_blocks,"connections":connections
    }))
    .unwrap();
    assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_supply_temperature_dymola.txt"
    ));
    let series = |name: &str| -> Vec<f64> {
        source
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
    let expected_max = series("conTSupSet.maxSupTemRes.y=");
    let expected_supply = series("conTSupSet.TAirSupSet=");
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    let mut excess_max = 0.0_f64;
    let mut excess_supply = 0.0_f64;
    let mut previous_max = 18.0_f64;
    let mut previous_supply = 15.6_f64;
    // One-second input resolution catches request threshold crossings
    // close to a control sample without changing the 120-second clock.
    for tick in 0..=86_400 {
        let seconds = tick as f64;
        let phase = std::f64::consts::TAU * seconds / 86_400.0;
        let request_count = (6.0 * phase.sin()).abs().round();
        let fan = tick == 0 || (seconds - 1e-9).rem_euclid(43_200.0) < 21_600.0;
        let mode = (1.0 + seconds / 90_000.0).round() as i64;
        let inputs = HashMap::from([
            ("request_count".into(), json!(request_count)),
            ("operation_mode".into(), json!(mode)),
            ("supply_fan_proven".into(), json!(fan)),
            ("outdoor_c".into(), json!(18.0 + 5.0 * phase.sin())),
        ]);
        let ordinary = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
        let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
        for name in ["reset_max", "supply_air_temp"] {
            assert!(
                (ordinary[name].as_f64().unwrap() - fast[name].as_f64().unwrap()).abs() < 1e-8,
                "tick {tick}, {name}"
            );
        }
        if tick % 864 == 0 {
            let index = tick / 864;
            // At a 120-second event, the archive records the left limit.
            let at_event = tick > 0 && tick % 120 == 0;
            let max_value = if at_event {
                previous_max
            } else {
                ordinary["reset_max"].as_f64().unwrap()
            };
            let supply_value = if at_event {
                previous_supply
            } else {
                ordinary["supply_air_temp"].as_f64().unwrap()
            };
            excess_max = excess_max.max((max_value - (expected_max[index] - 273.15)).abs());
            excess_supply =
                excess_supply.max((supply_value - (expected_supply[index] - 273.15)).abs());
        }
        previous_max = ordinary["reset_max"].as_f64().unwrap();
        previous_supply = ordinary["supply_air_temp"].as_f64().unwrap();
    }
    // The finer input drive resolves the validation sine's rounded
    // request transitions before the 120-second sampler consumes them.
    assert!(
        excess_max < 0.0005 && excess_supply < 0.0005,
        "OBC day excess: max {excess_max} K, supply {excess_supply} K"
    );
}

#[test]
fn g36_ahu_plant_request_enables_water_loop_in_cdl() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let plant = building
        .blocks
        .iter()
        .find(|block| block.name == "plant")
        .unwrap();
    let mut inputs: HashMap<String, Value> = plant
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
    let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
    for valve in [0.0, 1.0] {
        inputs.insert("ahu_1_heating_valve".into(), json!(valve));
        inputs.insert("ahu_1_cooling_valve".into(), json!(valve));
        let ordinary = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for result in [&ordinary, &fast] {
            let enabled = valve > 0.95;
            assert_eq!(result["ahu_1_hot_plant_request"], json!(enabled as i64));
            assert_eq!(result["ahu_1_chilled_plant_request"], json!(enabled as i64));
            assert_eq!(result["hot_water_enable"], json!(enabled));
            assert_eq!(result["chilled_water_enable"], json!(enabled));
        }
    }
}

#[test]
fn g36_ahu_plant_requests_match_pinned_obc_trace() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let plant = building
        .blocks
        .iter()
        .find(|block| block.name == "plant")
        .unwrap();
    let mut value = serde_json::to_value(plant).unwrap();
    value["name"] = json!("AHUPlantRequests");
    let inputs = [
        "ahu_1_supply_actual_c",
        "ahu_1_supply_setpoint_c",
        "ahu_1_heating_valve",
        "ahu_1_cooling_valve",
    ];
    value["inputs"]
        .as_array_mut()
        .unwrap()
        .retain(|port| inputs.contains(&port["name"].as_str().unwrap()));
    value["outputs"] = json!([
        {"name":"hot_reset","type":"Integer","unit":"count"},
        {"name":"chilled_reset","type":"Integer","unit":"count"},
        {"name":"ahu_1_hot_plant_request","type":"Integer","unit":"count"},
        {"name":"ahu_1_chilled_plant_request","type":"Integer","unit":"count"}
    ]);
    value["blocks"]
        .as_array_mut()
        .unwrap()
        .retain(|block| block["name"].as_str().unwrap().starts_with("ahu_1_"));
    value["connections"].as_array_mut().unwrap().retain(|edge| {
        let from = edge["from"].as_str().unwrap();
        let to = edge["to"].as_str().unwrap();
        (to.starts_with("ahu_1_")
            || to == "ahu_1_hot_plant_request"
            || to == "ahu_1_chilled_plant_request")
            && (from.starts_with("ahu_1_") || inputs.contains(&from))
    });
    value["connections"].as_array_mut().unwrap().extend([
        json!({"from":"ahu_1_hot_select_three_requests.y","to":"hot_reset"}),
        json!({"from":"ahu_1_select_three_requests.y","to":"chilled_reset"}),
    ]);
    let request: Block = serde_json::from_value(value).unwrap();
    assert!(validate(&request).is_empty(), "{:?}", validate(&request));
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_ahu_plant_requests_dymola.txt"
    ));
    let series = |name: &str| -> Vec<f64> {
        let prefix = format!("{name}=");
        source
            .lines()
            .find(|line| line.starts_with(&prefix))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|field| field.trim().parse::<f64>().unwrap())
            .collect()
    };
    let hot_actual = series("plaReq.TAirSup");
    let hot_target = series("plaReq.TAirSupSet");
    let hot_valve = series("plaReq.uHeaCoiSet");
    let hot_reset = series("plaReq.yHotWatResReq");
    let hot_plant = series("plaReq.yHotWatPlaReq");
    let chilled_actual = series("plaReq1.TAirSup");
    let chilled_target = series("plaReq1.TAirSupSet");
    let chilled_valve = series("plaReq1.uCooCoiSet");
    let chilled_reset = series("plaReq1.yChiWatResReq");
    let chilled_plant = series("plaReq1.yChiPlaReq");
    assert_eq!(hot_actual.len(), 101);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&request).unwrap().unwrap();
    for tick in 0..=3_600 {
        let sample = tick / 36;
        let seconds = tick as f64;
        let hot_valve_input = 0.96 - 0.3 * ((seconds - 1_000.0) / 3_600.0).clamp(0.0, 1.0);
        let inputs = HashMap::from([
            (
                "ahu_1_supply_actual_c".into(),
                json!(12.0 + 8.0 * seconds / 3_600.0),
            ),
            (
                "ahu_1_supply_setpoint_c".into(),
                json!(20.0 + 15.0 * seconds / 3_600.0),
            ),
            ("ahu_1_heating_valve".into(), json!(hot_valve_input)),
            ("ahu_1_cooling_valve".into(), json!(0.0)),
        ]);
        // The OBC validation uses two separate AHU instances with different
        // supply-temperature inputs. Check the hot and chilled paths in
        // separate runs below rather than mixing their temperature signals.
        let actual = dynamic.execute(&request, &inputs, 1.0, "").unwrap();
        let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
        if tick % 36 != 0 {
            continue;
        }
        assert!((hot_actual[sample] - 273.15 - (12.0 + 8.0 * seconds / 3_600.0)).abs() < 5e-5);
        assert!((hot_target[sample] - 273.15 - (20.0 + 15.0 * seconds / 3_600.0)).abs() < 5e-5);
        assert!((hot_valve[sample] - hot_valve_input).abs() < 5e-5);
        for result in [&actual, &fast] {
            let expected = json!(hot_reset[sample] as i64);
            assert_eq!(result["hot_reset"], expected, "hot reset sample {sample}");
            assert_eq!(
                result["ahu_1_hot_plant_request"],
                json!(hot_plant[sample] as i64),
                "hot plant sample {sample}"
            );
        }
    }
    // Re-run with the second OBC AHU's chilled-water temperature trace.
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&request).unwrap().unwrap();
    for tick in 0..=3_600 {
        let sample = tick / 36;
        let seconds = tick as f64;
        let chilled_valve_input = 0.96 - 0.3 * ((seconds - 1_000.0) / 3_600.0).clamp(0.0, 1.0);
        let inputs = HashMap::from([
            (
                "ahu_1_supply_actual_c".into(),
                json!(15.0 + 8.0 * seconds / 3_600.0),
            ),
            (
                "ahu_1_supply_setpoint_c".into(),
                json!(14.5 + 6.0 * seconds / 3_600.0),
            ),
            ("ahu_1_heating_valve".into(), json!(0.0)),
            ("ahu_1_cooling_valve".into(), json!(chilled_valve_input)),
        ]);
        let actual = dynamic.execute(&request, &inputs, 1.0, "").unwrap();
        let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
        if tick % 36 != 0 {
            continue;
        }
        assert!((chilled_actual[sample] - 273.15 - (15.0 + 8.0 * seconds / 3_600.0)).abs() < 5e-5);
        assert!((chilled_target[sample] - 273.15 - (14.5 + 6.0 * seconds / 3_600.0)).abs() < 5e-5);
        assert!((chilled_valve[sample] - chilled_valve_input).abs() < 5e-5);
        for result in [&actual, &fast] {
            let expected = json!(chilled_reset[sample] as i64);
            assert_eq!(
                result["chilled_reset"], expected,
                "chilled reset sample {sample}"
            );
            assert_eq!(
                result["ahu_1_chilled_plant_request"],
                json!(chilled_plant[sample] as i64),
                "chilled plant sample {sample}"
            );
        }
    }
}

#[test]
fn g36_terminal_hot_water_tiers_follow_discharge_and_valve_feedback() {
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
    let mut request = serde_json::to_value(zone).unwrap();
    request["name"] = json!("TerminalHotRequests");
    request["inputs"] = json!([
        {"name":"target","type":"Real","unit":"°C"},
        {"name":"discharge_temp","type":"Real","unit":"°C"},
        {"name":"valve","type":"Real","unit":"fraction (0–1)"}
    ]);
    request["outputs"] = json!([
        {"name":"hot_water_reset_request","type":"Integer","unit":"count"},
        {"name":"hot_water_plant_request","type":"Integer","unit":"count"}
    ]);
    let is_request = |name: &str| name.starts_with("hot_reset_") || name.starts_with("hot_plant_");
    request["blocks"]
        .as_array_mut()
        .unwrap()
        .retain(|block| is_request(block["name"].as_str().unwrap()));
    request["connections"]
        .as_array_mut()
        .unwrap()
        .retain_mut(|edge| {
            let from = edge["from"].as_str().unwrap().to_owned();
            let to = edge["to"].as_str().unwrap();
            if !is_request(to.split('.').next().unwrap())
                && to != "hot_water_reset_request"
                && to != "hot_water_plant_request"
            {
                return false;
            }
            if from == "discharge_target.y" {
                edge["from"] = json!("target");
            } else if from == "reheat_capped.y" {
                edge["from"] = json!("valve");
            } else if !is_request(from.split('.').next().unwrap()) && from != "discharge_temp" {
                return false;
            }
            true
        });
    let request: Block = serde_json::from_value(request).unwrap();
    assert!(validate(&request).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&request).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("target".into(), json!(35.0)),
        ("discharge_temp".into(), json!(20.0)),
        ("valve".into(), json!(0.96)),
    ]);
    let mut step = |inputs: &HashMap<String, Value>| {
        let ordinary = dynamic.execute(&request, inputs, 60.0, "").unwrap();
        let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
        assert_eq!(
            ordinary["hot_water_reset_request"],
            fast["hot_water_reset_request"]
        );
        assert_eq!(
            ordinary["hot_water_plant_request"],
            fast["hot_water_plant_request"]
        );
        ordinary
    };
    // OBC TrueDelay defaults to delayOnInit=false: an initial deficit is
    // immediately eligible, while a later rising deficit waits five minutes.
    assert_eq!(step(&inputs)["hot_water_reset_request"], json!(2));
    for _ in 0..6 {
        step(&inputs);
    }
    assert_eq!(step(&inputs)["hot_water_reset_request"], json!(2));
    inputs.insert("discharge_temp".into(), json!(15.0));
    for _ in 0..6 {
        step(&inputs);
    }
    assert_eq!(step(&inputs)["hot_water_reset_request"], json!(3));
    inputs.insert("discharge_temp".into(), json!(35.0));
    inputs.insert("valve".into(), json!(0.9));
    assert_eq!(step(&inputs)["hot_water_reset_request"], json!(1));
    assert_eq!(step(&inputs)["hot_water_plant_request"], json!(1));
    inputs.insert("valve".into(), json!(0.8));
    assert_eq!(step(&inputs)["hot_water_reset_request"], json!(0));
    assert_eq!(step(&inputs)["hot_water_plant_request"], json!(1));
    inputs.insert("valve".into(), json!(0.05));
    assert_eq!(step(&inputs)["hot_water_plant_request"], json!(0));

    // The archived OBC validation records 101 output samples at 72 s
    // spacing. Its discharge deficit stays in the two-request band and
    // its valve never reaches the plant-request threshold.
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_system_requests_dymola.txt"
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
    let discharge = series("sysReq.TDis=");
    let valve = series("sysReq.uVal=");
    let reset = series("sysReq.yHeaValResReq=");
    let plant = series("sysReq.yHotWatPlaReq=");
    assert_eq!(
        (discharge.len(), valve.len(), reset.len(), plant.len()),
        (101, 101, 101, 101)
    );
    let mut reference_dynamic = Executor::default();
    let mut reference_compiled = CompiledExecutor::try_new(&request).unwrap().unwrap();
    for sample in 0..101 {
        let values = HashMap::from([
            ("target".into(), json!(30.0)),
            ("discharge_temp".into(), json!(discharge[sample] - 273.15)),
            ("valve".into(), json!(valve[sample])),
        ]);
        let actual = reference_dynamic
            .execute(&request, &values, 72.0, "")
            .unwrap();
        let fast = reference_compiled.execute(&values, 72.0, false).unwrap().0;
        for output in [&actual, &fast] {
            assert_eq!(
                output["hot_water_reset_request"],
                json!(reset[sample] as i64),
                "sample {sample}"
            );
            assert_eq!(
                output["hot_water_plant_request"],
                json!(plant[sample] as i64),
                "sample {sample}"
            );
        }
    }
}

#[test]
fn g36_zone_pressure_request_levels_are_editable_cdl() {
    let graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = graph["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["name"] == "zone_01")
        .unwrap();
    let mut needed = std::collections::HashSet::from(["pressure_importance_select".to_string()]);
    loop {
        let mut changed = false;
        for edge in zone["connections"].as_array().unwrap() {
            let target = edge["to"].as_str().unwrap().split('.').next().unwrap();
            let source = edge["from"].as_str().unwrap().split('.').next().unwrap();
            if needed.contains(target) && source.starts_with("pressure_") {
                changed |= needed.insert(source.to_string());
            }
        }
        if !changed {
            break;
        }
    }
    let mut branch = json!({
        "name": "PressureRequestLevels",
        "block_type": "Composite",
        "category": "composite",
        "inputs": [
            {"name":"damper_position","type":"Real","unit":"fraction (0–1)"},
            {"name":"airflow_setpoint","type":"Real","unit":"normalized airflow"},
            {"name":"measured_airflow","type":"Real","unit":"normalized airflow"}
        ],
        "outputs": [zone["outputs"].as_array().unwrap().iter()
            .find(|port| port["name"] == "pressure_reset_request").unwrap().clone()],
        "blocks": zone["blocks"].as_array().unwrap().iter()
            .filter(|block| needed.contains(block["name"].as_str().unwrap()))
            .cloned().collect::<Vec<_>>(),
        "connections": []
    });
    branch["connections"] = Value::Array(
        zone["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|edge| {
                let target = edge["to"].as_str().unwrap();
                let source = edge["from"].as_str().unwrap();
                if !needed.contains(target.split('.').next().unwrap())
                    && target != "pressure_reset_request"
                {
                    return None;
                }
                let mut copy = edge.clone();
                copy["from"] = json!(match source {
                    "heating_damper_select.y" => "damper_position",
                    "flow_override_select.y" => "airflow_setpoint",
                    other => other,
                });
                Some(copy)
            })
            .collect(),
    );
    let branch: Block = serde_json::from_value(branch).unwrap();
    assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
    let cases = [
        (0.96, 1.0, 0.40, 3),
        (0.96, 1.0, 0.60, 2),
        (0.96, 1.0, 0.80, 1),
        (0.90, 1.0, 0.40, 3),
        (0.90, 1.0, 0.80, 1),
        (0.84, 1.0, 0.80, 0),
        (0.96, 0.0, 0.0, 1),
        (0.96, 0.005, 0.0, 1),
        (0.96, 0.02, 0.0, 3),
    ];
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    for (damper, setpoint, measured, expected) in cases {
        let inputs = HashMap::from([
            ("damper_position".into(), json!(damper)),
            ("airflow_setpoint".into(), json!(setpoint)),
            ("measured_airflow".into(), json!(measured)),
        ]);
        for _ in 0..2 {
            let result = dynamic.execute(&branch, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                result["pressure_reset_request"], fast["pressure_reset_request"],
                "damper={damper}, setpoint={setpoint}, measured={measured}"
            );
        }
        let ordinary = dynamic.execute(&branch, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            ordinary["pressure_reset_request"],
            fast["pressure_reset_request"]
        );
        assert_eq!(
            ordinary["pressure_reset_request"],
            json!(expected),
            "damper={damper}, setpoint={setpoint}, measured={measured}"
        );
    }
    // OBC samples damper and flow together at 120 s. Once the damper
    // qualifies, a newly sampled flow deficit raises the tier immediately;
    // it does not start a second one-minute flow-deficit timer.
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("damper_position".into(), json!(0.96)),
        ("airflow_setpoint".into(), json!(1.0)),
        ("measured_airflow".into(), json!(0.8)),
    ]);
    let mut step = |inputs: &HashMap<String, Value>, expected: i64| {
        let ordinary = dynamic.execute(&branch, inputs, 60.0, "").unwrap();
        let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
        assert_eq!(ordinary["pressure_reset_request"], json!(expected));
        assert_eq!(fast["pressure_reset_request"], json!(expected));
    };
    step(&inputs, 1); // initial sample
    step(&inputs, 1); // t=60 s: held between samples
    inputs.insert("measured_airflow".into(), json!(0.4));
    step(&inputs, 3); // t=120 s: coherent sample raises the tier
    step(&inputs, 3); // t=180 s: held between samples
                      // OBC's 0.01 flow hysteresis holds a severe request just above
                      // the 50% threshold, then releases it above that band.
    inputs.insert("measured_airflow".into(), json!(0.505));
    step(&inputs, 3); // t=240 s: still below 0.50 + 0.01 clearing threshold
    step(&inputs, 3); // t=300 s: held sample
    inputs.insert("measured_airflow".into(), json!(0.515));
    step(&inputs, 2); // t=360 s: severe request clears, moderate persists
    step(&inputs, 2); // t=420 s: held sample
    inputs.insert("damper_position".into(), json!(0.9));
    step(&inputs, 2); // t=480 s: shared OBC latch remains above 85%
    step(&inputs, 2); // t=540 s: held between samples
    inputs.insert("damper_position".into(), json!(0.84));
    step(&inputs, 0); // t=600 s: shared latch clears all request tiers
    let importance = branch
        .blocks
        .iter()
        .find(|block| block.name == "pressure_importance")
        .unwrap();
    assert_eq!(importance.parameters[0].value, json!(1.0));
    let mut disabled = branch.clone();
    disabled
        .blocks
        .iter_mut()
        .find(|block| block.name == "pressure_importance")
        .unwrap()
        .parameters[0]
        .value = json!(0.0);
    let inputs = HashMap::from([
        ("damper_position".into(), json!(1.0)),
        ("airflow_setpoint".into(), json!(1.0)),
        ("measured_airflow".into(), json!(0.0)),
    ]);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&disabled).unwrap().unwrap();
    for _ in 0..3 {
        assert_eq!(
            dynamic.execute(&disabled, &inputs, 60.0, "").unwrap()["pressure_reset_request"],
            json!(0)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["pressure_reset_request"],
            json!(0)
        );
    }
}

#[test]
fn g36_zone_sat_requests_match_pinned_obc_trace() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_system_requests_dymola.txt"
    ));
    let series = |name: &str| -> Vec<f64> {
        let line = source.lines().find(|line| line.starts_with(name)).unwrap();
        let values = line.split_once('=').unwrap().1.trim();
        values[1..values.len() - 1]
            .split(',')
            .map(|value| value.trim().parse::<f64>().unwrap())
            .collect()
    };
    let setpoint = series("sysReq.TCooSet=");
    let expected = series("sysReq.yZonTemResReq=");
    assert_eq!(setpoint.len(), 101);
    assert_eq!(expected.len(), 101);
    let building: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = building["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["name"] == "zone_01")
        .unwrap();
    let mut branch = json!({
        "name":"CoolingSatRequestsReference",
        "block_type":"Composite",
        "category":"composite",
        "inputs":[
            {"name":"room_temp","type":"Real","unit":"°C"},
            {"name":"cool_setpoint","type":"Real","unit":"°C"},
            {"name":"cooling_loop","type":"Real","unit":"fraction (0–1)"},
            {"name":"after_suppression","type":"Boolean"}
        ],
        "outputs":[{"name":"cooling_sat_reset_request","type":"Integer","unit":"count"}],
        "blocks":zone["blocks"].as_array().unwrap().iter()
            .filter(|block| block["name"].as_str().unwrap().starts_with("sat_"))
            .cloned().collect::<Vec<_>>(),
        "connections":zone["connections"].as_array().unwrap().iter()
            .filter(|edge| edge["to"].as_str().unwrap().starts_with("sat_")
                || edge["to"] == "cooling_sat_reset_request")
            .map(|edge| {
                let mut copy = edge.clone();
                copy["from"] = json!(match edge["from"].as_str().unwrap() {
                    "window_cool_target.y" => "cool_setpoint",
                    "cool_pi.y" => "cooling_loop",
                    other => other,
                });
                if copy["to"] == "sat_two_eligible.u2" || copy["to"] == "sat_three_eligible.u2" {
                    copy["from"] = json!("after_suppression");
                }
                copy
            }).collect::<Vec<_>>()
    });
    for block in branch["blocks"].as_array_mut().unwrap() {
        if block["name"] == "sat_cooling_loop_high" {
            block["parameters"][0]["value"] = json!(0.94);
        }
    }
    let branch: Block = serde_json::from_value(branch).unwrap();
    assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    let mut actual = Vec::new();
    for tick in 0..=600 {
        let time = tick as f64 * 12.0;
        let recorded = tick / 6;
        let room = 299.15 + 2.0 * (std::f64::consts::TAU * time / 7200.0).sin();
        let cooling_loop = 1.0 - ((time - 1000.0) / 2000.0).clamp(0.0, 1.0);
        let inputs = HashMap::from([
            ("room_temp".into(), json!(room - 273.15)),
            ("cool_setpoint".into(), json!(setpoint[recorded] - 273.15)),
            ("cooling_loop".into(), json!(cooling_loop)),
            ("after_suppression".into(), json!(time > 720.0)),
        ]);
        let slow = dynamic.execute(&branch, &inputs, 12.0, "").unwrap();
        let fast = compiled.execute(&inputs, 12.0, false).unwrap().0;
        assert_eq!(
            slow["cooling_sat_reset_request"],
            fast["cooling_sat_reset_request"]
        );
        if tick % 6 == 0 {
            actual.push(slow["cooling_sat_reset_request"].as_i64().unwrap());
        }
    }
    let mismatches: Vec<_> = actual
        .iter()
        .zip(expected.iter())
        .enumerate()
        .filter(|(_, (actual, expected))| **actual as f64 != **expected)
        .map(|(index, (actual, expected))| (index, *actual, *expected as i64))
        .collect();
    assert!(
        mismatches.is_empty(),
        "SAT request mismatches: {mismatches:?}"
    );
}

#[test]
fn g36_zone_sat_requests_and_setpoint_suppression_are_editable_cdl() {
    let graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = graph["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["name"] == "zone_01")
        .unwrap();
    let mut branch = json!({
        "name": "CoolingSatRequests",
        "block_type": "Composite",
        "category": "composite",
        "inputs": [
            {"name":"room_temp","type":"Real","unit":"°C"},
            {"name":"cool_setpoint","type":"Real","unit":"°C"},
            {"name":"cooling_loop","type":"Real","unit":"fraction (0–1)"}
        ],
        "outputs": [zone["outputs"].as_array().unwrap().iter()
            .find(|port| port["name"] == "cooling_sat_reset_request").unwrap().clone()],
        "blocks": zone["blocks"].as_array().unwrap().iter()
            .filter(|block| block["name"].as_str().unwrap().starts_with("sat_"))
            .cloned().collect::<Vec<_>>(),
        "connections": []
    });
    branch["connections"] = Value::Array(
        zone["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|edge| {
                let target = edge["to"].as_str().unwrap();
                if !target.starts_with("sat_") && target != "cooling_sat_reset_request" {
                    return None;
                }
                let source = edge["from"].as_str().unwrap();
                let mut copy = edge.clone();
                copy["from"] = json!(match source {
                    "window_cool_target.y" => "cool_setpoint",
                    "cool_pi.y" => "cooling_loop",
                    other => other,
                });
                Some(copy)
            })
            .collect(),
    );
    let branch: Block = serde_json::from_value(branch).unwrap();
    assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    let mut evaluate = |room: f64, setpoint: f64, loop_value: f64, steps: usize| -> Value {
        let inputs = HashMap::from([
            ("room_temp".into(), json!(room)),
            ("cool_setpoint".into(), json!(setpoint)),
            ("cooling_loop".into(), json!(loop_value)),
        ]);
        let mut output = Value::Null;
        for _ in 0..steps {
            let result = dynamic.execute(&branch, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                result["cooling_sat_reset_request"],
                fast["cooling_sat_reset_request"]
            );
            output = result["cooling_sat_reset_request"].clone();
        }
        output
    };
    assert_eq!(evaluate(24.0, 24.0, 0.0, 2), json!(0));
    assert_eq!(evaluate(27.5, 24.0, 0.85, 4), json!(3));
    assert_eq!(evaluate(26.5, 24.0, 0.85, 3), json!(2));
    assert_eq!(evaluate(24.0, 24.0, 0.85, 3), json!(1));
    assert_eq!(evaluate(24.0, 24.0, 0.79, 3), json!(1));
    assert_eq!(evaluate(24.0, 24.0, 0.70, 3), json!(0));
    assert_eq!(evaluate(27.5, 23.0, 0.0, 2), json!(0));
    assert_eq!(evaluate(27.5, 23.0, 0.0, 35), json!(3));
    // A 2.2 K error yields 1188 s, so Level 2 returns before the old
    // fixed 1800 s suppression would have elapsed.
    assert_eq!(evaluate(24.2, 22.0, 0.0, 2), json!(0));
    assert_eq!(evaluate(24.2, 22.0, 0.0, 15), json!(0));
    assert_eq!(evaluate(24.2, 22.0, 0.0, 8), json!(2));
}

#[test]
fn periodic_sampler_holds_between_events_in_both_executors() {
    let graph: Block = serde_json::from_value(json!({
        "name":"SampledInput","block_type":"Composite","category":"composite",
        "inputs":[{"name":"u","type":"Real"}],
        "outputs":[{"name":"y","type":"Real"}],
        "blocks":[{"name":"sampled","block_type":"Buildings.Controls.OBC.CDL.Discrete.Sampler",
            "inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],
            "parameters":[{"name":"samplePeriod","value":120.0}]}],
        "connections":[{"from":"u","to":"sampled.u"},{"from":"sampled.y","to":"y"}]
    }))
    .unwrap();
    assert!(validate(&graph).is_empty());
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    // OBC samples at t=0, 120, and 240 seconds.
    for (input, expected) in [(1.0, 1.0), (2.0, 1.0), (3.0, 3.0), (4.0, 3.0), (5.0, 5.0)] {
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
fn sample_trigger_uses_absolute_phase_in_both_executors() {
    for (period, shift, step, expected) in [
        (120.0, 0.0, 60.0, vec![true, false, true, false, true]),
        (
            60.0,
            30.0,
            15.0,
            vec![false, false, true, false, false, false, true],
        ),
    ] {
        let graph: Block = serde_json::from_value(json!({
            "name":"clock","block_type":"SampleTrigger",
            "outputs":[{"name":"y","type":"Boolean"}],
            "parameters":[{"name":"period","value":period},{"name":"shift","value":shift}]
        }))
        .unwrap();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (tick, pulse) in expected.into_iter().enumerate() {
            let input = HashMap::new();
            let ordinary = dynamic.execute(&graph, &input, step, "").unwrap();
            let fast = compiled.execute(&input, step, false).unwrap().0;
            assert_eq!(
                ordinary["y"],
                json!(pulse),
                "dynamic clock at t={}",
                tick as f64 * step
            );
            assert_eq!(
                fast["y"],
                json!(pulse),
                "compiled clock at t={}",
                tick as f64 * step
            );
        }
    }
}

#[test]
fn divide_block_matches_compiled_executor() {
    let block: Block = serde_json::from_value(json!({
        "name":"quotient","block_type":"Buildings.Controls.OBC.CDL.Reals.Divide",
        "inputs":[{"name":"u1","type":"Real"},{"name":"u2","type":"Real"}],
        "outputs":[{"name":"y","type":"Real"}]
    }))
    .unwrap();
    let inputs = HashMap::from([("u1".into(), json!(3.0)), ("u2".into(), json!(4.0))]);
    let dynamic = Executor::default()
        .execute(&block, &inputs, 60.0, "")
        .unwrap();
    let mut compiled = CompiledExecutor::try_new(&block).unwrap().unwrap();
    let (fast, _) = compiled.execute(&inputs, 60.0, false).unwrap();
    assert_eq!(dynamic["y"], json!(0.75));
    assert_eq!(fast["y"], dynamic["y"]);
}

#[test]
fn reference_limiter() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/p_controller_limiter.json"
    )))
    .unwrap();
    assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
    let mut ex = Executor::default();
    let result = ex
        .execute(
            &graph,
            &HashMap::from([("e".into(), json!(5.0)), ("yMax".into(), json!(20.0))]),
            900.0,
            "",
        )
        .unwrap();
    assert_eq!(result.get("y"), Some(&json!(20.0)));
}

#[test]
fn obc_zone_status_unrecorded_outputs_follow_temperature_and_window() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/obc_zone_status.json"
    )))
    .unwrap();
    for (zone_k, window_closed, expected) in [
        (280.0, true, (false, false, true, 1800.0)),
        (310.0, true, (true, true, false, 1800.0)),
        (280.0, false, (false, true, true, 0.0)),
    ] {
        let inputs = HashMap::from([
            ("cooDowTim".into(), json!(1800.0)),
            ("warUpTim".into(), json!(1800.0)),
            ("TZon".into(), json!(zone_k)),
            ("TOccHeaSet".into(), json!(293.15)),
            ("TOccCooSet".into(), json!(297.15)),
            ("TUnoHeaSet".into(), json!(285.15)),
            ("TUnoCooSet".into(), json!(303.15)),
            ("u1Win".into(), json!(window_closed)),
        ]);
        let dynamic = Executor::default()
            .execute(&graph, &inputs, 864.0, "")
            .unwrap();
        let compiled = CompiledExecutor::try_new(&graph)
            .unwrap()
            .unwrap()
            .execute(&inputs, 864.0, false)
            .unwrap()
            .0;
        for actual in [&dynamic, &compiled] {
            assert_eq!(actual["yHigUnoCoo"], json!(expected.0));
            assert_eq!(actual["yEndSetBac"], json!(expected.1));
            assert_eq!(actual["yEndSetUp"], json!(expected.2));
            assert_eq!(actual["yCooTim"], json!(expected.3));
            assert_eq!(actual["yWarTim"], json!(expected.3));
        }
    }
}

#[test]
fn g36_terminal_alarms_require_persistent_measured_faults() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let mut executor = Executor::default();
    let mut inputs = HashMap::from([
        ("flow_override_code".into(), json!(0)),
        ("room_temp".into(), json!(27.0)),
        ("co2_ppm".into(), json!(420.0)),
        ("supply_air_temp".into(), json!(16.0)),
        ("supply_air_setpoint".into(), json!(16.0)),
        ("supply_pressure".into(), json!(1.0)),
        ("scheduled_occupied".into(), json!(true)),
        ("group_warmup".into(), json!(false)),
        ("group_cooldown".into(), json!(false)),
        ("group_setback".into(), json!(false)),
        ("group_setup".into(), json!(false)),
        ("group_freeze_setback".into(), json!(false)),
        ("window_closed".into(), json!(true)),
        ("local_override".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("zone_presence_raw".into(), json!(true)),
        ("occupancy_count".into(), json!(8.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("measured_airflow".into(), json!(0.0)),
        ("discharge_temp".into(), json!(40.0)),
    ]);
    for step in 0..36 {
        let outputs = executor.execute(zone, &inputs, 60.0, "").unwrap();
        // OBC TrueDelay passes a fault already present at initialization;
        // later low-flow faults still require five continuous minutes.
        assert_eq!(outputs["low_airflow_alarm"], json!(true));
        assert_eq!(outputs["high_discharge_alarm"], json!(step >= 5));
    }
    inputs.insert("measured_airflow".into(), json!(1.0));
    inputs.insert("discharge_temp".into(), json!(30.0));
    let recovered = executor.execute(zone, &inputs, 60.0, "").unwrap();
    assert_eq!(recovered["low_airflow_alarm"], false);
    assert_eq!(recovered["high_discharge_alarm"], false);
    let mut disabled = zone.clone();
    disabled
        .blocks
        .iter_mut()
        .find(|block| block.name == "pressure_importance")
        .unwrap()
        .parameters[0]
        .value = json!(0.0);
    inputs.insert("measured_airflow".into(), json!(0.0));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&disabled).unwrap().unwrap();
    for _ in 0..36 {
        let result = dynamic.execute(&disabled, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for point in [
            "pressure_reset_request",
            "low_airflow_alarm_level",
            "low_airflow_alarm",
        ] {
            assert_eq!(result[point], fast[point]);
            assert_eq!(
                result[point],
                if point == "low_airflow_alarm" {
                    json!(false)
                } else {
                    json!(0)
                },
                "{point}"
            );
        }
    }
}

#[test]
fn g36_minimum_discharge_reheat_requires_occupied_cdl_mode() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let zone = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let mut inputs = HashMap::from([
        ("flow_override_code".into(), json!(0)),
        ("room_temp".into(), json!(22.0)),
        ("co2_ppm".into(), json!(420.0)),
        ("supply_air_temp".into(), json!(8.0)),
        ("supply_air_setpoint".into(), json!(8.0)),
        ("supply_pressure".into(), json!(1.0)),
        ("discharge_temp".into(), json!(8.0)),
        ("scheduled_occupied".into(), json!(false)),
        ("group_warmup".into(), json!(false)),
        ("group_cooldown".into(), json!(false)),
        ("group_setback".into(), json!(false)),
        ("group_setup".into(), json!(false)),
        ("group_freeze_setback".into(), json!(false)),
        ("zone_presence_raw".into(), json!(false)),
        ("occupancy_count".into(), json!(0.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("window_closed".into(), json!(true)),
        ("local_override".into(), json!(false)),
        ("measured_airflow".into(), json!(0.95)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
    ]);
    for (occupied, expect_open) in [(false, false), (true, true)] {
        inputs.insert("scheduled_occupied".into(), json!(occupied));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
        let first = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let first_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            first["reheat_valve_position"],
            first_fast["reheat_valve_position"]
        );
        assert_eq!(
            first["reheat_valve_position"].as_f64().unwrap() > 0.0,
            expect_open
        );
        let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for result in [&slow, &fast] {
            let valve = result["reheat_valve_position"].as_f64().unwrap();
            assert_eq!(
                valve > 0.0,
                expect_open,
                "occupied={occupied}, valve={valve}"
            );
        }
    }
}
