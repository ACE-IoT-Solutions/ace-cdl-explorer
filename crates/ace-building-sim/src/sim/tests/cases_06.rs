use super::*;

#[test]
fn return_enthalpy_conserves_the_flow_weighted_zone_streams() {
    let temperatures = [10.0, 30.0, 20.0, 20.0, 20.0, 22.0, 22.0, 22.0, 22.0, 22.0];
    let humidity = [0.002, 0.02, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01];
    let flow = [0.1, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let expected =
        0.1 * moist_air_enthalpy_kj_kg(10.0, 0.002) + 0.9 * moist_air_enthalpy_kj_kg(30.0, 0.02);
    let actual = ahu_return_enthalpy_kj_kg(&temperatures, &humidity, &flow, 0);
    assert!((actual - expected).abs() < 1e-12);
    let averaged = moist_air_enthalpy_kj_kg(
        ahu_return_value(&temperatures, &flow, 0),
        ahu_return_value(&humidity, &flow, 0),
    );
    assert!((actual - averaged).abs() > 0.02);

    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        json!({"outdoor_c":24.0,"relative_humidity":0.5,"solar_kw":0.0,
            "internal_kw":0.0,"occupied":true});
        35_040
    ])
    .unwrap();
    let mut sim = Simulation::new_test(graph, &weather, "{}").unwrap();
    sim.cursor = 40 * 60 + 59;
    sim.temps = temperatures;
    sim.zone_humidity_ratio = humidity;
    sim.last_zone_airflow = flow;
    let trace = "FourPipeG36Controls|ahu_1_return_air_enthalpy_kj_kg";
    sim.set_traces_json(&json!([trace]).to_string()).unwrap();
    let result: Value = serde_json::from_str(&sim.step_chunk_json(1).unwrap()).unwrap();
    let measured = result["rows"][0]["traces"][trace].as_f64().unwrap();
    assert!((measured - expected).abs() < 1e-9);
}

#[test]
fn physical_occupants_add_zone_moisture_without_a_cdl_humidity_decision() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = serde_json::to_string(&vec![
        json!({"outdoor_c": 22.0, "relative_humidity": 0.5,
            "solar_kw": 0.0, "internal_kw": 0.0, "occupied": true});
        35_040
    ])
    .unwrap();
    let mut humidity = Vec::new();
    for source in [0.0, 0.06] {
        let scenario = json!({
            "occupant_moisture_kg_h_per_person": source,
            "occupant_count_events": [{"zone": 1, "count": 20.0,
                "start_hour": 0.0, "duration_hours": 1.0}]
        });
        let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
        let rows: Value = serde_json::from_str(&sim.step_chunk_json(1).unwrap()).unwrap();
        humidity.push(
            rows["rows"][0]["zone_relative_humidity"][0]
                .as_f64()
                .unwrap(),
        );
    }
    assert!(
        humidity[1] > humidity[0] + 0.001,
        "physical occupant moisture must change zone RH: {humidity:?}"
    );
}

#[test]
fn g36_optional_low_temperature_guard_is_an_editable_cdl_choice() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut unrestricted = building
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1")
        .unwrap()
        .clone();
    for parameter in &mut unrestricted
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_high_limit_hold")
        .unwrap()
        .parameters
    {
        parameter.value = json!(0.0);
    }
    let mut guarded = unrestricted.clone();
    guarded
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_use_teaching_low_limit")
        .unwrap()
        .parameters[0]
        .value = json!(true);
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(5.0)),
        ("return_air_temp".into(), json!(24.0)),
        ("flow_request".into(), json!(5.0)),
        ("mixed_air_temp".into(), json!(20.0)),
        ("actual_supply_air_temp".into(), json!(16.0)),
        ("duct_pressure".into(), json!(0.4)),
        ("outdoor_airflow".into(), json!(0.3)),
        ("freeze_stat_trip".into(), json!(false)),
        ("freeze_reset".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    for (graph, expected) in [(&unrestricted, true), (&guarded, false)] {
        let slow = Executor::default()
            .execute(graph, &inputs, 15.0, "")
            .unwrap();
        let fast = CompiledExecutor::try_new(graph)
            .unwrap()
            .unwrap()
            .execute(&inputs, 15.0, false)
            .unwrap()
            .0;
        assert_eq!(slow["economizer_mode_enabled"], json!(expected));
        assert_eq!(fast["economizer_mode_enabled"], json!(expected));
    }
    // The PI is deselected without fan proof. It must discard any stored
    // command on the next enable edge, including after a long warm return.
    assert_eq!(
        unrestricted
            .blocks
            .iter()
            .find(|block| block.name == "economizer_mixed_air_pi")
            .unwrap()
            .block_type,
        "PIDWithReset"
    );
    assert!(unrestricted
        .connections
        .iter()
        .any(|edge| edge.from_path == "economizer_enabled.y"
            && edge.to_path == "economizer_mixed_air_pi.trigger"));
    let mut slow = Executor::default();
    let mut fast = CompiledExecutor::try_new(&unrestricted).unwrap().unwrap();
    inputs.insert("supply_fan_proven".into(), json!(false));
    inputs.insert("mixed_air_temp".into(), json!(25.0));
    for _ in 0..240 {
        slow.execute(&unrestricted, &inputs, 15.0, "").unwrap();
        fast.execute(&inputs, 15.0, false).unwrap();
    }
    inputs.insert("supply_fan_proven".into(), json!(true));
    inputs.insert("mixed_air_temp".into(), json!(20.0));
    let after_enable = slow.execute(&unrestricted, &inputs, 15.0, "").unwrap();
    let compiled_after_enable = fast.execute(&inputs, 15.0, false).unwrap().0;
    assert!(
        after_enable["outdoor_air_damper_position"]
            .as_f64()
            .unwrap()
            < 0.5
    );
    assert_eq!(
        after_enable["outdoor_air_damper_position"],
        compiled_after_enable["outdoor_air_damper_position"]
    );
}

#[test]
fn g36_economizer_enable_is_not_gated_by_teaching_cooling_demand() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut default = building
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1")
        .unwrap()
        .clone();
    for parameter in &mut default
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_high_limit_hold")
        .unwrap()
        .parameters
    {
        parameter.value = json!(0.0);
    }
    let mut teaching = default.clone();
    teaching
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_use_teaching_demand_gate")
        .unwrap()
        .parameters[0]
        .value = json!(true);
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(5.0)),
        ("return_air_temp".into(), json!(16.0)),
        ("flow_request".into(), json!(5.0)),
        ("mixed_air_temp".into(), json!(16.0)),
        ("actual_supply_air_temp".into(), json!(16.0)),
        ("duct_pressure".into(), json!(0.4)),
        ("outdoor_airflow".into(), json!(0.3)),
        ("freeze_stat_trip".into(), json!(false)),
        ("freeze_reset".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    for (graph, enabled) in [(&default, true), (&teaching, false)] {
        let slow = Executor::default()
            .execute(graph, &inputs, 15.0, "")
            .unwrap();
        let fast = CompiledExecutor::try_new(graph)
            .unwrap()
            .unwrap()
            .execute(&inputs, 15.0, false)
            .unwrap()
            .0;
        assert_eq!(slow["economizer_mode_enabled"], json!(enabled));
        assert_eq!(fast["economizer_mode_enabled"], json!(enabled));
    }
}

#[test]
fn g36_economizer_disable_damper_timers_are_compiled_cdl() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut ahu = building
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1")
        .unwrap()
        .clone();
    let hold = ahu
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_high_limit_hold")
        .unwrap();
    for parameter in &mut hold.parameters {
        parameter.value = json!(0.0);
    }
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(22.0)),
        ("flow_request".into(), json!(5.0)),
        ("return_air_temp".into(), json!(24.0)),
        ("mixed_air_temp".into(), json!(20.0)),
        ("actual_supply_air_temp".into(), json!(16.0)),
        ("duct_pressure".into(), json!(0.4)),
        ("outdoor_airflow".into(), json!(0.3)),
        ("freeze_stat_trip".into(), json!(false)),
        ("freeze_reset".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&ahu).unwrap().unwrap();
    let mut no_outdoor_delay = ahu.clone();
    no_outdoor_delay
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_outdoor_close_delay")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "delayTime")
        .unwrap()
        .value = json!(0.0);
    let mut immediate = Executor::default();
    for _ in 0..60 {
        let actual = dynamic.execute(&ahu, &inputs, 15.0, "").unwrap();
        assert_eq!(actual["economizer_mode_enabled"], json!(true));
        compiled.execute(&inputs, 15.0, false).unwrap();
        immediate
            .execute(&no_outdoor_delay, &inputs, 15.0, "")
            .unwrap();
    }
    inputs.insert("outdoor_c".into(), json!(24.1));
    for step in 0..14 {
        let actual = dynamic.execute(&ahu, &inputs, 15.0, "").unwrap();
        let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
        let without_delay = immediate
            .execute(&no_outdoor_delay, &inputs, 15.0, "")
            .unwrap();
        assert_eq!(actual["economizer_mode_enabled"], json!(false));
        if step == 0 {
            assert!(
                actual["outdoor_air_damper_position"].as_f64().unwrap()
                    > without_delay["outdoor_air_damper_position"]
                        .as_f64()
                        .unwrap()
            );
        } else {
            assert_eq!(
                actual["outdoor_air_damper_position"],
                without_delay["outdoor_air_damper_position"]
            );
        }
        for signal in ["outdoor_air_damper_position", "return_air_damper_position"] {
            let slow = actual[signal].as_f64().unwrap();
            assert!(
                (slow - fast[signal].as_f64().unwrap()).abs() < 1e-9,
                "{signal}, step {step}"
            );
        }
        if step < 12 {
            assert_eq!(actual["return_air_damper_position"], json!(1.0));
        }
        if step == 12 {
            assert!(actual["return_air_damper_position"].as_f64().unwrap() < 1.0);
        }
    }
}

#[test]
fn short_physical_outdoor_heat_pulse_exercises_editable_economizer_hold() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let mut immediate: Value = serde_json::from_str(graph).unwrap();
    for ahu in immediate["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|block| block["block_type"] == "AHUController")
    {
        let hold = ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "economizer_high_limit_hold")
            .unwrap();
        for parameter in hold["parameters"].as_array_mut().unwrap() {
            parameter["value"] = json!(0.0);
        }
    }
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 22.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let scenario = r#"{"step_seconds":60,"outdoor_temperature_events":[{"delta_c":10,"start_hour":0.05,"duration_hours":0.05}]}"#;
    let mut held = Simulation::new_test(graph, &weather, scenario).unwrap();
    let mut direct = Simulation::new_test(&immediate.to_string(), &weather, scenario).unwrap();
    held.temps = [29.0; 10];
    direct.temps = [29.0; 10];
    let held_rows = run_weather_intervals(&mut held, 2);
    let direct_rows = run_weather_intervals(&mut direct, 2);
    let held_row = &held_rows["rows"][1];
    let direct_row = &direct_rows["rows"][1];
    assert!(
        (held_row["zone_c"][0].as_f64().unwrap() - direct_row["zone_c"][0].as_f64().unwrap()).abs()
            > 1e-5
    );
    assert!((held.electricity_kwh - direct.electricity_kwh).abs() > 1e-5);
}

#[test]
fn g36_minimum_outdoor_air_requires_occupied_mode_and_fan_proof() {
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
        ("actual_supply_air_temp".into(), json!(16.0)),
        ("duct_pressure".into(), json!(0.4)),
        ("outdoor_airflow".into(), json!(0.0)),
        ("freeze_stat_trip".into(), json!(false)),
        ("freeze_reset".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    let off = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
    let fast_off = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(off["minimum_outdoor_air_enabled"], json!(false));
    assert_eq!(off["minimum_outdoor_airflow"], json!(0.0));
    // Free cooling may still open the damper with the minimum-air loop off.
    assert_eq!(
        off["outdoor_air_fraction"],
        fast_off["outdoor_air_fraction"]
    );
    inputs.insert("flow_request".into(), json!(0.0));
    assert_eq!(
        Executor::default().execute(ahu, &inputs, 60.0, "").unwrap()["outdoor_air_fraction"],
        json!(0.0)
    );
    inputs.insert("flow_request".into(), json!(5.0));
    inputs.insert("occupied_mode".into(), json!(true));
    let on = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
    let fast_on = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(on["minimum_outdoor_air_enabled"], json!(true));
    assert!((on["minimum_outdoor_airflow"].as_f64().unwrap() - 0.45).abs() < 1e-9);
    assert_eq!(on["outdoor_air_fraction"], fast_on["outdoor_air_fraction"]);
    inputs.insert("supply_fan_proven".into(), json!(false));
    let unproven = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
    assert_eq!(unproven["minimum_outdoor_air_enabled"], json!(false));
    assert_eq!(unproven["minimum_outdoor_airflow"], json!(0.0));
    let mut limited = ahu.clone();
    limited
        .blocks
        .iter_mut()
        .find(|block| block.name == "outdoor_damper_physical_max")
        .unwrap()
        .parameters[0]
        .value = json!(0.1);
    limited
        .blocks
        .iter_mut()
        .find(|block| block.name == "minimum_oa_pi")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "y_reset")
        .unwrap()
        .value = json!(0.5);
    inputs.insert("supply_fan_proven".into(), json!(true));
    let mut capped_runtime = Executor::default();
    let mut fan_off_inputs = inputs.clone();
    fan_off_inputs.insert("supply_fan_proven".into(), json!(false));
    capped_runtime
        .execute(&limited, &fan_off_inputs, 0.0, "")
        .unwrap();
    let capped = capped_runtime.execute(&limited, &inputs, 60.0, "").unwrap();
    assert_eq!(capped["outdoor_air_fraction"], json!(0.1));
    let mut return_limited = ahu.clone();
    return_limited
        .blocks
        .iter_mut()
        .find(|block| block.name == "outdoor_damper_physical_max")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "k")
        .unwrap()
        .value = json!(0.2);
    return_limited
        .blocks
        .iter_mut()
        .find(|block| block.name == "minimum_oa_pi")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "y_reset")
        .unwrap()
        .value = json!(0.8);
    inputs.insert("outdoor_c".into(), json!(25.0));
    inputs.insert("occupied_mode".into(), json!(false));
    let mut return_runtime = Executor::default();
    let return_off = return_runtime
        .execute(&return_limited, &inputs, 60.0, "")
        .unwrap();
    inputs.insert("occupied_mode".into(), json!(true));
    inputs.insert("outdoor_airflow".into(), json!(0.0));
    inputs.insert("supply_fan_proven".into(), json!(false));
    return_runtime
        .execute(&return_limited, &inputs, 0.0, "")
        .unwrap();
    inputs.insert("supply_fan_proven".into(), json!(true));
    let return_on = return_runtime
        .execute(&return_limited, &inputs, 60.0, "")
        .unwrap();
    assert_eq!(return_off["outdoor_air_damper_position"], json!(0.2));
    assert_eq!(return_on["outdoor_air_damper_position"], json!(0.2));
    assert!(
        return_on["return_air_damper_position"].as_f64().unwrap()
            < return_off["return_air_damper_position"].as_f64().unwrap()
    );
    inputs.insert("zone_01_measured_airflow".into(), json!(0.1));
    let constrained = Executor::default().execute(ahu, &inputs, 60.0, "").unwrap();
    let expected_efficiency = 1.0 + 0.45 / 1.7 - 0.9;
    assert!(
        (constrained["ventilation_efficiency"].as_f64().unwrap() - expected_efficiency).abs()
            < 1e-9
    );
    assert!((constrained["minimum_outdoor_airflow"].as_f64().unwrap() - 0.75).abs() < 1e-9);
    let mut raised_design = ahu.clone();
    raised_design
        .blocks
        .iter_mut()
        .find(|block| block.name == "vent_total_design")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "k")
        .unwrap()
        .value = json!(1.5);
    let raised = Executor::default()
        .execute(&raised_design, &inputs, 60.0, "")
        .unwrap();
    assert!(
        (raised["minimum_outdoor_airflow"].as_f64().unwrap() - 0.45 / expected_efficiency).abs()
            < 1e-9
    );
}

#[test]
fn g36_zone_ventilation_uses_confirmed_presence_and_editable_design_air() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 0.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let area = "FourPipeG36Controls|output.zone_01_ventilation_area";
    let population = "FourPipeG36Controls|output.zone_01_ventilation_population";
    let minimum = "FourPipeG36Controls|output.zone_01_ventilation_minimum_oa_flow";
    let ahu_air = "FourPipeG36Controls|output.ahu_1_ventilation_uncorrected";
    let ahu_setpoint = "FourPipeG36Controls|output.ahu_1_minimum_outdoor_airflow";
    let primary_minimum = "FourPipeG36Controls.zone_01|window_primary_minimum.y";
    let targets = json!([
        area,
        population,
        minimum,
        ahu_air,
        ahu_setpoint,
        primary_minimum
    ])
    .to_string();
    let scenario = r#"{"outdoor_co2_ppm":900}"#;
    let mut normal = Simulation::new_test(graph, &weather, scenario).unwrap();
    normal.set_traces_json(&targets).unwrap();
    let rows = run_weather_intervals(&mut normal, 35);
    assert_eq!(rows["rows"][30]["traces"][area], json!(0.0));
    assert_eq!(rows["rows"][30]["traces"][primary_minimum], json!(0.0));
    assert_eq!(rows["rows"][33]["traces"][area], json!(0.03));
    assert_eq!(rows["rows"][33]["traces"][population], json!(0.02));
    let baseline_minimum = rows["rows"][33]["traces"][minimum].as_f64().unwrap();
    assert!((0.05..=0.0625 + 1e-9).contains(&baseline_minimum));
    let mut low_co2 = Simulation::new_test(graph, &weather, "{}").unwrap();
    low_co2.set_traces_json(&targets).unwrap();
    let low_rows = run_weather_intervals(&mut low_co2, 35);
    assert_eq!(low_rows["rows"][33]["traces"][population], json!(0.0));
    let window_scenario = r#"{"outdoor_co2_ppm":900,"window_openings":[{"zone":1,"start_hour":8,"duration_hours":1}]}"#;
    let mut open_window = Simulation::new_test(graph, &weather, window_scenario).unwrap();
    open_window.set_traces_json(&targets).unwrap();
    let open_rows = run_weather_intervals(&mut open_window, 35);
    assert_eq!(open_rows["rows"][33]["traces"][area], json!(0.0));
    assert_eq!(open_rows["rows"][33]["traces"][population], json!(0.0));
    assert_eq!(open_rows["rows"][33]["traces"][primary_minimum], json!(0.0));
    let mut no_standby: Value = serde_json::from_str(graph).unwrap();
    no_standby["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "zone_01")
        .unwrap()["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ventilation_standby_permitted")
        .unwrap()["parameters"][0]["value"] = json!(false);
    let mut retained = Simulation::new_test(&no_standby.to_string(), &weather, scenario).unwrap();
    retained.set_traces_json(&targets).unwrap();
    let retained_rows = run_weather_intervals(&mut retained, 35);
    assert_eq!(retained_rows["rows"][30]["traces"][area], json!(0.03));
    assert_eq!(retained_rows["rows"][30]["traces"][population], json!(0.0));
    assert!(
        retained_rows["rows"][30]["traces"][primary_minimum]
            .as_f64()
            .unwrap()
            > 0.0
    );
    let total_flow_difference: f64 = (30..35)
        .map(|index| {
            let retained = retained_rows["rows"][index]["zone_airflow"][0]
                .as_f64()
                .unwrap();
            let standby = rows["rows"][index]["zone_airflow"][0].as_f64().unwrap();
            (retained - standby).abs()
        })
        .sum();
    assert!(
        total_flow_difference > 0.02,
        "CDL standby permission should change physical flow across five intervals: total difference={total_flow_difference}"
    );
    let mut retained_window =
        Simulation::new_test(&no_standby.to_string(), &weather, window_scenario).unwrap();
    retained_window.set_traces_json(&targets).unwrap();
    let retained_window_rows = run_weather_intervals(&mut retained_window, 35);
    assert_eq!(retained_window_rows["rows"][33]["traces"][area], json!(0.0));
    assert_eq!(
        retained_window_rows["rows"][33]["traces"][primary_minimum],
        json!(0.0)
    );
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    let zone = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "zone_01")
        .unwrap();
    zone["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ventilation_area_rate")
        .unwrap()["parameters"][0]["value"] = json!(0.0008);
    let mut tuned = Simulation::new_test(&edited.to_string(), &weather, scenario).unwrap();
    tuned.set_traces_json(&targets).unwrap();
    let tuned_rows = run_weather_intervals(&mut tuned, 37);
    assert_eq!(tuned_rows["rows"][33]["traces"][area], json!(0.08));
    assert!(tuned_rows["rows"][33]["traces"][minimum].as_f64().unwrap() > baseline_minimum);
    let person_rate = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "zone_01")
        .unwrap()["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ventilation_person_rate")
        .unwrap();
    person_rate["parameters"][0]["value"] = json!(0.02);
    let mut more_people_air =
        Simulation::new_test(&edited.to_string(), &weather, scenario).unwrap();
    more_people_air.set_traces_json(&targets).unwrap();
    let rate_rows = run_weather_intervals(&mut more_people_air, 37);
    assert_eq!(rate_rows["rows"][33]["traces"][population], json!(0.16));
    assert!(
        rate_rows["rows"][33]["traces"][ahu_air].as_f64().unwrap()
            > tuned_rows["rows"][33]["traces"][ahu_air].as_f64().unwrap()
    );
    assert!(
        rate_rows["rows"][36]["traces"][ahu_setpoint]
            .as_f64()
            .unwrap()
            > tuned_rows["rows"][36]["traces"][ahu_setpoint]
                .as_f64()
                .unwrap()
    );
    let greatest_flow_change = (0..37)
        .map(|index| {
            (rate_rows["rows"][index]["ahu_1_outdoor_airflow"]
                .as_f64()
                .unwrap()
                - tuned_rows["rows"][index]["ahu_1_outdoor_airflow"]
                    .as_f64()
                    .unwrap())
            .abs()
        })
        .fold(0.0_f64, f64::max);
    assert!(
        greatest_flow_change > 0.01,
        "edited rate should change physical outdoor airflow; maximum change {greatest_flow_change}"
    );
}

#[test]
fn g36_common_damper_maps_pi_halves_to_separate_actuators() {
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
        ("outdoor_c".into(), json!(30.0)),
        ("flow_request".into(), json!(5.0)),
        ("return_air_temp".into(), json!(24.0)),
        ("mixed_air_temp".into(), json!(20.0)),
        ("actual_supply_air_temp".into(), json!(16.0)),
        ("duct_pressure".into(), json!(0.4)),
        ("outdoor_airflow".into(), json!(0.0)),
        ("freeze_stat_trip".into(), json!(false)),
        ("freeze_reset".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
    }
    let mut low = ahu.clone();
    for block_name in ["vent_uncorrected_design", "vent_total_design"] {
        low.blocks
            .iter_mut()
            .find(|block| block.name == block_name)
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(1.0);
    }
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_ventilation_area"), json!(0.2));
        inputs.insert(format!("zone_{zone:02}_ventilation_population"), json!(0.0));
        inputs.insert(
            format!("zone_{zone:02}_ventilation_minimum_oa_flow"),
            json!(0.2),
        );
    }
    low.blocks
        .iter_mut()
        .find(|block| block.name == "minimum_oa_pi")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "y_reset")
        .unwrap()
        .value = json!(0.2);
    let mut cold_start = inputs.clone();
    cold_start.insert("supply_fan_proven".into(), json!(false));
    let mut lower_runtime = Executor::default();
    lower_runtime.execute(&low, &cold_start, 0.0, "").unwrap();
    let lower = lower_runtime.execute(&low, &inputs, 60.0, "").unwrap();
    assert!((0.4..0.42).contains(&lower["outdoor_air_damper_position"].as_f64().unwrap()));
    assert!((lower["return_air_damper_position"].as_f64().unwrap() - 0.8).abs() < 1e-9);
    let mut high = low.clone();
    high.blocks
        .iter_mut()
        .find(|block| block.name == "minimum_oa_pi")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "y_reset")
        .unwrap()
        .value = json!(0.8);
    let mut upper_runtime = Executor::default();
    upper_runtime.execute(&high, &cold_start, 0.0, "").unwrap();
    let upper = upper_runtime.execute(&high, &inputs, 60.0, "").unwrap();
    assert!((upper["outdoor_air_damper_position"].as_f64().unwrap() - 1.0).abs() < 1e-9);
    assert!((0.35..0.4).contains(&upper["return_air_damper_position"].as_f64().unwrap()));
    let mut compiled = CompiledExecutor::try_new(&high).unwrap().unwrap();
    compiled.execute(&cold_start, 0.0, false).unwrap();
    let (fast, _) = compiled.execute(&inputs, 60.0, false).unwrap();
    assert_eq!(
        upper["outdoor_air_damper_position"],
        fast["outdoor_air_damper_position"]
    );
    assert_eq!(
        upper["return_air_damper_position"],
        fast["return_air_damper_position"]
    );
}

#[test]
fn g36_outdoor_damper_cap_changes_physical_air_mixing() {
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
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    start_present_window(&mut normal);
    let normal_rows = run_weather_intervals(&mut normal, 4);
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    let ahu = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ahu_1")
        .unwrap();
    ahu["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "outdoor_damper_physical_max")
        .unwrap()["parameters"][0]["value"] = json!(0.05);
    let mut limited = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    start_present_window(&mut limited);
    let limited_rows = run_weather_intervals(&mut limited, 4);
    let baseline = &normal_rows["rows"][3];
    let capped = &limited_rows["rows"][3];
    assert!(baseline["ahu_1_outdoor_air_fraction"].as_f64().unwrap() > 0.2);
    assert!(
        capped["ahu_1_intake_opening"].as_f64().unwrap()
            < baseline["ahu_1_intake_opening"].as_f64().unwrap()
    );
    assert!(
        capped["ahu_1_outdoor_airflow"].as_f64().unwrap()
            < baseline["ahu_1_outdoor_airflow"].as_f64().unwrap()
    );
}

#[test]
fn g36_return_damper_cap_changes_physical_air_mixing() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 30.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    let ahu = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ahu_1")
        .unwrap();
    ahu["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "outdoor_damper_physical_max")
        .unwrap()["parameters"][0]["value"] = json!(0.2);
    let mut normal = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    start_present_window(&mut normal);
    let normal_rows = run_weather_intervals(&mut normal, 4);
    let ahu = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ahu_1")
        .unwrap();
    ahu["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "return_damper_physical_max")
        .unwrap()["parameters"][0]["value"] = json!(0.05);
    let mut limited = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    start_present_window(&mut limited);
    let limited_rows = run_weather_intervals(&mut limited, 4);
    let baseline = &normal_rows["rows"][3];
    let capped = &limited_rows["rows"][3];
    assert!(
        capped["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
            > baseline["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
    );
    assert!(
        capped["ahu_1_intake_opening"].as_f64().unwrap()
            < baseline["ahu_1_intake_opening"].as_f64().unwrap()
    );
    assert!(
        capped["delivered_airflow"].as_f64().unwrap()
            < baseline["delivered_airflow"].as_f64().unwrap()
    );
    assert!(
        capped["ahu_1_mixed_air_c"].as_f64().unwrap()
            > baseline["ahu_1_mixed_air_c"].as_f64().unwrap()
    );
}

#[test]
fn closed_common_intake_stops_ahu_airflow() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 18.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    let ahu = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ahu_1")
        .unwrap();
    for name in ["outdoor_damper_physical_max", "return_damper_physical_max"] {
        ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == name)
            .unwrap()["parameters"][0]["value"] = json!(0.0);
    }
    let mut simulation = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    let rows = run_weather_intervals(&mut simulation, 4);
    let row = &rows["rows"][3];
    assert_eq!(row["ahu_1_intake_opening"], json!(0.0));
    assert_eq!(row["ahu_1_duct_pressure"], json!(0.0));
    assert_eq!(row["ahu_1_outdoor_airflow"], json!(0.0));
    assert!(row["zone_airflow"].as_array().unwrap()[..5]
        .iter()
        .all(|flow| flow.as_f64() == Some(0.0)));
}

#[test]
fn g36_economizer_uses_physical_fan_motor_status_as_proof() {
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
    let trace = "FourPipeG36Controls|output.ahu_1_economizer_mode_enabled";
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    start_present_window(&mut normal);
    normal.set_traces_json(&json!([trace]).to_string()).unwrap();
    let normal_rows = run_weather_intervals(&mut normal, 4);
    assert_eq!(normal_rows["rows"][3]["traces"][trace], json!(1.0));
    let mut disabled: Value = serde_json::from_str(graph).unwrap();
    let ahu = disabled["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ahu_1")
        .unwrap();
    let fan_edge = ahu["connections"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|edge| edge["to"] == "freeze_stage_three_fan.u2")
        .unwrap();
    fan_edge["from"] = json!("fan_off.y");
    let mut stopped = Simulation::new_test(&disabled.to_string(), &weather, "{}").unwrap();
    start_present_window(&mut stopped);
    stopped
        .set_traces_json(&json!([trace]).to_string())
        .unwrap();
    let stopped_rows = run_weather_intervals(&mut stopped, 4);
    assert_eq!(stopped_rows["rows"][3]["traces"][trace], json!(0.0));
    assert_eq!(stopped_rows["rows"][3]["ahu_1_outdoor_airflow"], json!(0.0));
}

#[test]
fn ahu_coil_heat_depends_on_editable_valve_command() {
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
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let normal_rows = run_weather_intervals(&mut normal, 4);
    let mut closed: Value = serde_json::from_str(graph).unwrap();
    for ahu in 1..=2 {
        let controller = closed["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == format!("ahu_{ahu}"))
            .unwrap();
        let edge = controller["connections"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|edge| edge["to"] == "heating_coil_valve_position")
            .unwrap();
        edge["from"] = json!("coil_zero.y");
    }
    let mut no_heat = Simulation::new_test(&closed.to_string(), &weather, "{}").unwrap();
    let closed_rows = run_weather_intervals(&mut no_heat, 4);
    let normal_row = &normal_rows["rows"][3];
    let closed_row = &closed_rows["rows"][3];
    assert!(
        normal_row["ahu_1_heating_coil_valve_position"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(closed_row["ahu_1_heating_coil_valve_position"], 0.0);
    assert!(
        normal_row["ahu_1_supply_actual_c"].as_f64().unwrap()
            > closed_row["ahu_1_supply_actual_c"].as_f64().unwrap()
    );
    assert!(
        normal_row["ahu_1_heating_coil_request_kw"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(closed_row["ahu_1_heating_coil_request_kw"], 0.0);
    assert_coil_air_balance(normal_row, 1);
    assert_coil_air_balance(closed_row, 1);
}

#[test]
fn ahu_coil_cooling_depends_on_editable_valve_command() {
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 32.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let normal_rows = run_weather_intervals(&mut normal, 4);
    let mut closed: Value = serde_json::from_str(graph).unwrap();
    for ahu in 1..=2 {
        let controller = closed["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == format!("ahu_{ahu}"))
            .unwrap();
        let edge = controller["connections"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|edge| edge["to"] == "cooling_coil_valve_position")
            .unwrap();
        edge["from"] = json!("coil_zero.y");
    }
    let mut no_cooling = Simulation::new_test(&closed.to_string(), &weather, "{}").unwrap();
    let closed_rows = run_weather_intervals(&mut no_cooling, 4);
    let normal_row = &normal_rows["rows"][3];
    let closed_row = &closed_rows["rows"][3];
    assert!(
        normal_row["ahu_1_cooling_coil_valve_position"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(closed_row["ahu_1_cooling_coil_valve_position"], 0.0);
    assert!(
        normal_row["ahu_1_supply_actual_c"].as_f64().unwrap()
            < closed_row["ahu_1_supply_actual_c"].as_f64().unwrap()
    );
    assert!(
        normal_row["ahu_1_cooling_coil_request_kw"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(closed_row["ahu_1_cooling_coil_request_kw"], 0.0);
    assert_coil_air_balance(normal_row, 1);
    assert_coil_air_balance(closed_row, 1);
}

#[test]
fn water_pump_commands_gate_delivered_heat_and_cooling() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    for (outdoor, output_name, delivered_name, flow_name) in [
        (
            -10.0,
            "hot_water_pump_speed",
            "heat_kw",
            "hot_water_flow_kg_s",
        ),
        (
            32.0,
            "chilled_water_pump_speed",
            "cool_kw",
            "chilled_water_flow_kg_s",
        ),
    ] {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: outdoor,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
        let normal_rows = run_weather_intervals(&mut normal, 4);
        let mut disabled: Value = serde_json::from_str(graph).unwrap();
        let plant = disabled["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "plant")
            .unwrap();
        let edge = plant["connections"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|edge| edge["to"] == output_name)
            .unwrap();
        edge["from"] = json!("pump_zero.y");
        let mut no_flow = Simulation::new_test(&disabled.to_string(), &weather, "{}").unwrap();
        let disabled_rows = run_weather_intervals(&mut no_flow, 4);
        let normal_row = &normal_rows["rows"][3];
        let disabled_row = &disabled_rows["rows"][3];
        assert!(normal_row[flow_name].as_f64().unwrap() > 0.0, "{flow_name}");
        assert!(
            normal_row[delivered_name].as_f64().unwrap() > 0.0,
            "{delivered_name}"
        );
        assert_eq!(disabled_row[flow_name], 0.0);
        assert_eq!(disabled_row[delivered_name], 0.0);
        assert_water_heat_balance(normal_row);
        assert_water_heat_balance(disabled_row);
    }
}

#[test]
fn editable_water_supply_setpoints_limit_coil_delivery() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    for (outdoor, constant_name, changed_setpoint, delivered_name, sensor_name) in [
        (
            -10.0,
            "hot_water_supply_setpoint",
            35.0,
            "heat_kw",
            "hot_water_supply_temp_c",
        ),
        (
            32.0,
            "chilled_water_supply_setpoint",
            17.0,
            "cool_kw",
            "chilled_water_supply_temp_c",
        ),
    ] {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: outdoor,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut default = Simulation::new_test(graph, &weather, "{}").unwrap();
        let baseline = run_weather_intervals(&mut default, 48);
        let mut changed: Value = serde_json::from_str(graph).unwrap();
        let plant = changed["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "plant")
            .unwrap();
        let setpoint = plant["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == constant_name)
            .unwrap();
        setpoint["parameters"][0]["value"] = json!(changed_setpoint);
        let mut limited = Simulation::new_test(&changed.to_string(), &weather, "{}").unwrap();
        let altered = run_weather_intervals(&mut limited, 48);
        let baseline_row = &baseline["rows"][47];
        let altered_row = &altered["rows"][47];
        assert!(baseline_row[delivered_name].as_f64().unwrap() > 0.0);
        assert_eq!(
            altered_row[if sensor_name == "hot_water_supply_temp_c" {
                "hot_water_supply_setpoint_c"
            } else {
                "chilled_water_supply_setpoint_c"
            }],
            changed_setpoint
        );
        let actual = altered_row[sensor_name].as_f64().unwrap();
        let baseline_actual = baseline_row[sensor_name].as_f64().unwrap();
        if sensor_name == "hot_water_supply_temp_c" {
            assert!(actual < baseline_actual - 1.0);
        } else {
            assert!(actual > baseline_actual + 1.0);
        }
        let altered_zone = altered_row["avg_zone_c"].as_f64().unwrap();
        let baseline_zone = baseline_row["avg_zone_c"].as_f64().unwrap();
        if delivered_name == "heat_kw" {
            assert!(altered_zone < baseline_zone);
        } else {
            assert!(altered_zone > baseline_zone);
        }
        assert_water_heat_balance(altered_row);
    }
}

#[test]
fn vav_request_starts_fan_when_duct_pressure_is_zero() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 30.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false,
        };
        35_040
    ])
    .unwrap();
    let mut sim = Simulation::new_test(graph, &weather, "{}").unwrap();
    sim.temps = [34.0; 10];
    sim.last_duct_pressure = [0.0; 2];
    let result = run_weather_intervals(&mut sim, 3);
    assert!(
        result["rows"][0]["requested_airflow"].as_f64().unwrap() > 0.0,
        "row0 request={} avg={} supply={} fan={}",
        result["rows"][0]["requested_airflow"],
        result["rows"][0]["avg_zone_c"],
        result["rows"][0]["ahu_1_supply_actual_c"],
        result["rows"][0]["ahu_1_duct_pressure"]
    );
    assert!(result["rows"][1]["delivered_airflow"].as_f64().unwrap() > 0.0);
}

#[test]
fn plant_nameplate_capacity_limits_heating_delivery() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: -8.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut sized = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut undersized = Simulation::new_test(
        graph,
        &weather,
        r#"{"heat_pump_capacity_kw":1,"boiler_capacity_kw":1}"#,
    )
    .unwrap();
    let normal: Value = serde_json::from_str(&sized.step_chunk_json(12).unwrap()).unwrap();
    let limited: Value = serde_json::from_str(&undersized.step_chunk_json(12).unwrap()).unwrap();
    let normal = &normal["rows"][11];
    let limited = &limited["rows"][11];
    assert!(normal["heat_kw"].as_f64().unwrap() > limited["heat_kw"].as_f64().unwrap());
    let source_heat = limited["heat_pump_heating_used_kw"].as_f64().unwrap()
        + limited["boiler_used_kw"].as_f64().unwrap();
    assert!(source_heat <= 2.0 + 1e-9);
    assert!(
        limited["heat_kw"].as_f64().unwrap() > source_heat,
        "stored hot water should briefly cover a source shortfall"
    );
    let stored_heat = (undersized.last_water_supply_c[0]
        - limited["hot_water_supply_temp_c"].as_f64().unwrap())
        * (5_000.0 * 4.18 / 3_600.0)
        / 0.25;
    assert!((source_heat - limited["heat_kw"].as_f64().unwrap() - stored_heat).abs() < 1e-8);
    assert!(normal["avg_zone_c"].as_f64().unwrap() > limited["avg_zone_c"].as_f64().unwrap());
}

#[test]
fn water_tanks_never_supply_more_energy_than_their_inventory_and_sources() {
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let plant = graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "plant")
        .unwrap();
    for output in ["hot_water_pump_speed", "chilled_water_pump_speed"] {
        plant["connections"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|edge| edge["to"] == output)
            .unwrap()["from"] = json!("one.y");
    }
    let graph = graph.to_string();
    let capacity = 5_000.0 * 4.18 / 3_600.0;
    for (outdoor, initial_zone, loop_index, delivered, hp, backup) in [
        (
            -12.0,
            12.0,
            0,
            "heat_kw",
            "heat_pump_heating_used_kw",
            "boiler_used_kw",
        ),
        (
            38.0,
            34.0,
            1,
            "cool_kw",
            "heat_pump_cooling_used_kw",
            "chiller_used_kw",
        ),
    ] {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: outdoor,
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
            r#"{"water_pump_design_flow_kg_s":[20,20],"equipment_outages":[{"equipment":"heat_pump","start_hour":0,"duration_hours":4},{"equipment":"boiler","start_hour":0,"duration_hours":4},{"equipment":"chiller","start_hour":0,"duration_hours":4}]}"#,
        ).unwrap();
        sim.temps = [initial_zone; 10];
        sim.last_water_supply_c[loop_index] = if loop_index == 0 { 35.1 } else { 16.9 };
        let mut saw_delivery = false;
        let mut saw_inventory_limit = false;
        for _ in 0..12 {
            let before = sim.last_water_supply_c[loop_index];
            let chunk: Value = serde_json::from_str(&sim.step_chunk_json(1).unwrap()).unwrap();
            let row = &chunk["rows"][0];
            let delivered_kw = row[delivered].as_f64().unwrap();
            let source_kw = row[hp].as_f64().unwrap() + row[backup].as_f64().unwrap();
            let after = sim.last_water_supply_c[loop_index];
            let inventory_change_kw = if loop_index == 0 {
                (after - before) * capacity / 0.25
            } else {
                (before - after) * capacity / 0.25
            };
            assert!(
                (source_kw - delivered_kw - inventory_change_kw).abs() < 1e-7,
                "loop={loop_index} before={before} after={after} source={source_kw} delivered={delivered_kw}"
            );
            saw_delivery |= delivered_kw > 0.0;
            saw_inventory_limit |= delivered_kw > 0.0
                && (after - if loop_index == 0 { 35.0 } else { 17.0 }).abs() < 1e-9;
        }
        assert!(
            saw_delivery,
            "loop {loop_index} did not exercise a coil load"
        );
        assert!(
            saw_inventory_limit,
            "loop {loop_index} did not exhaust stored energy"
        );
    }
}

#[test]
fn smaller_timestep_advances_weather_every_fifteen_minutes() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let mut samples = vec![
        Weather {
            outdoor_c: 12.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false,
        };
        35_040
    ];
    samples[1].outdoor_c = 30.0;
    let weather = serde_json::to_string(&samples).unwrap();
    let mut sim = Simulation::new_test(graph, &weather, r#"{"step_seconds":15}"#).unwrap();
    let first: Value = serde_json::from_str(&sim.step_chunk_json(59).unwrap()).unwrap();
    assert_eq!(first["progress"], 59);
    assert_eq!(first["total"], 2_102_400);
    assert_eq!(first["rows"].as_array().unwrap().len(), 0);
    let boundary: Value = serde_json::from_str(&sim.step_chunk_json(1).unwrap()).unwrap();
    assert_eq!(boundary["rows"][0]["index"], 0);
    assert_eq!(boundary["rows"][0]["outdoor_c"], 12.0);
    let next: Value = serde_json::from_str(&sim.step_chunk_json(60).unwrap()).unwrap();
    assert_eq!(next["rows"][0]["index"], 1);
    assert_eq!(next["rows"][0]["outdoor_c"], 30.0);
    assert!(next["summary"]["electricity_kwh"].as_f64().unwrap() > 0.0);
}

#[test]
fn compiled_simulation_matches_dynamic_executor() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/weather_2025.json"
    ));
    let traces = serde_json::to_string(&vec![
        "FourPipeBuildingControls.plant|backup_fraction.y",
        "FourPipeBuildingControls.ahu_1|is_cold.y",
        "FourPipeBuildingControls|edge.0",
        "FourPipeBuildingControls|outdoor_c",
        "FourPipeBuildingControls|output.ahu_1_supply_air_temp",
        "FourPipeBuildingControls.zone_01|edge.0",
        "FourPipeBuildingControls.zone_01|cool_error.y",
    ])
    .unwrap();
    for step_seconds in [900, 15] {
        let scenario = format!("{{\"step_seconds\":{step_seconds}}}");
        let mut fast = Simulation::new_test(graph, weather, &scenario).unwrap();
        assert!(fast.oracle.is_none());
        fast.set_traces_json(&traces).unwrap();
        let mut dynamic = Simulation::new_test(graph, weather, &scenario).unwrap();
        dynamic.oracle = Some(cdl_runtime_oracle::Executor::default());
        dynamic.set_traces_json(&traces).unwrap();
        let count = if step_seconds == 900 { 96 } else { 120 };
        let fast_rows: Value = serde_json::from_str(&fast.step_chunk_json(count).unwrap()).unwrap();
        let dynamic_rows: Value =
            serde_json::from_str(&dynamic.step_chunk_json(count).unwrap()).unwrap();
        assert_eq!(fast_rows["progress"], dynamic_rows["progress"]);
        let fast_rows = fast_rows["rows"].as_array().unwrap();
        let dynamic_rows = dynamic_rows["rows"].as_array().unwrap();
        assert_eq!(fast_rows.len(), dynamic_rows.len());
        for (a, b) in fast_rows.iter().zip(dynamic_rows) {
            for name in [
                "avg_zone_c",
                "electric_kw",
                "gas_kw",
                "heat_kw",
                "cool_kw",
                "requested_airflow",
            ] {
                assert!(
                    (a[name].as_f64().unwrap() - b[name].as_f64().unwrap()).abs() < 1e-9,
                    "{name} differs at step {step_seconds}"
                );
            }
            for target in [
                "FourPipeBuildingControls.plant|backup_fraction.y",
                "FourPipeBuildingControls.ahu_1|is_cold.y",
                "FourPipeBuildingControls|edge.0",
                "FourPipeBuildingControls|outdoor_c",
                "FourPipeBuildingControls|output.ahu_1_supply_air_temp",
                "FourPipeBuildingControls.zone_01|edge.0",
                "FourPipeBuildingControls.zone_01|cool_error.y",
            ] {
                assert!(
                    (a["traces"][target].as_f64().unwrap() - b["traces"][target].as_f64().unwrap())
                        .abs()
                        < 1e-9,
                    "{target} differs"
                );
            }
        }
    }
}

#[test]
#[ignore = "Manual performance comparison; run with --ignored --nocapture"]
fn benchmark_compiled_vs_dynamic() {
    use std::time::Instant;
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/weather_2025.json"
    ));
    let mut fast = Simulation::new_test(graph, weather, r#"{"step_seconds":15}"#).unwrap();
    let mut dynamic = Simulation::new_test(graph, weather, r#"{"step_seconds":15}"#).unwrap();
    dynamic.oracle = Some(cdl_runtime_oracle::Executor::default());
    for (label, sim) in [("compiled", &mut fast), ("dynamic", &mut dynamic)] {
        let started = Instant::now();
        while sim.cursor < 12_000 {
            sim.step_chunk_json(672).unwrap();
        }
        let seconds = started.elapsed().as_secs_f64();
        println!(
            "{label}: {} steps in {seconds:.3}s ({:.0} steps/s)",
            sim.cursor,
            sim.cursor as f64 / seconds
        );
    }
}

#[test]
fn closed_loop_changes_after_setpoint_edit() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 20.0,
            solar_kw: 1.0,
            internal_kw: 1.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let original = serde_json::to_string(&graph).unwrap();
    let mut a = Simulation::new_test(&original, &weather, "{}").unwrap();
    let baseline: Value = serde_json::from_str(&a.step_chunk_json(16).unwrap()).unwrap();
    for b in &mut graph.blocks {
        if let Some(source) = b
            .blocks
            .iter_mut()
            .find(|child| child.name == "cool_setpoint")
        {
            if let Some(p) = source.parameters.iter_mut().find(|p| p.name == "k") {
                p.value = json!(20.0)
            }
        }
    }
    let mut b =
        Simulation::new_test(&serde_json::to_string(&graph).unwrap(), &weather, "{}").unwrap();
    let changed: Value = serde_json::from_str(&b.step_chunk_json(16).unwrap()).unwrap();
    assert_ne!(
        baseline["rows"][15]["avg_zone_c"],
        changed["rows"][15]["avg_zone_c"]
    );
}
