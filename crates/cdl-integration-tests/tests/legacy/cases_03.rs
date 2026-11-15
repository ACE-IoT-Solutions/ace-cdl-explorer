use super::*;

#[test]
fn g36_low_discharge_alarm_uses_physical_heating_proof_and_cdl_thresholds() {
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
        ("room_temp".into(), json!(24.0)),
        ("co2_ppm".into(), json!(420.0)),
        ("supply_air_temp".into(), json!(14.0)),
        ("supply_air_setpoint".into(), json!(14.0)),
        ("supply_pressure".into(), json!(1.0)),
        ("discharge_temp".into(), json!(10.0)),
        ("scheduled_occupied".into(), json!(true)),
        ("group_warmup".into(), json!(false)),
        ("group_cooldown".into(), json!(false)),
        ("group_setback".into(), json!(false)),
        ("group_setup".into(), json!(false)),
        ("group_freeze_setback".into(), json!(false)),
        ("zone_presence_raw".into(), json!(true)),
        ("occupancy_count".into(), json!(8.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("window_closed".into(), json!(true)),
        ("local_override".into(), json!(false)),
        ("measured_airflow".into(), json!(0.2)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("hot_water_plant_proven".into(), json!(false)),
    ]);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
    let setpoint = dynamic.execute(zone, &inputs, 60.0, "").unwrap()["discharge_air_setpoint_c"]
        .as_f64()
        .unwrap();
    compiled.execute(&inputs, 60.0, false).unwrap();
    inputs.insert("discharge_temp".into(), json!(setpoint - 10.0));
    for _ in 0..12 {
        assert_eq!(
            dynamic.execute(zone, &inputs, 60.0, "").unwrap()
                ["low_discharge_temperature_alarm_level"],
            json!(0)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0
                ["low_discharge_temperature_alarm_level"],
            json!(0)
        );
    }
    inputs.insert("hot_water_plant_proven".into(), json!(true));
    for _ in 0..11 {
        let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow["low_discharge_temperature_alarm_level"], json!(3));
        assert_eq!(
            slow["low_discharge_temperature_alarm_level"],
            fast["low_discharge_temperature_alarm_level"]
        );
    }
    inputs.insert("discharge_temp".into(), json!(setpoint - 20.0));
    for step in 0..11 {
        let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            slow["low_discharge_temperature_alarm_level"],
            json!(if step >= 10 { 2 } else { 3 })
        );
        assert_eq!(
            slow["low_discharge_temperature_alarm_level"],
            fast["low_discharge_temperature_alarm_level"]
        );
    }
    inputs.insert("hot_water_plant_proven".into(), json!(false));
    assert_eq!(
        dynamic.execute(zone, &inputs, 60.0, "").unwrap()["low_discharge_temperature_alarm_level"],
        json!(0)
    );
    assert_eq!(
        compiled.execute(&inputs, 60.0, false).unwrap().0["low_discharge_temperature_alarm_level"],
        json!(0)
    );
    inputs.insert("hot_water_plant_proven".into(), json!(true));
    assert_eq!(
        dynamic.execute(zone, &inputs, 60.0, "").unwrap()["low_discharge_temperature_alarm_level"],
        json!(2)
    );
    assert_eq!(
        compiled.execute(&inputs, 60.0, false).unwrap().0["low_discharge_temperature_alarm_level"],
        json!(2)
    );
    let mut suppressed = zone.clone();
    suppressed
        .blocks
        .iter_mut()
        .find(|block| block.name == "low_discharge_importance")
        .unwrap()
        .parameters[0]
        .value = json!(0.0);
    let mut suppression_engine = Executor::default();
    for _ in 0..12 {
        assert_eq!(
            suppression_engine
                .execute(&suppressed, &inputs, 60.0, "")
                .unwrap()["low_discharge_temperature_alarm_level"],
            json!(0)
        );
    }
}

#[test]
fn g36_low_discharge_alarm_matches_pinned_obc_validation_trace() {
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
    let blocks: Vec<Value> = zone["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|block| {
            block["name"]
                .as_str()
                .unwrap()
                .starts_with("low_discharge_")
        })
        .cloned()
        .collect();
    let connections: Vec<Value> = zone["connections"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|edge| {
            let target = edge["to"].as_str().unwrap();
            if !target.starts_with("low_discharge_") {
                return None;
            }
            let mut edge = edge.clone();
            edge["from"] = match edge["from"].as_str().unwrap() {
                "local_occupied.y" => json!("occupied"),
                "discharge_target.y" => json!("target"),
                source => json!(source),
            };
            if target == "low_discharge_temperature_alarm_level" {
                edge["to"] = json!("alarm");
            }
            Some(edge)
        })
        .collect();
    let branch: Block = serde_json::from_value(json!({
        "name":"OBCReheatLowDischargeAlarm",
        "block_type":"Composite",
        "category":"composite",
        "inputs":[
            {"name":"occupied","type":"Boolean"},
            {"name":"supply_fan_proven","type":"Boolean"},
            {"name":"hot_water_plant_proven","type":"Boolean"},
            {"name":"target","type":"Real","unit":"°C"},
            {"name":"discharge_temp","type":"Real","unit":"°C"}
        ],
        "outputs":[{"name":"alarm","type":"Integer"}],
        "blocks":blocks,
        "connections":connections
    }))
    .unwrap();
    assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_reheat_alarms_dymola.txt"
    ));
    let expected: Vec<i64> = archive
        .lines()
        .find(|line| line.starts_with("ala.yLowTemAla="))
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .trim_matches(['[', ']'])
        .split(',')
        .map(|value| value.trim().parse::<f64>().unwrap() as i64)
        .collect();
    assert_eq!(expected.len(), 101);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    for tick in 0..=7_200 {
        let seconds = tick as f64;
        let inputs = HashMap::from([
            ("occupied".into(), json!(tick >= 850)),
            ("supply_fan_proven".into(), json!(tick < 5_625)),
            ("hot_water_plant_proven".into(), json!(tick < 6_750)),
            ("target".into(), json!(30.0)),
            (
                "discharge_temp".into(),
                json!(20.0 - 5.0 * (seconds / 3_600.0).min(1.0)),
            ),
        ]);
        let ordinary = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
        let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
        if tick % 72 == 0 {
            let sample = tick / 72;
            for result in [&ordinary, &fast] {
                assert_eq!(result["alarm"], json!(expected[sample]), "sample {sample}");
            }
        }
    }
}

#[test]
fn g36_airflow_sensor_alarm_matches_pinned_obc_validation_trace() {
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
    let mut blocks: Vec<Value> = zone["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|block| block["name"].as_str().unwrap().starts_with("sensor_"))
        .cloned()
        .collect();
    blocks
        .iter_mut()
        .find(|block| block["name"] == "sensor_cooling_design_flow")
        .unwrap()["parameters"][0]["value"] = json!(1.0);
    let connections: Vec<Value> = zone["connections"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|edge| {
            let target = edge["to"].as_str().unwrap();
            if !target.starts_with("sensor_") && target != "airflow_sensor_alarm_level" {
                return None;
            }
            let mut edge = edge.clone();
            if target == "airflow_sensor_alarm_level" {
                edge["to"] = json!("alarm");
            }
            Some(edge)
        })
        .collect();
    let branch: Block = serde_json::from_value(json!({
        "name":"OBCReheatAirflowSensorAlarm",
        "block_type":"Composite",
        "category":"composite",
        "inputs":[
            {"name":"supply_fan_proven","type":"Boolean"},
            {"name":"measured_airflow","type":"Real","unit":"normalized airflow"}
        ],
        "outputs":[{"name":"alarm","type":"Integer"}],
        "blocks":blocks,
        "connections":connections
    }))
    .unwrap();
    assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_reheat_alarms_dymola.txt"
    ));
    let expected: Vec<i64> = archive
        .lines()
        .find(|line| line.starts_with("ala.yFloSenAla="))
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .trim_matches(['[', ']'])
        .split(',')
        .map(|value| value.trim().parse::<f64>().unwrap() as i64)
        .collect();
    assert_eq!(expected.len(), 101);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    for tick in 0..=7_200 {
        let seconds = tick as f64;
        let inputs = HashMap::from([
            ("supply_fan_proven".into(), json!(tick < 5_625)),
            (
                "measured_airflow".into(),
                json!(0.1 + 0.3 * seconds / 7_200.0),
            ),
        ]);
        let ordinary = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
        let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
        if tick % 72 == 0 {
            let sample = tick / 72;
            for result in [&ordinary, &fast] {
                assert_eq!(result["alarm"], json!(expected[sample]), "sample {sample}");
            }
        }
    }
}

#[test]
fn g36_terminal_leak_alarms_use_measured_feedback_and_cdl_timers() {
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
        ("room_temp".into(), json!(24.0)),
        ("co2_ppm".into(), json!(420.0)),
        ("supply_air_temp".into(), json!(14.0)),
        ("supply_air_setpoint".into(), json!(14.0)),
        ("supply_pressure".into(), json!(1.0)),
        ("discharge_temp".into(), json!(20.0)),
        ("scheduled_occupied".into(), json!(false)),
        ("group_warmup".into(), json!(false)),
        ("group_cooldown".into(), json!(false)),
        ("group_setback".into(), json!(false)),
        ("group_setup".into(), json!(false)),
        ("group_freeze_setback".into(), json!(false)),
        ("zone_presence_raw".into(), json!(false)),
        ("occupancy_count".into(), json!(0.0)),
        ("window_closed".into(), json!(false)),
        ("local_override".into(), json!(false)),
        ("measured_airflow".into(), json!(0.0)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
    ]);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
    for step in 0..16 {
        let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow["damper_position"], json!(0.0));
        assert_eq!(
            slow["leaking_reheat_valve_alarm_level"],
            json!(if step >= 15 { 4 } else { 0 })
        );
        assert_eq!(
            slow["leaking_reheat_valve_alarm_level"],
            fast["leaking_reheat_valve_alarm_level"]
        );
        assert_eq!(slow["leaking_damper_alarm_level"], json!(0));
    }
    inputs.insert("discharge_temp".into(), json!(14.0));
    inputs.insert("measured_airflow".into(), json!(0.2));
    for step in 0..11 {
        let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow["leaking_reheat_valve_alarm_level"], json!(0));
        assert_eq!(
            slow["leaking_damper_alarm_level"],
            json!(if step >= 10 { 4 } else { 0 })
        );
        assert_eq!(
            slow["leaking_damper_alarm_level"],
            fast["leaking_damper_alarm_level"]
        );
    }
    inputs.insert("supply_fan_proven".into(), json!(false));
    assert_eq!(
        dynamic.execute(zone, &inputs, 60.0, "").unwrap()["leaking_damper_alarm_level"],
        json!(0)
    );
    assert_eq!(
        compiled.execute(&inputs, 60.0, false).unwrap().0["leaking_damper_alarm_level"],
        json!(0)
    );
}

#[test]
fn g36_terminal_leak_closed_thresholds_follow_obc_defaults() {
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
    for (constant_name, comparator_name) in [
        ("leak_damper_closed_limit", "leak_damper_command_closed"),
        ("leak_valve_closed_limit", "leak_valve_position_closed"),
    ] {
        let constant = zone
            .blocks
            .iter()
            .find(|block| block.name == constant_name)
            .unwrap();
        assert_eq!(param(constant, "k", 0.0), 0.05);
        let comparator = zone
            .blocks
            .iter()
            .find(|block| block.name == comparator_name)
            .unwrap();
        assert_eq!(param(comparator, "h", 0.0), 0.025);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(comparator).unwrap().unwrap();
        for (position, expected) in [(0.06, false), (0.04, true), (0.06, true), (0.08, false)] {
            let inputs =
                HashMap::from([("u1".into(), json!(position)), ("u2".into(), json!(0.05))]);
            let slow = dynamic.execute(comparator, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                slow["y"],
                json!(expected),
                "{comparator_name} at {position}"
            );
            assert_eq!(slow["y"], fast["y"]);
        }
    }
}

#[test]
fn g36_damper_tracks_measured_flow_in_both_executors() {
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
        ("supply_fan_proven".into(), json!(false)),
        ("actual_fan_speed".into(), json!(0.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("zone_presence_raw".into(), json!(true)),
        ("occupancy_count".into(), json!(8.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("measured_airflow".into(), json!(0.1)),
        ("discharge_temp".into(), json!(16.0)),
    ]);
    let low_flow = Executor::default()
        .execute(zone, &inputs, 60.0, "")
        .unwrap();
    inputs.insert("measured_airflow".into(), json!(0.8));
    let high_flow = Executor::default()
        .execute(zone, &inputs, 60.0, "")
        .unwrap();
    assert_eq!(low_flow["airflow"], json!(0.1));
    assert_eq!(high_flow["airflow"], json!(0.8));
    assert_eq!(low_flow["airflow_request"], high_flow["airflow_request"]);
    assert!(
        low_flow["damper_position"].as_f64().unwrap()
            > high_flow["damper_position"].as_f64().unwrap()
    );
    let compiled = CompiledExecutor::try_new(zone)
        .unwrap()
        .unwrap()
        .execute(&inputs, 60.0, false)
        .unwrap()
        .0;
    assert_eq!(high_flow["damper_position"], compiled["damper_position"]);
    assert_eq!(high_flow["airflow"], compiled["airflow"]);
}

#[test]
fn g36_heating_airflow_waits_for_warm_discharge() {
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
        ("room_temp".into(), json!(15.0)),
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
        ("supply_fan_proven".into(), json!(false)),
        ("actual_fan_speed".into(), json!(0.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("zone_presence_raw".into(), json!(true)),
        ("occupancy_count".into(), json!(8.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("measured_airflow".into(), json!(0.3)),
        ("discharge_temp".into(), json!(17.0)),
    ]);
    let cold = Executor::default()
        .execute(zone, &inputs, 60.0, "")
        .unwrap();
    inputs.insert("discharge_temp".into(), json!(19.0));
    let warm = Executor::default()
        .execute(zone, &inputs, 60.0, "")
        .unwrap();
    assert!(
        warm["damper_position"].as_f64().unwrap() > cold["damper_position"].as_f64().unwrap() + 0.2,
        "warm damper {}, cold damper {}, warm setpoint {}, cold setpoint {}",
        warm["damper_position"],
        cold["damper_position"],
        warm["airflow_request"],
        cold["airflow_request"]
    );
    let compiled = CompiledExecutor::try_new(zone)
        .unwrap()
        .unwrap()
        .execute(&inputs, 60.0, false)
        .unwrap()
        .0;
    assert_eq!(warm["damper_position"], compiled["damper_position"]);
    let mut edited = zone.clone();
    edited
        .blocks
        .iter_mut()
        .find(|block| block.name == "heating_airflow_margin")
        .unwrap()
        .parameters[0]
        .value = json!(5.0);
    let held = Executor::default()
        .execute(&edited, &inputs, 60.0, "")
        .unwrap();
    assert_eq!(held["damper_position"], cold["damper_position"]);
    inputs.insert("discharge_temp".into(), json!(17.0));
    inputs.insert("scheduled_occupied".into(), json!(false));
    inputs.insert("group_warmup".into(), json!(true));
    let warmup = Executor::default()
        .execute(zone, &inputs, 60.0, "")
        .unwrap();
    assert!(
        warmup["damper_position"].as_f64().unwrap()
            > cold["damper_position"].as_f64().unwrap() + 0.2,
        "warmup damper {}, cold damper {}",
        warmup["damper_position"],
        cold["damper_position"]
    );
}

#[test]
fn g36_fixed_dry_bulb_climate_cutoff_is_editable_cdl_in_both_executors() {
    // The three groups follow the pinned OBC AirEconomizerHighLimits
    // ASHRAE 90.1 cutoff branches: 1A/2A/3A/4A = 18 °C,
    // 5A/6A = 21 °C, and the remaining zones = 24 °C.
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut climate = building
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1")
        .unwrap()
        .clone();
    let names: HashSet<&str> = [
        "economizer_fixed_dry_bulb_cutoff",
        "economizer_climate_zone_group",
        "economizer_climate_code_real",
        "economizer_climate_low_boundary",
        "economizer_climate_mid_boundary",
        "economizer_climate_low_group",
        "economizer_climate_mid_group",
        "economizer_climate_cutoff_18",
        "economizer_climate_cutoff_21",
        "economizer_climate_cutoff_24",
        "economizer_climate_mid_select",
        "economizer_climate_cutoff_select",
        "economizer_use_climate_cutoff",
        "economizer_effective_fixed_cutoff",
    ]
    .into_iter()
    .collect();
    climate
        .blocks
        .retain(|block| names.contains(block.name.as_str()));
    climate.connections.retain(|edge| {
        names.contains(edge.source().0.as_str()) && names.contains(edge.target().0.as_str())
    });
    climate.inputs.clear();
    climate.outputs = vec![Connector {
        name: "cutoff".into(),
        kind: "Real".into(),
        unit: Some("°C".into()),
    }];
    climate.connections.push(Connection {
        from_path: "economizer_effective_fixed_cutoff.y".into(),
        to_path: "cutoff".into(),
        from_block: String::new(),
        from_output: String::new(),
        to_block: String::new(),
        to_input: String::new(),
    });
    for (group, cutoff) in [(1, 18.0), (2, 21.0), (3, 24.0)] {
        climate
            .blocks
            .iter_mut()
            .find(|block| block.name == "economizer_climate_zone_group")
            .unwrap()
            .parameters[0]
            .value = json!(group);
        let slow = Executor::default()
            .execute(&climate, &HashMap::new(), 15.0, "")
            .unwrap();
        let fast = CompiledExecutor::try_new(&climate)
            .unwrap()
            .unwrap()
            .execute(&HashMap::new(), 15.0, false)
            .unwrap()
            .0;
        assert_eq!(slow["cutoff"], json!(cutoff), "climate group {group}");
        assert_eq!(fast["cutoff"], slow["cutoff"]);
    }
    climate
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_fixed_dry_bulb_cutoff")
        .unwrap()
        .parameters[0]
        .value = json!(22.0);
    climate
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_use_climate_cutoff")
        .unwrap()
        .parameters[0]
        .value = json!(false);
    let slow = Executor::default()
        .execute(&climate, &HashMap::new(), 15.0, "")
        .unwrap();
    let fast = CompiledExecutor::try_new(&climate)
        .unwrap()
        .unwrap()
        .execute(&HashMap::new(), 15.0, false)
        .unwrap()
        .0;
    assert_eq!(slow["cutoff"], json!(22.0));
    assert_eq!(fast["cutoff"], slow["cutoff"]);
}

#[test]
fn g36_zone_ventilation_effectiveness_uses_obc_cooling_hysteresis() {
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
    let names = [
        "ventilation_cooling_supply",
        "ventilation_cooling_effectiveness",
        "ventilation_heating_effectiveness",
        "ventilation_effectiveness",
    ];
    let blocks: Vec<Value> = zone["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|block| names.contains(&block["name"].as_str().unwrap_or("")))
        .cloned()
        .collect();
    assert_eq!(blocks.len(), 4);
    let graph: Block = serde_json::from_value(json!({
        "name":"ventilation_effectiveness_test", "block_type":"Composite", "category":"composite",
        "inputs":[{"name":"room_temp","type":"Real"},{"name":"discharge_temp","type":"Real"}],
        "outputs":[{"name":"effectiveness","type":"Real"}],
        "blocks":blocks,
        "connections":[
            {"from":"room_temp","to":"ventilation_cooling_supply.u1"},
            {"from":"discharge_temp","to":"ventilation_cooling_supply.u2"},
            {"from":"ventilation_cooling_effectiveness.y","to":"ventilation_effectiveness.u1"},
            {"from":"ventilation_heating_effectiveness.y","to":"ventilation_effectiveness.u2"},
            {"from":"ventilation_cooling_supply.y","to":"ventilation_effectiveness.u3"},
            {"from":"ventilation_effectiveness.y","to":"effectiveness"}
        ]
    }))
    .unwrap();
    assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (room, discharge, expected) in [
        (22.0, 22.0, 0.8),
        (22.0, 20.0, 1.0),
        (22.0, 22.1, 1.0),
        (22.0, 22.3, 0.8),
        (22.1, 22.3, 0.8),
        (22.4, 22.3, 1.0),
    ] {
        let inputs = HashMap::from([
            ("room_temp".into(), json!(room)),
            ("discharge_temp".into(), json!(discharge)),
        ]);
        let standard = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            standard["effectiveness"],
            json!(expected),
            "room={room}, discharge={discharge}"
        );
        assert_eq!(fast["effectiveness"], standard["effectiveness"]);
    }
}

#[test]
fn g36_common_damper_pi_normalizes_flow_and_resets_on_fan_proof() {
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
    let names = [
        "minimum_oa_design_flow",
        "minimum_oa_design_floor",
        "minimum_oa_design_safe",
        "minimum_oa_setpoint_normalized",
        "minimum_oa_measured_normalized",
        "minimum_oa_pi",
    ];
    let blocks: Vec<Value> = ahu["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|block| names.contains(&block["name"].as_str().unwrap_or("")))
        .cloned()
        .collect();
    assert_eq!(blocks.len(), names.len());
    assert_eq!(
        blocks
            .iter()
            .find(|block| block["name"] == "minimum_oa_pi")
            .unwrap()["block_type"],
        "Buildings.Controls.OBC.CDL.Reals.PIDWithReset"
    );
    let graph: Block = serde_json::from_value(json!({
        "name":"minimum_oa_loop", "block_type":"Composite", "category":"composite",
        "inputs":[{"name":"setpoint","type":"Real"},{"name":"measured","type":"Real"},{"name":"fan_proven","type":"Boolean"}],
        "outputs":[{"name":"normalized_setpoint","type":"Real"},{"name":"normalized_measured","type":"Real"},{"name":"command","type":"Real"}],
        "blocks":blocks,
        "connections":[
            {"from":"minimum_oa_design_flow.y","to":"minimum_oa_design_safe.u1"},
            {"from":"minimum_oa_design_floor.y","to":"minimum_oa_design_safe.u2"},
            {"from":"setpoint","to":"minimum_oa_setpoint_normalized.u1"},
            {"from":"measured","to":"minimum_oa_measured_normalized.u1"},
            {"from":"minimum_oa_design_safe.y","to":"minimum_oa_setpoint_normalized.u2"},
            {"from":"minimum_oa_design_safe.y","to":"minimum_oa_measured_normalized.u2"},
            {"from":"minimum_oa_setpoint_normalized.y","to":"minimum_oa_pi.u_s"},
            {"from":"minimum_oa_measured_normalized.y","to":"minimum_oa_pi.u_m"},
            {"from":"fan_proven","to":"minimum_oa_pi.trigger"},
            {"from":"minimum_oa_setpoint_normalized.y","to":"normalized_setpoint"},
            {"from":"minimum_oa_measured_normalized.y","to":"normalized_measured"},
            {"from":"minimum_oa_pi.y","to":"command"}
        ]
    })).unwrap();
    assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for (fan, expected_command) in [
        (false, None),
        (false, None),
        (true, Some(0.0)),
        (true, None),
    ] {
        let inputs = HashMap::from([
            ("setpoint".into(), json!(0.45)),
            ("measured".into(), json!(0.15)),
            ("fan_proven".into(), json!(fan)),
        ]);
        let regular = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(regular, fast);
        assert!((regular["normalized_setpoint"].as_f64().unwrap() - 0.6).abs() < 1e-9);
        assert!((regular["normalized_measured"].as_f64().unwrap() - 0.2).abs() < 1e-9);
        if let Some(expected) = expected_command {
            assert_eq!(regular["command"], json!(expected));
        } else {
            assert!(regular["command"].as_f64().unwrap() > 0.0);
        }
    }
    let mut larger_design = graph.clone();
    larger_design
        .blocks
        .iter_mut()
        .find(|block| block.name == "minimum_oa_design_flow")
        .unwrap()
        .parameters[0]
        .value = json!(1.5);
    let inputs = HashMap::from([
        ("setpoint".into(), json!(0.45)),
        ("measured".into(), json!(0.15)),
        ("fan_proven".into(), json!(false)),
    ]);
    let larger = Executor::default()
        .execute(&larger_design, &inputs, 60.0, "")
        .unwrap();
    assert!((larger["normalized_setpoint"].as_f64().unwrap() - 0.3).abs() < 1e-9);
    assert!((larger["normalized_measured"].as_f64().unwrap() - 0.1).abs() < 1e-9);
}

#[test]
fn g36_common_damper_matches_pinned_obc_flow_ramp() {
    // The official Common_VOut_flow validation keeps the fan proven and
    // occupied while measured flow ramps from 0.61 to 0.81 over 1,800 s.
    // Its inputs are already normalized; use a design flow of one here.
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_common_vout_flow_dymola.txt"
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
    let expected_pi = series("damLim.damLimCon.y=");
    let expected_damper = series("damLim.yOutDam_min=");
    assert_eq!((expected_pi.len(), expected_damper.len()), (101, 101));

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
    let names = [
        "minimum_oa_design_flow",
        "minimum_oa_design_floor",
        "minimum_oa_design_safe",
        "minimum_oa_setpoint_normalized",
        "minimum_oa_measured_normalized",
        "minimum_oa_pi",
        "return_damper_pi_midpoint",
        "minimum_oa_lower_half",
        "minimum_oa_outdoor_stroke",
    ];
    let mut blocks: Vec<Block> = ahu
        .blocks
        .iter()
        .filter(|block| names.contains(&block.name.as_str()))
        .cloned()
        .collect();
    assert_eq!(blocks.len(), names.len());
    blocks
        .iter_mut()
        .find(|block| block.name == "minimum_oa_design_flow")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "k")
        .unwrap()
        .value = json!(1.0);
    let graph: Block = serde_json::from_value(json!({
        "name":"CommonVOutFlow", "block_type":"Composite", "category":"composite",
        "inputs":[{"name":"setpoint","type":"Real"},
            {"name":"measured","type":"Real"},{"name":"fan_proven","type":"Boolean"}],
        "outputs":[{"name":"pi","type":"Real"},{"name":"outdoor_min","type":"Real"}],
        "blocks":blocks,
        "connections":[
            {"from":"minimum_oa_design_flow.y","to":"minimum_oa_design_safe.u1"},
            {"from":"minimum_oa_design_floor.y","to":"minimum_oa_design_safe.u2"},
            {"from":"setpoint","to":"minimum_oa_setpoint_normalized.u1"},
            {"from":"measured","to":"minimum_oa_measured_normalized.u1"},
            {"from":"minimum_oa_design_safe.y","to":"minimum_oa_setpoint_normalized.u2"},
            {"from":"minimum_oa_design_safe.y","to":"minimum_oa_measured_normalized.u2"},
            {"from":"minimum_oa_setpoint_normalized.y","to":"minimum_oa_pi.u_s"},
            {"from":"minimum_oa_measured_normalized.y","to":"minimum_oa_pi.u_m"},
            {"from":"fan_proven","to":"minimum_oa_pi.trigger"},
            {"from":"minimum_oa_pi.y","to":"pi"},
            {"from":"minimum_oa_pi.y","to":"minimum_oa_lower_half.u1"},
            {"from":"return_damper_pi_midpoint.y","to":"minimum_oa_lower_half.u2"},
            {"from":"minimum_oa_lower_half.y","to":"minimum_oa_outdoor_stroke.u"},
            {"from":"minimum_oa_outdoor_stroke.y","to":"outdoor_min"}
        ]
    }))
    .unwrap();
    assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
    for edge in &graph.connections {
        let source_name = edge.from_path.split('.').next().unwrap_or("");
        let target_name = edge.to_path.split('.').next().unwrap_or("");
        if names.contains(&source_name) && names.contains(&target_name) {
            assert!(
                ahu.connections
                    .iter()
                    .any(|shipped| shipped.from_path == edge.from_path
                        && shipped.to_path == edge.to_path),
                "shipped AHU no longer contains {} -> {}",
                edge.from_path,
                edge.to_path
            );
        }
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    let mut max_pi_error = 0.0_f64;
    let mut max_damper_error = 0.0_f64;
    for second in 0..=1_800 {
        let measured = 0.61 + 0.2 * second as f64 / 1_800.0;
        let inputs = HashMap::from([
            ("setpoint".into(), json!(0.71)),
            ("measured".into(), json!(measured)),
            ("fan_proven".into(), json!(true)),
        ]);
        let regular = dynamic
            .execute(&graph, &inputs, if second == 0 { 0.0 } else { 1.0 }, "")
            .unwrap();
        let faster = compiled
            .execute(&inputs, if second == 0 { 0.0 } else { 1.0 }, false)
            .unwrap()
            .0;
        assert_eq!(regular, faster, "second {second}");
        if second % 18 == 0 {
            let sample = second / 18;
            max_pi_error =
                max_pi_error.max((regular["pi"].as_f64().unwrap() - expected_pi[sample]).abs());
            max_damper_error = max_damper_error
                .max((regular["outdoor_min"].as_f64().unwrap() - expected_damper[sample]).abs());
        }
    }
    assert!(max_pi_error < 5e-5, "PI error {max_pi_error}");
    assert!(
        max_damper_error < 1e-4,
        "outdoor damper limit error {max_damper_error}"
    );
}

#[test]
fn g36_common_damper_disabled_loop_keeps_obc_pi_setpoint() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_common_loop_disable_dymola.txt"
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
    let names = [
        "minimum_oa_design_flow",
        "minimum_oa_design_floor",
        "minimum_oa_design_safe",
        "minimum_oa_setpoint_normalized",
        "minimum_oa_measured_normalized",
        "minimum_oa_pi",
        "minimum_oa_mode_enabled",
        "minimum_oa_off_setpoint",
        "minimum_oa_setpoint",
        "minimum_oa_off_command",
        "minimum_oa_active",
        "return_damper_pi_midpoint",
        "minimum_oa_lower_half",
        "minimum_oa_outdoor_stroke",
    ];
    let mut blocks: Vec<Block> = ahu
        .blocks
        .iter()
        .filter(|block| names.contains(&block.name.as_str()))
        .cloned()
        .collect();
    assert_eq!(blocks.len(), names.len());
    blocks
        .iter_mut()
        .find(|block| block.name == "minimum_oa_design_flow")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "k")
        .unwrap()
        .value = json!(1.0);
    let graph: Block = serde_json::from_value(json!({
        "name":"CommonDisabled", "block_type":"Composite", "category":"composite",
        "inputs":[{"name":"raw_setpoint","type":"Real"},
            {"name":"measured","type":"Real"},
            {"name":"fan_proven","type":"Boolean"},
            {"name":"occupied","type":"Boolean"}],
        "outputs":[{"name":"pi","type":"Real"},
            {"name":"outdoor_min","type":"Real"},
            {"name":"published_setpoint","type":"Real"},
            {"name":"enabled","type":"Boolean"}],
        "blocks":blocks,
        "connections":[
            {"from":"minimum_oa_design_flow.y","to":"minimum_oa_design_safe.u1"},
            {"from":"minimum_oa_design_floor.y","to":"minimum_oa_design_safe.u2"},
            {"from":"raw_setpoint","to":"minimum_oa_setpoint_normalized.u1"},
            {"from":"measured","to":"minimum_oa_measured_normalized.u1"},
            {"from":"minimum_oa_design_safe.y","to":"minimum_oa_setpoint_normalized.u2"},
            {"from":"minimum_oa_design_safe.y","to":"minimum_oa_measured_normalized.u2"},
            {"from":"minimum_oa_setpoint_normalized.y","to":"minimum_oa_pi.u_s"},
            {"from":"minimum_oa_measured_normalized.y","to":"minimum_oa_pi.u_m"},
            {"from":"fan_proven","to":"minimum_oa_pi.trigger"},
            {"from":"occupied","to":"minimum_oa_mode_enabled.u1"},
            {"from":"fan_proven","to":"minimum_oa_mode_enabled.u2"},
            {"from":"minimum_oa_mode_enabled.y","to":"enabled"},
            {"from":"raw_setpoint","to":"minimum_oa_setpoint.u1"},
            {"from":"minimum_oa_off_setpoint.y","to":"minimum_oa_setpoint.u2"},
            {"from":"minimum_oa_mode_enabled.y","to":"minimum_oa_setpoint.u3"},
            {"from":"minimum_oa_setpoint.y","to":"published_setpoint"},
            {"from":"minimum_oa_pi.y","to":"pi"},
            {"from":"minimum_oa_pi.y","to":"minimum_oa_lower_half.u1"},
            {"from":"return_damper_pi_midpoint.y","to":"minimum_oa_lower_half.u2"},
            {"from":"minimum_oa_lower_half.y","to":"minimum_oa_outdoor_stroke.u"},
            {"from":"minimum_oa_outdoor_stroke.y","to":"minimum_oa_active.u1"},
            {"from":"minimum_oa_off_command.y","to":"minimum_oa_active.u2"},
            {"from":"minimum_oa_mode_enabled.y","to":"minimum_oa_active.u3"},
            {"from":"minimum_oa_active.y","to":"outdoor_min"}
        ]
    }))
    .unwrap();
    assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
    for edge in &graph.connections {
        let source_name = edge.from_path.split('.').next().unwrap_or("");
        let target_name = edge.to_path.split('.').next().unwrap_or("");
        if names.contains(&source_name) && names.contains(&target_name) {
            assert!(
                ahu.connections
                    .iter()
                    .any(|shipped| shipped.from_path == edge.from_path
                        && shipped.to_path == edge.to_path),
                "shipped AHU no longer contains {} -> {}",
                edge.from_path,
                edge.to_path
            );
        }
    }
    for target in [
        "minimum_oa_setpoint.u1",
        "minimum_oa_setpoint_normalized.u1",
    ] {
        assert!(
            ahu.connections
                .iter()
                .any(|edge| edge.from_path == "vent_effective_limit.y" && edge.to_path == target),
            "shipped AHU must feed raw ventilation setpoint to {target}"
        );
    }
    for (fan, occupied, prefix) in [(false, true, "damLim"), (true, false, "damLim1")] {
        let expected_pi = series(&format!("{prefix}.damLimCon.y="));
        let expected_min = series(&format!("{prefix}.yOutDam_min="));
        assert_eq!(expected_pi.len(), 101);
        assert_eq!(expected_min, vec![0.0, 0.0]);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        let mut max_error = 0.0_f64;
        for second in 0..=1_800 {
            let inputs = HashMap::from([
                ("raw_setpoint".into(), json!(0.71)),
                (
                    "measured".into(),
                    json!(0.61 + 0.2 * second as f64 / 1_800.0),
                ),
                ("fan_proven".into(), json!(fan)),
                ("occupied".into(), json!(occupied)),
            ]);
            let dt = if second == 0 { 0.0 } else { 1.0 };
            let regular = dynamic.execute(&graph, &inputs, dt, "").unwrap();
            let faster = compiled.execute(&inputs, dt, false).unwrap().0;
            assert_eq!(regular, faster, "{prefix} second {second}");
            assert_eq!(regular["enabled"], json!(false));
            assert_eq!(regular["published_setpoint"], json!(0.0));
            assert_eq!(regular["outdoor_min"], json!(0.0));
            if second % 18 == 0 {
                max_error = max_error
                    .max((regular["pi"].as_f64().unwrap() - expected_pi[second / 18]).abs());
            }
        }
        assert!(max_error < 5e-5, "{prefix} PI error {max_error}");
    }
}

#[test]
fn examples_produce_declared_outputs() {
    for text in [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/obc_zone_status.json"
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
        let inputs = graph
            .inputs
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    json!(if p.kind == "Boolean" || p.name.contains("pressure") {
                        1.0
                    } else if p.name.contains("supply_air") {
                        13.0
                    } else {
                        22.0
                    }),
                )
            })
            .collect();
        let output = Executor::default()
            .execute(&graph, &inputs, 900.0, "")
            .unwrap();
        assert!(!output.is_empty(), "{} returned no outputs", graph.name);
    }
}

#[test]
fn ahu_and_plant_are_executed_from_the_building_graph() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(20.0)),
        ("heat_request_kw".into(), json!(10.0)),
        ("cool_request_kw".into(), json!(5.0)),
        ("ahu_1_flow_request".into(), json!(2.0)),
        ("ahu_2_flow_request".into(), json!(2.0)),
        ("ahu_1_actual_supply_air_temp".into(), json!(16.0)),
        ("ahu_2_actual_supply_air_temp".into(), json!(16.0)),
        ("ahu_1_mixed_air_temp".into(), json!(20.0)),
        ("ahu_2_mixed_air_temp".into(), json!(20.0)),
        ("hot_water_supply_temp_c".into(), json!(45.0)),
        ("hot_water_return_temp_c".into(), json!(40.0)),
        ("chilled_water_supply_temp_c".into(), json!(7.0)),
        ("chilled_water_return_temp_c".into(), json!(12.0)),
        ("hot_water_flow_kg_s".into(), json!(1.0)),
        ("chilled_water_flow_kg_s".into(), json!(1.0)),
    ]);
    for zone in 1..=10 {
        inputs.insert(format!("zone_{zone:02}_room_temp"), json!(22.0));
        inputs.insert(format!("zone_{zone:02}_supply_pressure"), json!(1.0));
        inputs.insert(format!("zone_{zone:02}_occupied"), json!(false));
    }
    graph
        .blocks
        .iter_mut()
        .find(|block| block.name == "plant")
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.name == "heat_pump_dispatch_fraction")
        .unwrap()
        .parameters[0]
        .value = json!(0.5);
    let baseline = Executor::default()
        .execute(&graph, &inputs, 900.0, "")
        .unwrap();
    assert_eq!(baseline["ahu_1_supply_air_temp"], json!(16.0));
    assert_eq!(baseline["plant_boiler_kw"], json!(5.0));
    let mut traced = Executor::default();
    traced.trace_targets = vec![
        "FourPipeBuildingControls.plant|backup_fraction.y".into(),
        "FourPipeBuildingControls.plant|edge.2".into(),
        "FourPipeBuildingControls.ahu_1|is_cold.y".into(),
    ];
    traced.execute(&graph, &inputs, 900.0, "").unwrap();
    assert_eq!(
        traced
            .trace_values
            .get("FourPipeBuildingControls.plant|backup_fraction.y"),
        Some(&json!(0.5))
    );
    assert_eq!(
        traced
            .trace_values
            .get("FourPipeBuildingControls.plant|edge.2"),
        Some(&json!(0.5))
    );
    assert_eq!(
        traced
            .trace_values
            .get("FourPipeBuildingControls.ahu_1|is_cold.y"),
        Some(&json!(false))
    );
    let ahu = graph.blocks.iter_mut().find(|b| b.name == "ahu_1").unwrap();
    ahu.blocks
        .iter_mut()
        .find(|b| b.name == "mild_supply")
        .unwrap()
        .parameters[0]
        .value = json!(19.0);
    let plant = graph.blocks.iter_mut().find(|b| b.name == "plant").unwrap();
    plant
        .blocks
        .iter_mut()
        .find(|b| b.name == "one")
        .unwrap()
        .parameters[0]
        .value = json!(0.75);
    let changed = Executor::default()
        .execute(&graph, &inputs, 900.0, "")
        .unwrap();
    assert_eq!(changed["ahu_1_supply_air_temp"], json!(19.0));
    assert_eq!(changed["plant_boiler_kw"], json!(2.5));
}
