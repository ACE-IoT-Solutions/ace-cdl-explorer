use super::*;

#[test]
fn g36_unequal_water_volumes_conserve_energy_across_fine_steps() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: -10.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut sim = Simulation::new_test(
        graph,
        &weather,
        r#"{"step_seconds":900,"water_volume_kg":[8000,3000]}"#,
    )
    .unwrap();
    assert_eq!(sim.effective_step_seconds, 15);
    let chunk: Value = serde_json::from_str(&sim.step_chunk_json(672).unwrap()).unwrap();
    let mut rows = chunk["rows"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 11);
    let next: Value = serde_json::from_str(&sim.step_chunk_json(48).unwrap()).unwrap();
    rows.extend(next["rows"].as_array().unwrap().iter().cloned());
    assert_eq!(rows.len(), 12);
    let mut any_source = false;
    for (index, row) in rows.iter().enumerate() {
        for loop_index in 0..2 {
            let start = row["water_interval_start_c"][loop_index].as_f64().unwrap();
            let end = row["water_interval_end_c"][loop_index].as_f64().unwrap();
            let capacity = row["water_capacity_kwh_per_k"][loop_index]
                .as_f64()
                .unwrap();
            let source = row["water_interval_source_kwh"][loop_index]
                .as_f64()
                .unwrap();
            let delivered = row["water_interval_delivered_kwh"][loop_index]
                .as_f64()
                .unwrap();
            any_source |= source > 0.0;
            let stored = (if loop_index == 0 {
                end - start
            } else {
                start - end
            }) * capacity;
            assert!((stored - source + delivered).abs() < 1e-8);
            if index > 0 {
                let previous_end = rows[index - 1]["water_interval_end_c"][loop_index]
                    .as_f64()
                    .unwrap();
                assert!((start - previous_end).abs() < 1e-9);
            }
        }
    }
    assert!(
        any_source,
        "cold occupied G36 replay should operate a water source"
    );
}

#[test]
fn simulation_step_resolves_sample_trigger_period_and_phase() {
    let mut graph: Block = serde_json::from_value(json!({
        "name": "clock", "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.SampleTrigger",
        "outputs": [{"name": "y", "type": "Boolean"}],
        "parameters": [{"name": "period", "value": 60.0}, {"name": "shift", "value": 15.0}]
    }))
    .unwrap();
    assert_eq!(sampled_step(&graph, 900).unwrap(), 15);
    graph.parameters[1].value = json!(30.0);
    assert_eq!(sampled_step(&graph, 900).unwrap(), 30);
    graph.parameters[0].value = json!(45.0);
    assert_eq!(sampled_step(&graph, 900).unwrap(), 15);
    graph.parameters[1].value = json!(5.0);
    assert!(sampled_step(&graph, 900)
        .unwrap_err()
        .contains("SampleTrigger shift"));
    graph.parameters[1].value = json!(0.0);
    graph.parameters[0].value = json!(10.0);
    assert!(sampled_step(&graph, 900)
        .unwrap_err()
        .contains("SampleTrigger period"));

    let mut building: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    for group in building["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|block| {
            block["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("zone_group_"))
        })
    {
        let trigger = group["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "optimal_daily_sample")
            .unwrap();
        trigger["parameters"][0]["value"] = json!(45.0);
        trigger["parameters"][1]["value"] = json!(15.0);
    }
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 20.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false,
        };
        35_040
    ])
    .unwrap();
    let sim = Simulation::new_test(&building.to_string(), &weather, "{}").unwrap();
    assert_eq!(sim.effective_step_seconds(), 15);
}

#[test]
fn g36_ahu_aggregates_zone_requests_and_responds_to_measured_airflow() {
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
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(0.0)),
        ("flow_request".into(), json!(5.0)),
        ("return_air_temp".into(), json!(24.0)),
        ("mixed_air_temp".into(), json!(15.0)),
        ("actual_supply_air_temp".into(), json!(16.0)),
        ("duct_pressure".into(), json!(0.2)),
        ("outdoor_airflow".into(), json!(0.0)),
        ("zone_01_pressure_request".into(), json!(1)),
        ("zone_02_pressure_request".into(), json!(1)),
        ("zone_03_pressure_request".into(), json!(1)),
        ("zone_04_pressure_request".into(), json!(0)),
        ("zone_05_pressure_request".into(), json!(0)),
        ("freeze_stat_trip".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
        ("freeze_reset".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    let mut ex = Executor::default();
    let mut optimized = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    let mut result = HashMap::new();
    for _ in 0..20 {
        result = ex.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = optimized.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            fast["pressure_request_count"],
            result["pressure_request_count"]
        );
        assert_eq!(
            fast["duct_pressure_setpoint"],
            result["duct_pressure_setpoint"]
        );
    }
    assert_eq!(result["pressure_request_count"], json!(3));
    assert!(result["duct_pressure_setpoint"].as_f64().unwrap() > 0.35);
    assert!(result["duct_pressure_setpoint"].as_f64().unwrap() <= 0.8);
    assert!(result["minimum_outdoor_airflow"].as_f64().unwrap() > 0.0);
    assert!(result["outdoor_air_fraction"].as_f64().unwrap() > 0.2);
    let mut recovered = inputs.clone();
    recovered.insert("outdoor_airflow".into(), json!(3.0));
    let after = ex.execute(ahu, &recovered, 60.0, "").unwrap();
    let fast = optimized.execute(&recovered, 60.0, false).unwrap().0;
    assert_eq!(
        fast["duct_pressure_setpoint"],
        after["duct_pressure_setpoint"]
    );
    assert!(
        after["outdoor_air_fraction"].as_f64().unwrap()
            <= result["outdoor_air_fraction"].as_f64().unwrap()
    );
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        recovered.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
    }
    let high = after["duct_pressure_setpoint"].as_f64().unwrap();
    for _ in 0..20 {
        let reference = ex.execute(ahu, &recovered, 60.0, "").unwrap();
        let fast = optimized.execute(&recovered, 60.0, false).unwrap().0;
        assert_eq!(
            fast["duct_pressure_setpoint"],
            reference["duct_pressure_setpoint"]
        );
        result = reference;
    }
    assert!(result["duct_pressure_setpoint"].as_f64().unwrap() < high);
    recovered.insert("flow_request".into(), json!(0.0));
    assert_eq!(
        ex.execute(ahu, &recovered, 60.0, "").unwrap()["duct_pressure_setpoint"],
        json!(0.35)
    );
}

#[test]
fn g36_supply_fan_alarm_follows_cdl_command_and_raw_motor_status() {
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
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(18.0)),
        ("flow_request".into(), json!(5.0)),
        ("return_air_temp".into(), json!(24.0)),
        ("mixed_air_temp".into(), json!(20.0)),
        ("actual_supply_air_temp".into(), json!(16.0)),
        ("duct_pressure".into(), json!(0.4)),
        ("outdoor_airflow".into(), json!(0.0)),
        ("freeze_stat_trip".into(), json!(false)),
        ("supply_fan_proven".into(), json!(false)),
        ("actual_fan_speed".into(), json!(0.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
        ("freeze_reset".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    for step in 0..4 {
        let result = dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
        let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
        assert!(result["fan_speed"].as_f64().unwrap() > 0.01);
        assert_eq!(
            result["supply_fan_alarm_level"],
            json!(if step == 0 { 0 } else { 2 })
        );
        assert_eq!(
            result["supply_fan_alarm_level"].as_f64().unwrap(),
            fast["supply_fan_alarm_level"].as_f64().unwrap()
        );
    }
    inputs.insert("supply_fan_proven".into(), json!(true));
    let proven = dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
    assert_eq!(proven["supply_fan_alarm_level"], json!(0));
    inputs.insert("flow_request".into(), json!(0.0));
    inputs.insert("occupied_mode".into(), json!(false));
    for step in 0..6 {
        let result = dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
        let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
        assert_eq!(result["fan_speed"], json!(0.0));
        assert_eq!(
            result["supply_fan_alarm_level"],
            json!(if step < 4 { 0 } else { 4 })
        );
        assert_eq!(
            result["supply_fan_alarm_level"].as_f64().unwrap(),
            fast["supply_fan_alarm_level"].as_f64().unwrap()
        );
    }
}

#[test]
fn g36_fan_speed_tracking_alarm_matches_dynamic_and_compiled_cdl() {
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
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(28.0)),
        ("flow_request".into(), json!(5.0)),
        ("return_air_temp".into(), json!(29.0)),
        ("mixed_air_temp".into(), json!(28.0)),
        ("actual_supply_air_temp".into(), json!(20.0)),
        ("duct_pressure".into(), json!(0.2)),
        ("outdoor_airflow".into(), json!(0.0)),
        ("freeze_stat_trip".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(0.3)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
        ("freeze_reset".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    for step in 0..8 {
        let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert!(result["fan_speed"].as_f64().unwrap() > 0.45);
        assert_eq!(
            result["fan_speed_tracking_alarm_level"],
            json!(if step < 5 { 0 } else { 3 })
        );
        assert_eq!(
            result["fan_speed_tracking_alarm_level"].as_f64().unwrap(),
            fast["fan_speed_tracking_alarm_level"].as_f64().unwrap()
        );
    }
    inputs.insert("actual_fan_speed".into(), json!(1.0));
    let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
    let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(result["fan_speed_tracking_alarm_level"], json!(0));
    assert_eq!(fast["fan_speed_tracking_alarm_level"].as_f64(), Some(0.0));
}

#[test]
fn g36_ahu_damper_tracking_alarms_match_dynamic_and_compiled_cdl() {
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
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(35.0)),
        ("flow_request".into(), json!(5.0)),
        ("return_air_temp".into(), json!(28.0)),
        ("mixed_air_temp".into(), json!(28.0)),
        ("actual_supply_air_temp".into(), json!(20.0)),
        ("duct_pressure".into(), json!(0.2)),
        ("outdoor_airflow".into(), json!(0.0)),
        ("freeze_stat_trip".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(1.0)),
        ("actual_return_damper_position".into(), json!(0.0)),
        ("occupied_mode".into(), json!(true)),
        ("freeze_reset".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    let mut last_damper_command = (0.2, 0.8);
    for step in 0..13 {
        let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for damper in ["outdoor", "return"] {
            let alarm = format!("{damper}_damper_tracking_alarm_level");
            let expected = if step < 10 { 0.0 } else { 3.0 };
            assert_eq!(result[&alarm].as_f64(), Some(expected));
            assert_eq!(fast[&alarm].as_f64(), Some(expected));
        }
        last_damper_command = (
            result["outdoor_air_damper_position"].as_f64().unwrap(),
            result["return_air_damper_position"].as_f64().unwrap(),
        );
    }
    inputs.insert(
        "actual_outdoor_damper_position".into(),
        json!(last_damper_command.0),
    );
    inputs.insert(
        "actual_return_damper_position".into(),
        json!(last_damper_command.1),
    );
    let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
    let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    for damper in ["outdoor", "return"] {
        let alarm = format!("{damper}_damper_tracking_alarm_level");
        assert_eq!(result[&alarm].as_f64(), Some(0.0));
        assert_eq!(fast[&alarm].as_f64(), Some(0.0));
    }
}

#[test]
fn g36_ahu_coil_valve_tracking_is_timed_and_editable_cdl() {
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
    for (mixed, service) in [(0.0, "heating"), (30.0, "cooling")] {
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(0.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(mixed)),
            ("actual_supply_air_temp".into(), json!(16.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("actual_heating_coil_valve_position".into(), json!(0.0)),
            ("actual_cooling_coil_valve_position".into(), json!(0.0)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let alarm = format!("{service}_coil_valve_tracking_alarm_level");
        let command = format!("{service}_coil_valve_position");
        let sensor = format!("actual_{service}_coil_valve_position");
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        let mut result = HashMap::new();
        for step in 0..12 {
            result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            let expected = if step < 10 { 0.0 } else { 3.0 };
            assert_eq!(
                result[&alarm].as_f64(),
                Some(expected),
                "{service} step {step}"
            );
            assert_eq!(
                fast[&alarm].as_f64(),
                Some(expected),
                "{service} compiled step {step}"
            );
        }
        let commanded = result[&command].as_f64().unwrap();
        assert!(commanded > 0.25, "{service} command {commanded}");
        inputs.insert(sensor, json!(commanded));
        let cleared = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let cleared_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(cleared[&alarm].as_f64(), Some(0.0));
        assert_eq!(cleared_fast[&alarm].as_f64(), Some(0.0));
        let mut tuned = ahu.clone();
        tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == format!("{service}_coil_valve_tracking_tolerance"))
            .unwrap()
            .parameters[0]
            .value = json!(1.0);
        inputs.insert(format!("actual_{service}_coil_valve_position"), json!(0.0));
        let mut tuned_dynamic = Executor::default();
        let mut tuned_compiled = CompiledExecutor::try_new(&tuned).unwrap().unwrap();
        for _ in 0..12 {
            let result = tuned_dynamic.execute(&tuned, &inputs, 60.0, "").unwrap();
            let fast = tuned_compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(result[&alarm].as_f64(), Some(0.0));
            assert_eq!(fast[&alarm].as_f64(), Some(0.0));
        }
    }
}

#[test]
fn physical_ahu_valve_fault_reaches_g36_cdl_tracking_alarm() {
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ahu_1")
        .unwrap()["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "heating_coil_valve_tracking_tolerance")
        .unwrap()["parameters"][0]["value"] = json!(0.05);
    let graph = graph.to_string();
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: -5.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut sim = Simulation::new_test(
        &graph,
        &weather,
        r#"{"ahu_coil_valve_faults":[{"ahu":1,"service":"heating","stuck_position":0,"start_hour":7.75,"duration_hours":2}],"ahu_alarm_acknowledgements":[{"ahu":1,"start_hour":8.25,"duration_hours":0.25}]}"#,
    )
    .unwrap();
    let mut healthy = Simulation::new_test(&graph, &weather, "{}").unwrap();
    let alarm = "FourPipeG36Controls|output.ahu_1_heating_coil_valve_tracking_alarm_level";
    let acknowledged =
        "FourPipeG36Controls|output.ahu_1_heating_coil_valve_tracking_alarm_acknowledged";
    for runner in [&mut sim, &mut healthy] {
        start_present_window(runner);
        runner.temps = [15.0; 10];
        runner.last_zone_airflow = [0.4; 10];
        runner.last_heat_request = 200.0;
        runner.last_water_flow_kg_s = [2.0, 2.0];
        runner.last_water_pump_running = [true, true];
        runner
            .set_traces_json(&json!([alarm, acknowledged]).to_string())
            .unwrap();
    }
    let rows = run_weather_intervals(&mut sim, 3);
    let healthy_rows = run_weather_intervals(&mut healthy, 3);
    let first = &rows["rows"][0];
    let second = &rows["rows"][1];
    let command = first["ahu_1_heating_coil_valve_position"].as_f64().unwrap();
    let actual = first["ahu_1_heating_coil_valve_actual_position"]
        .as_f64()
        .unwrap();
    assert!(
        command > actual + 0.05,
        "command {command}, actual {actual}"
    );
    assert_eq!(second["traces"][alarm].as_f64(), Some(3.0));
    assert_eq!(healthy_rows["rows"][1]["traces"][alarm].as_f64(), Some(0.0));
    assert_eq!(second["traces"][acknowledged].as_f64(), Some(0.0));
    assert_eq!(rows["rows"][2]["traces"][alarm].as_f64(), Some(3.0));
    assert_eq!(rows["rows"][2]["traces"][acknowledged].as_f64(), Some(1.0));
    assert_eq!(
        healthy_rows["rows"][2]["traces"][acknowledged].as_f64(),
        Some(0.0)
    );
    assert!(
        healthy_rows["rows"][0]["ahu_1_heating_coil_valve_actual_position"]
            .as_f64()
            .unwrap()
            > actual
    );
    assert!(
        healthy_rows["rows"][1]["ahu_1_heating_coil_delivered_kw"]
            .as_f64()
            .unwrap()
            > second["ahu_1_heating_coil_delivered_kw"].as_f64().unwrap()
    );
}

#[test]
fn ahu_coil_valve_sensor_bias_changes_cdl_alarm_without_changing_physical_valve() {
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ahu_1")
        .unwrap()["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "heating_coil_valve_tracking_tolerance")
        .unwrap()["parameters"][0]["value"] = json!(0.05);
    let graph = graph.to_string();
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: -5.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut biased = Simulation::new_test(
        &graph,
        &weather,
        r#"{"ahu_coil_valve_sensor_events":[{"ahu":1,"service":"heating","bias":-1,"start_hour":7.75,"duration_hours":1}]}"#,
    )
    .unwrap();
    let mut healthy = Simulation::new_test(&graph, &weather, "{}").unwrap();
    let alarm = "FourPipeG36Controls|output.ahu_1_heating_coil_valve_tracking_alarm_level";
    for runner in [&mut biased, &mut healthy] {
        start_present_window(runner);
        runner.temps = [15.0; 10];
        runner.last_zone_airflow = [0.4; 10];
        runner.last_heat_request = 200.0;
        runner.last_water_flow_kg_s = [2.0, 2.0];
        runner.last_water_pump_running = [true, true];
        runner.set_traces_json(&json!([alarm]).to_string()).unwrap();
    }
    let biased_rows = run_weather_intervals(&mut biased, 2);
    let healthy_rows = run_weather_intervals(&mut healthy, 2);
    let sensed = &biased_rows["rows"][1];
    let physical = &healthy_rows["rows"][1];
    assert_eq!(
        sensed["ahu_1_heating_coil_valve_measured_position"],
        json!(0.0)
    );
    assert!(
        physical["ahu_1_heating_coil_valve_measured_position"]
            .as_f64()
            .unwrap()
            > 0.05
    );
    assert_eq!(sensed["traces"][alarm], json!(3.0));
    assert_eq!(physical["traces"][alarm], json!(0.0));
    for field in [
        "ahu_1_heating_coil_valve_actual_position",
        "ahu_1_heating_coil_delivered_kw",
        "hot_water_actual_flow_kg_s",
    ] {
        let difference =
            (sensed[field].as_f64().unwrap() - physical[field].as_f64().unwrap()).abs();
        assert!(
            difference < 1e-9,
            "{field} changed with sensor bias: {difference}"
        );
    }
}

#[test]
fn physical_fan_failure_reduces_air_delivery_and_cdl_reports_it() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 28.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut failed = Simulation::new_test(
        graph,
        &weather,
        r#"{"fan_faults":[{"equipment":"ahu_1_stuck_off","start_hour":0,"duration_hours":2}]}"#,
    )
    .unwrap();
    normal.temps = [29.0; 10];
    failed.temps = [29.0; 10];
    let alarm = "FourPipeG36Controls|output.ahu_1_supply_fan_alarm_level";
    failed.set_traces_json(&json!([alarm]).to_string()).unwrap();
    let normal_rows = run_weather_intervals(&mut normal, 2);
    let failed_rows = run_weather_intervals(&mut failed, 2);
    assert!(normal.last_fan_running[0]);
    assert!(!failed.last_fan_running[0]);
    assert!(
        normal_rows["rows"][1]["zone_airflow"][0].as_f64().unwrap()
            > failed_rows["rows"][1]["zone_airflow"][0].as_f64().unwrap()
    );
    assert_eq!(failed_rows["rows"][1]["traces"][alarm], json!(2.0));
}

#[test]
fn physical_fan_startup_delay_withholds_proof_and_air_until_motor_runs() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 28.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut delayed =
        Simulation::new_test(graph, &weather, r#"{"ahu_fan_startup_seconds":[1200,0]}"#).unwrap();
    normal.temps = [29.0; 10];
    delayed.temps = [29.0; 10];
    let alarm = "FourPipeG36Controls|output.ahu_1_supply_fan_alarm_level";
    let command = "FourPipeG36Controls|output.ahu_1_fan_speed";
    delayed
        .set_traces_json(&json!([alarm, command]).to_string())
        .unwrap();
    let normal_rows = run_weather_intervals(&mut normal, 2);
    let delayed_rows = run_weather_intervals(&mut delayed, 2);
    let startup = &delayed_rows["rows"][0];
    let running = &delayed_rows["rows"][1];
    assert!(
        normal_rows["rows"][0]["ahu_1_actual_fan_speed"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(startup["traces"][command].as_f64().unwrap() > 0.0);
    assert_eq!(startup["ahu_1_actual_fan_speed"], json!(0.0));
    assert_eq!(startup["traces"][alarm], json!(2.0));
    assert!(running["ahu_1_actual_fan_speed"].as_f64().unwrap() > 0.0);
    assert_eq!(running["traces"][alarm], json!(0.0));
    assert!(
        normal_rows["rows"][0]["zone_airflow"][0].as_f64().unwrap()
            > startup["zone_airflow"][0].as_f64().unwrap()
    );
}

#[test]
fn physical_fan_stuck_on_uses_energy_and_cdl_reports_failed_stop() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 22.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false
        };
        35_040
    ])
    .unwrap();
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut stuck = Simulation::new_test(
        graph,
        &weather,
        r#"{"fan_faults":[{"equipment":"ahu_1_stuck_on","start_hour":0,"duration_hours":2}]}"#,
    )
    .unwrap();
    let alarm = "FourPipeG36Controls|output.ahu_1_supply_fan_alarm_level";
    stuck.set_traces_json(&json!([alarm]).to_string()).unwrap();
    let _ = run_weather_intervals(&mut normal, 2);
    let fault_rows = run_weather_intervals(&mut stuck, 2);
    assert!(!normal.last_fan_running[0]);
    assert!(stuck.last_fan_running[0]);
    assert!(stuck.electricity_kwh > normal.electricity_kwh);
    assert_eq!(fault_rows["rows"][1]["traces"][alarm], json!(4.0));
}

#[test]
fn physical_fan_speed_cap_reduces_delivery_and_cdl_reports_tracking_fault() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 28.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut capped = Simulation::new_test(
        graph,
        &weather,
        r#"{"fan_faults":[{"equipment":"ahu_1_speed_capped","start_hour":0,"duration_hours":2}]}"#,
    )
    .unwrap();
    normal.temps = [29.0; 10];
    capped.temps = [29.0; 10];
    let command = "FourPipeG36Controls|output.ahu_1_fan_speed";
    let alarm = "FourPipeG36Controls|output.ahu_1_fan_speed_tracking_alarm_level";
    capped
        .set_traces_json(&json!([command, alarm]).to_string())
        .unwrap();
    let normal_rows = run_weather_intervals(&mut normal, 4);
    let fault_rows = run_weather_intervals(&mut capped, 4);
    let normal_row = &normal_rows["rows"][3];
    let fault_row = &fault_rows["rows"][3];
    assert!(fault_row["traces"][command].as_f64().unwrap() > 0.45);
    assert_eq!(fault_row["ahu_1_actual_fan_speed"], json!(0.3));
    assert!(normal_row["ahu_1_actual_fan_speed"].as_f64().unwrap() > 0.3);
    assert!(
        normal_row["delivered_airflow"].as_f64().unwrap()
            > fault_row["delivered_airflow"].as_f64().unwrap()
    );
    assert_eq!(fault_row["traces"][alarm], json!(3.0));
}

#[test]
fn fan_speed_sensor_bias_changes_cdl_alarm_without_changing_physical_delivery() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 28.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut biased = Simulation::new_test(
        graph,
        &weather,
        r#"{"fan_speed_sensor_events":[{"ahu":1,"bias":-0.5,"start_hour":0,"duration_hours":1}]}"#,
    )
    .unwrap();
    normal.temps = [29.0; 10];
    biased.temps = [29.0; 10];
    let alarm = "FourPipeG36Controls|output.ahu_1_fan_speed_tracking_alarm_level";
    normal.set_traces_json(&json!([alarm]).to_string()).unwrap();
    biased.set_traces_json(&json!([alarm]).to_string()).unwrap();
    let normal_rows = run_weather_intervals(&mut normal, 6);
    let biased_rows = run_weather_intervals(&mut biased, 6);
    let normal_row = &normal_rows["rows"][3];
    let biased_row = &biased_rows["rows"][3];
    assert_eq!(normal_row["traces"][alarm], json!(0.0));
    assert_eq!(biased_row["traces"][alarm], json!(3.0));
    assert_eq!(
        normal_row["ahu_1_actual_fan_speed"],
        biased_row["ahu_1_actual_fan_speed"]
    );
    assert_eq!(
        normal_row["delivered_airflow"],
        biased_row["delivered_airflow"]
    );
    assert_eq!(normal_row["electric_kw"], biased_row["electric_kw"]);
    assert!(
        biased_row["ahu_1_measured_fan_speed"].as_f64().unwrap()
            < biased_row["ahu_1_actual_fan_speed"].as_f64().unwrap()
    );
    let cleared = &biased_rows["rows"][5];
    assert_eq!(cleared["traces"][alarm], json!(0.0));
    assert_eq!(
        cleared["ahu_1_measured_fan_speed"],
        cleared["ahu_1_actual_fan_speed"]
    );
}

#[test]
fn physical_ahu_damper_stroke_follows_cdl_command_and_fault_override() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 20.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut slow = Simulation::new_test(
        graph,
        &weather,
        r#"{"step_seconds":60,"ahu_damper_stroke_seconds":600}"#,
    )
    .unwrap();
    slow.step_chunk_json(1).unwrap();
    assert!((slow.last_outdoor_damper_position[0] - 0.1).abs() < 1e-9);
    slow.step_chunk_json(1).unwrap();
    assert!((slow.last_outdoor_damper_position[0] - 0.2).abs() < 1e-9);
    let mut fast = Simulation::new_test(
        graph,
        &weather,
        r#"{"step_seconds":60,"ahu_damper_stroke_seconds":60}"#,
    )
    .unwrap();
    fast.step_chunk_json(1).unwrap();
    assert!((fast.last_outdoor_damper_position[0] - 0.2).abs() < 1e-9);
    let mut faulted = Simulation::new_test(graph, &weather,
        r#"{"step_seconds":60,"ahu_damper_stroke_seconds":600,"ahu_damper_faults":[{"ahu":1,"damper":"outdoor","stuck_position":1,"start_hour":0,"duration_hours":1}]}"#).unwrap();
    faulted.step_chunk_json(1).unwrap();
    assert_eq!(faulted.last_outdoor_damper_position[0], 1.0);
}

#[test]
fn physical_ahu_damper_faults_change_air_and_cdl_reports_tracking() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 35.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut outdoor = Simulation::new_test(
        graph,
        &weather,
        r#"{"ahu_damper_faults":[{"ahu":1,"damper":"outdoor","stuck_position":1,"start_hour":0,"duration_hours":12}]}"#,
    )
    .unwrap();
    let mut return_air = Simulation::new_test(
        graph,
        &weather,
        r#"{"ahu_damper_faults":[{"ahu":1,"damper":"return","stuck_position":0,"start_hour":0,"duration_hours":12}]}"#,
    )
    .unwrap();
    for sim in [&mut normal, &mut outdoor, &mut return_air] {
        sim.temps = [28.0; 10];
        // Start during the synthetic occupied period so fan proof can
        // qualify the damper tracking alarm after its ten-minute delay.
        sim.cursor = 32 * 900 / sim.effective_step_seconds;
    }
    let outdoor_alarm = "FourPipeG36Controls|output.ahu_1_outdoor_damper_tracking_alarm_level";
    let return_alarm = "FourPipeG36Controls|output.ahu_1_return_damper_tracking_alarm_level";
    let traces = json!([outdoor_alarm, return_alarm]).to_string();
    outdoor.set_traces_json(&traces).unwrap();
    return_air.set_traces_json(&traces).unwrap();
    let normal_rows = run_weather_intervals(&mut normal, 4);
    let outdoor_rows = run_weather_intervals(&mut outdoor, 4);
    let return_rows = run_weather_intervals(&mut return_air, 4);
    let base = &normal_rows["rows"][3];
    let open = &outdoor_rows["rows"][3];
    let closed = &return_rows["rows"][3];
    assert!(
        base["ahu_1_actual_outdoor_damper_position"]
            .as_f64()
            .unwrap()
            < 0.7
    );
    assert_eq!(open["ahu_1_actual_outdoor_damper_position"], json!(1.0));
    assert!(
        open["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
            > base["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
    );
    assert!(
        open["ahu_1_mixed_air_c"].as_f64().unwrap() > base["ahu_1_mixed_air_c"].as_f64().unwrap()
    );
    assert_eq!(
        open["traces"][outdoor_alarm],
        json!(3.0),
        "outdoor fault row: {open}"
    );
    assert_eq!(closed["ahu_1_actual_return_damper_position"], json!(0.0));
    // The economizer can compensate for a closed return path by opening
    // outdoor air. The resulting mix changes even if fan delivery holds.
    assert!(
        closed["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
            > base["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
    );
    assert!(
        closed["ahu_1_mixed_air_c"].as_f64().unwrap() > base["ahu_1_mixed_air_c"].as_f64().unwrap()
    );
    assert_eq!(
        closed["traces"][return_alarm],
        json!(3.0),
        "return fault row: {closed}"
    );
}

#[test]
fn ahu_damper_sensor_bias_changes_cdl_alarm_without_changing_physical_air() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 35.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let scenario = r#"{"ahu_damper_sensor_events":[{"ahu":1,"damper":"outdoor","bias":0.8,"start_hour":8,"duration_hours":1}]}"#;
    let mut biased = Simulation::new_test(graph, &weather, scenario).unwrap();
    let mut dynamic_biased = Simulation::new_test(graph, &weather, scenario).unwrap();
    dynamic_biased.oracle = Some(cdl_runtime_oracle::Executor::default());
    let alarm = "FourPipeG36Controls|output.ahu_1_outdoor_damper_tracking_alarm_level";
    for sim in [&mut normal, &mut biased, &mut dynamic_biased] {
        sim.temps = [28.0; 10];
        sim.cursor = 32 * 900 / sim.effective_step_seconds;
        sim.set_traces_json(&json!([alarm]).to_string()).unwrap();
    }
    let normal_rows = run_weather_intervals(&mut normal, 4);
    let biased_rows = run_weather_intervals(&mut biased, 4);
    let dynamic_rows = run_weather_intervals(&mut dynamic_biased, 4);
    let ordinary = &normal_rows["rows"][3];
    let fault = &biased_rows["rows"][3];
    let dynamic_fault = &dynamic_rows["rows"][3];
    assert_eq!(fault["traces"][alarm], json!(3.0));
    for field in [
        "ahu_1_actual_outdoor_damper_position",
        "ahu_1_measured_outdoor_damper_position",
        "ahu_1_outdoor_air_fraction",
        "ahu_1_mixed_air_c",
    ] {
        assert_eq!(fault[field], dynamic_fault[field], "{field}");
    }
    assert_eq!(fault["traces"][alarm], dynamic_fault["traces"][alarm]);
    assert_eq!(
        ordinary["ahu_1_actual_outdoor_damper_position"],
        fault["ahu_1_actual_outdoor_damper_position"]
    );
    assert_eq!(
        ordinary["ahu_1_outdoor_air_fraction"],
        fault["ahu_1_outdoor_air_fraction"]
    );
    assert_eq!(ordinary["ahu_1_mixed_air_c"], fault["ahu_1_mixed_air_c"]);
    assert!(
        fault["ahu_1_measured_outdoor_damper_position"]
            .as_f64()
            .unwrap()
            > fault["ahu_1_actual_outdoor_damper_position"]
                .as_f64()
                .unwrap()
    );
}

#[test]
fn g36_ahu_freeze_stage_one_uses_timed_supply_feedback_and_cdl_overrides() {
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
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(18.0)),
        ("flow_request".into(), json!(5.0)),
        ("return_air_temp".into(), json!(24.0)),
        ("mixed_air_temp".into(), json!(0.0)),
        ("actual_supply_air_temp".into(), json!(3.5)),
        ("duct_pressure".into(), json!(0.4)),
        ("outdoor_airflow".into(), json!(0.0)),
        ("freeze_stat_trip".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
        ("freeze_reset".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    for step in 0..6 {
        let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(result["freeze_stage_one"], json!(step == 5));
        assert_eq!(
            result["freeze_stage_one"].as_bool().unwrap(),
            fast["freeze_stage_one"].as_bool().unwrap()
        );
        assert_eq!(
            result["freeze_hot_water_request"].as_i64().unwrap() as f64,
            fast["freeze_hot_water_request"].as_f64().unwrap()
        );
        if step == 5 {
            assert_eq!(result["freeze_hot_water_request"], json!(2));
            assert_eq!(result["outdoor_air_fraction"], json!(0.2));
            assert!(result["heating_coil_valve_position"].as_f64().unwrap() > 0.0);
        }
    }
    inputs.insert("actual_supply_air_temp".into(), json!(8.0));
    for step in 0..6 {
        let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(result["freeze_stage_one"], json!(step < 5));
        assert_eq!(
            result["freeze_stage_one"].as_bool().unwrap(),
            fast["freeze_stage_one"].as_bool().unwrap()
        );
        assert_eq!(result["outdoor_air_fraction"], fast["outdoor_air_fraction"]);
    }
    let mut edited = ahu.clone();
    edited
        .blocks
        .iter_mut()
        .find(|block| block.name == "freeze_stage_one_on")
        .unwrap()
        .parameters[0]
        .value = json!(2.0);
    inputs.insert("actual_supply_air_temp".into(), json!(3.5));
    let mut tuned = Executor::default();
    for _ in 0..6 {
        assert_eq!(
            tuned.execute(&edited, &inputs, 60.0, "").unwrap()["freeze_stage_one"],
            json!(false)
        );
    }
}

#[test]
fn g36_ahu_freeze_stage_two_interrupts_ventilation_for_one_hour() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    assert!(validate(&building).is_empty(), "{:?}", validate(&building));
    let ahu = building
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1")
        .unwrap();
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(18.0)),
        ("flow_request".into(), json!(5.0)),
        ("return_air_temp".into(), json!(24.0)),
        ("mixed_air_temp".into(), json!(20.0)),
        ("actual_supply_air_temp".into(), json!(3.0)),
        ("duct_pressure".into(), json!(0.4)),
        ("outdoor_airflow".into(), json!(0.0)),
        ("freeze_stat_trip".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
        ("freeze_reset".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    for step in 0..6 {
        let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(result["freeze_stage_two"], json!(step == 5));
        assert_eq!(
            result["freeze_stage_two"].as_bool().unwrap(),
            fast["freeze_stage_two"].as_bool().unwrap()
        );
        if step == 5 {
            assert_eq!(result["outdoor_air_fraction"], json!(0.0));
            assert_eq!(result["freeze_ventilation_alarm_level"], json!(3));
        }
    }
    inputs.insert("actual_supply_air_temp".into(), json!(4.0));
    let mut released_at = None;
    for step in 1..=65 {
        let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(result["outdoor_air_fraction"], fast["outdoor_air_fraction"]);
        if result["freeze_stage_two"] == json!(false) {
            released_at.get_or_insert(step);
        } else {
            assert_eq!(result["outdoor_air_fraction"], json!(0.0));
            assert_eq!(result["freeze_ventilation_alarm_level"], json!(3));
        }
    }
    assert_eq!(released_at, Some(60));
    let recovered = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
    assert_eq!(recovered["freeze_stage_one"], json!(true));
    assert_eq!(recovered["outdoor_air_fraction"], json!(0.2));
    assert_eq!(recovered["freeze_ventilation_alarm_level"], json!(0));
    let mut tuned = ahu.clone();
    tuned
        .blocks
        .iter_mut()
        .find(|block| block.name == "freeze_stage_two_on")
        .unwrap()
        .parameters[0]
        .value = json!(2.0);
    let mut executor = Executor::default();
    inputs.insert("actual_supply_air_temp".into(), json!(3.0));
    for _ in 0..6 {
        assert_eq!(
            executor.execute(&tuned, &inputs, 60.0, "").unwrap()["freeze_stage_two"],
            json!(false)
        );
    }
    let mut fine = Executor::default();
    for _ in 0..=20 {
        fine.execute(ahu, &inputs, 15.0, "").unwrap();
    }
    inputs.insert("actual_supply_air_temp".into(), json!(4.0));
    let mut fine_release = None;
    for step in 1..=244 {
        if fine.execute(ahu, &inputs, 15.0, "").unwrap()["freeze_stage_two"] == json!(false) {
            fine_release.get_or_insert(step);
        }
    }
    assert_eq!(fine_release, Some(240));
}

#[test]
fn g36_ahu_freeze_stage_three_latches_shutdown_until_operator_reset() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    assert!(validate(&building).is_empty(), "{:?}", validate(&building));
    let ahu = building
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1")
        .unwrap();
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(18.0)),
        ("flow_request".into(), json!(5.0)),
        ("return_air_temp".into(), json!(24.0)),
        ("mixed_air_temp".into(), json!(20.0)),
        ("actual_supply_air_temp".into(), json!(8.0)),
        ("duct_pressure".into(), json!(0.4)),
        ("outdoor_airflow".into(), json!(0.0)),
        ("freeze_stat_trip".into(), json!(true)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
        ("freeze_reset".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
    let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(result["freeze_stage_three"], json!(true));
    assert_eq!(fast["freeze_stage_three"], json!(true));
    assert_eq!(result["fan_speed"], json!(0.0));
    assert_eq!(result["outdoor_air_fraction"], json!(0.0));
    assert_eq!(result["cooling_coil_valve_position"], json!(1.0));
    assert!(result["heating_coil_valve_position"].as_f64().unwrap() > 0.0);
    assert_eq!(result["freeze_hot_water_request"], json!(2));
    assert_eq!(result["freeze_chilled_water_request"], json!(true));
    assert_eq!(result["freeze_alarm_level"], json!(2));
    inputs.insert("freeze_reset".into(), json!(true));
    assert_eq!(
        dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
        json!(false)
    );
    assert_eq!(
        compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
        json!(false)
    );
    inputs.insert("freeze_reset".into(), json!(false));
    assert_eq!(
        dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
        json!(false)
    );
    assert_eq!(
        compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
        json!(false)
    );
    inputs.insert("freeze_stat_trip".into(), json!(false));
    for _ in 0..3 {
        assert_eq!(
            dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
            json!(false)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
            json!(false)
        );
    }
    inputs.insert("freeze_reset".into(), json!(true));
    assert_eq!(
        dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
        json!(false)
    );
    assert_eq!(
        compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
        json!(false)
    );
    inputs.insert("freeze_reset".into(), json!(false));
    inputs.insert("freeze_stat_trip".into(), json!(true));
    assert_eq!(
        dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
        json!(true)
    );
    assert_eq!(
        compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
        json!(true)
    );
}

#[test]
fn g36_ahu_freeze_stage_three_uses_both_timed_low_temperature_triggers() {
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
    for (temperature, trigger_step) in [(3.0, 16), (0.5, 6)] {
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(18.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(20.0)),
            ("actual_supply_air_temp".into(), json!(temperature)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        for step in 1..=trigger_step {
            let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(result["freeze_stage_three"], json!(step == trigger_step));
            assert_eq!(fast["freeze_stage_three"], json!(step == trigger_step));
        }
        inputs.insert("freeze_reset".into(), json!(true));
        assert_eq!(
            dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
            json!(false)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
            json!(false)
        );
        inputs.insert("freeze_reset".into(), json!(false));
        assert_eq!(
            dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
            json!(false)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
            json!(false)
        );
        inputs.insert("actual_supply_air_temp".into(), json!(8.0));
        assert_eq!(
            dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
            json!(false)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
            json!(false)
        );
    }
}

#[test]
fn g36_ahu_freeze_stages_replay_pinned_obc_validation_trace() {
    // Pinned Dymola validation: 6 -> 2 °C supply-air ramp over one hour,
    // with a freeze-reset contact after 3420 s. Compare stage priority,
    // alarm priority, and hot-water requests at all 101 archived samples.
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_ahu_freeze_dymola.txt"
    ));
    let series = |name: &str| -> Vec<f64> {
        let prefix = format!("{name}=[");
        let line = archive
            .lines()
            .find(|line| line.starts_with(&prefix))
            .unwrap();
        line[prefix.len()..]
            .trim_end_matches(']')
            .split(',')
            .map(|value| value.trim().parse().unwrap())
            .collect()
    };
    let expected_stage = series("frePro.yFreProSta");
    let expected_alarm = series("frePro.yAla");
    let expected_hot_request = series("frePro.yHotWatPlaReq");
    assert_eq!(expected_stage.len(), 101);
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
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    let mut observed = Vec::new();
    for step in 0..240 {
        let time = step as f64 * 15.0;
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
        for (name, value) in [
            ("outdoor_c", 18.0),
            ("flow_request", 5.0),
            ("actual_supply_air_temp", 6.0 - 4.0 * time / 3600.0),
            ("mixed_air_temp", 8.0 - 5.0 * time / 3600.0),
            ("return_air_temp", 24.0),
            ("duct_pressure", 0.4),
            ("outdoor_airflow", 0.3),
            ("actual_fan_speed", 0.5 + 0.2 * time / 3600.0),
            ("actual_outdoor_damper_position", 0.1 + 0.5 * time / 3600.0),
            ("actual_return_damper_position", 0.7 + 0.2 * time / 3600.0),
            ("outdoor_air_enthalpy_kj_kg", 40.0),
            ("return_air_enthalpy_kj_kg", 45.0),
        ] {
            inputs.insert(name.into(), json!(value));
        }
        inputs.insert("freeze_reset".into(), json!(time >= 3420.0));
        inputs.insert("supply_fan_proven".into(), json!(true));
        inputs.insert("occupied_mode".into(), json!(true));
        inputs.insert("operation_mode".into(), json!(1));
        let slow = dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
        let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
        for name in [
            "freeze_stage_one",
            "freeze_stage_two",
            "freeze_stage_three",
            "freeze_ventilation_alarm_level",
            "freeze_alarm_level",
            "freeze_hot_water_request",
        ] {
            assert_eq!(slow[name], fast[name], "{name} at {time} s");
        }
        let stage = if slow["freeze_stage_three"] == json!(true) {
            3.0
        } else if slow["freeze_stage_two"] == json!(true) {
            2.0
        } else if slow["freeze_stage_one"] == json!(true) {
            1.0
        } else {
            0.0
        };
        let alarm = if stage == 3.0 {
            slow["freeze_alarm_level"].as_f64().unwrap()
        } else {
            slow["freeze_ventilation_alarm_level"].as_f64().unwrap()
        };
        let hot_request = slow["freeze_hot_water_request"].as_f64().unwrap();
        observed.push((time + 15.0, stage, alarm, hot_request));
    }
    for sample in 0..101 {
        let time = sample as f64 * 36.0;
        let index = ((time / 15.0).round() as usize)
            .saturating_sub(1)
            .min(observed.len() - 1);
        let (_, stage, alarm, hot_request) = observed[index];
        let near_event = [1764.0, 2736.0, 3348.0, 3456.0]
            .iter()
            .any(|event| (time - event).abs() <= 36.0);
        if !near_event {
            assert_eq!(stage, expected_stage[sample], "stage at {time} s");
            assert_eq!(alarm, expected_alarm[sample], "alarm at {time} s");
            assert_eq!(
                hot_request, expected_hot_request[sample],
                "hot-water request at {time} s"
            );
        }
    }
}

#[test]
fn g36_simulation_injects_raw_freeze_stat_and_operator_reset() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 18.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut sim = Simulation::new_test(graph, &weather,
        r#"{"freeze_test_ahu":1,"freeze_test_start_hour":0,"freeze_test_duration_hours":0.25,"freeze_test_reset_hour":0.5}"#).unwrap();
    let first = "FourPipeG36Controls|output.ahu_1_freeze_stage_three";
    let second = "FourPipeG36Controls|output.ahu_2_freeze_stage_three";
    sim.set_traces_json(&json!([first, second]).to_string())
        .unwrap();
    let result = run_weather_intervals(&mut sim, 3);
    let rows = result["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["traces"][first], json!(1.0));
    assert_eq!(rows[0]["traces"][second], json!(0.0));
    assert_eq!(rows[0]["chilled_water_enable"], json!(1.0));
    assert_eq!(rows[1]["traces"][first], json!(1.0));
    assert_eq!(rows[2]["traces"][first], json!(0.0));
}

#[test]
fn g36_pressure_trim_waits_for_startup_and_one_sample_in_editable_cdl() {
    let graph = g36_pressure_reset_branch();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    for tick in 0..=16 {
        let inputs = HashMap::from([
            ("request_count".into(), json!(5)),
            ("fan_requested".into(), json!(true)),
        ]);
        let ordinary = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        let value = ordinary["setpoint"].as_f64().unwrap();
        assert!(
            (value - fast["setpoint"].as_f64().unwrap()).abs() < 1e-9,
            "executors disagree at tick {tick}: dynamic {value}, compiled {}",
            fast["setpoint"]
        );
        if tick * 60 < 720 {
            assert!((value - 0.35).abs() < 1e-9, "tick {tick}: {value}");
        }
        if tick == 16 {
            assert!(value > 0.35, "pressure reset never started: {value}");
        }
    }
}

#[test]
fn g36_pressure_reset_replays_pinned_obc_supply_fan_requests() {
    let mut graph = g36_pressure_reset_branch();
    for (name, parameter, value) in [
        ("initial_pressure_target", "k", 0.24),
        ("low_pressure_target", "k", 0.05),
        ("high_pressure_target", "k", 0.8),
        ("pressure_previous", "y_start", 0.24),
        ("trim_amount", "k", -0.024),
        ("response_per_request", "k", 0.03),
        ("maximum_response", "k", 0.064),
    ] {
        graph
            .blocks
            .iter_mut()
            .find(|block| block.name == name)
            .unwrap()
            .parameters
            .iter_mut()
            .find(|item| item.name == parameter)
            .unwrap()
            .value = json!(value);
    }
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_supply_fan_dymola.txt"
    ));
    let expected: Vec<f64> = archive
        .lines()
        .find(|line| line.starts_with("conSupFan.staPreSetRes.y="))
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .trim_matches(['[', ']'])
        .split(',')
        .map(|field| field.trim().parse::<f64>().unwrap() / 500.0)
        .collect();
    assert_eq!(expected.len(), 101);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
    let mut prior_value = None;
    for tick in 0..=28_800 {
        let seconds = tick as f64;
        let phase = std::f64::consts::TAU * seconds / 14_400.0;
        let count = (3.0 + 2.0 * phase.sin()).abs().round() as i64;
        let mode = (6.0 * seconds / 28_800.0).abs().round() as i64;
        let inputs = HashMap::from([
            ("request_count".into(), json!(count)),
            ("fan_requested".into(), json!((1..=3).contains(&mode))),
        ]);
        let ordinary = dynamic.execute(&graph, &inputs, 1.0, "").unwrap();
        let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
        let value = ordinary["setpoint"].as_f64().unwrap();
        assert!((value - fast["setpoint"].as_f64().unwrap()).abs() < 1e-9);
        if tick % 288 == 0 {
            let index = tick / 288;
            // The Dymola archive records the left limit when its 288 s
            // observation lands on the 120 s reset event. At other
            // observations, compare the settled value at that instant.
            let observed = if tick > 0 && tick % 120 == 0 {
                prior_value.unwrap_or(value)
            } else {
                value
            };
            let difference = (observed - expected[index]).abs();
            assert!(
                difference < 1e-9,
                "OBC pressure reset sample {index} at {tick}s: runtime {} Pa, archived {} Pa",
                observed * 500.0,
                expected[index] * 500.0
            );
            if index == 11 {
                assert!(difference < 1e-9, "first OBC pressure response: {value}");
            }
        }
        prior_value = Some(value);
    }
}

#[test]
fn g36_normal_coil_commands_require_fan_proof_in_editable_cdl() {
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
    for mixed in [0.0, 30.0] {
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(0.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(mixed)),
            ("actual_supply_air_temp".into(), json!(16.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(false)),
            ("actual_fan_speed".into(), json!(0.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        let normal = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for key in ["heating_coil_valve_position", "cooling_coil_valve_position"] {
            assert_eq!(normal[key], json!(0.0), "mixed {mixed}, {key}");
            assert_eq!(fast[key], normal[key], "mixed {mixed}, {key}");
        }
        inputs.insert("supply_fan_proven".into(), json!(true));
        let normal = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        let active = if mixed < 10.0 {
            "heating_coil_valve_position"
        } else {
            "cooling_coil_valve_position"
        };
        assert!(
            normal[active].as_f64().unwrap() > 0.0,
            "mixed {mixed}, {active}"
        );
        assert_eq!(fast[active], normal[active]);
    }
}
