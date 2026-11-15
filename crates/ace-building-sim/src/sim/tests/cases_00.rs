use super::*;

#[test]
fn physical_water_pump_curve_changes_measured_feedback_from_same_cdl_request() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
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
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut smaller = Simulation::new_test(
        graph,
        &weather,
        r#"{"step_seconds":900,"water_pump_design_flow_kg_s":[2.0,4.31],"water_pump_shutoff_pressure_pa":[40000,80000]}"#,
    )
    .unwrap();
    for sim in [&mut normal, &mut smaller] {
        start_present_window(sim);
        sim.temps = [16.0; 10];
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        sim.step_chunk_json(1).unwrap();
    }
    assert!(normal.last_water_dp_pa[0] > 0.0);
    assert!(normal.last_water_flow_kg_s[0] > 0.0);
    assert!(smaller.last_water_dp_pa[0] < normal.last_water_dp_pa[0]);
    assert!(smaller.last_water_flow_kg_s[0] < normal.last_water_flow_kg_s[0]);
    assert_eq!(smaller.last_water_dp_pa[1], normal.last_water_dp_pa[1]);
    assert_eq!(
        smaller.last_water_flow_kg_s[1],
        normal.last_water_flow_kg_s[1]
    );
}

#[test]
fn physical_water_valve_network_changes_flow_without_changing_cdl_pump_command() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
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
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut restricted = Simulation::new_test(
        graph,
        &weather,
        r#"{"water_bypass_conductance":[0.05,0.2],"water_coil_conductance":[0.2,0.8],"hot_reheat_conductance":0.2}"#,
    )
    .unwrap();
    for sim in [&mut normal, &mut restricted] {
        start_present_window(sim);
        sim.temps = [16.0; 10];
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
    }
    let normal_row: Value = serde_json::from_str(&normal.step_chunk_json(1).unwrap()).unwrap();
    let restricted_row: Value =
        serde_json::from_str(&restricted.step_chunk_json(1).unwrap()).unwrap();
    assert_eq!(
        normal_row["rows"][0]["hot_water_pump_speed"],
        restricted_row["rows"][0]["hot_water_pump_speed"]
    );
    assert!(normal.last_water_flow_kg_s[0] > restricted.last_water_flow_kg_s[0]);
    assert!(normal.last_water_dp_pa[0] < restricted.last_water_dp_pa[0]);
    assert_eq!(
        normal.last_water_flow_kg_s[1],
        restricted.last_water_flow_kg_s[1]
    );
}

#[test]
fn bypass_flow_cannot_deliver_heat_or_cooling_to_closed_water_branches() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    for (outdoor_c, zone_c, hot) in [(-10.0, 16.0, true), (35.0, 32.0, false)] {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut connected = Simulation::new_test(graph, &weather, "{}").unwrap();
        let mut bypass = Simulation::new_test(
            graph,
            &weather,
            r#"{"water_bypass_conductance":[0.8,0.8],"water_coil_conductance":[0,0],"hot_reheat_conductance":0}"#,
        )
        .unwrap();
        for sim in [&mut connected, &mut bypass] {
            start_present_window(sim);
            sim.temps = [zone_c; 10];
            if hot {
                sim.last_heat_request = 100.0;
            } else {
                sim.last_cool_request = 100.0;
            }
            sim.last_water_flow_kg_s = [2.0, 2.0];
            sim.last_water_pump_running = [true, true];
        }
        let connected_rows = run_weather_intervals(&mut connected, 4);
        let bypass_rows = run_weather_intervals(&mut bypass, 4);
        let demand = if hot {
            "heat_request_kw"
        } else {
            "cool_request_kw"
        };
        let delivery = if hot { "heat_kw" } else { "cool_kw" };
        let loop_name = if hot { "hot" } else { "chilled" };
        let connected_peak = connected_rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row[delivery].as_f64().unwrap())
            .fold(0.0_f64, f64::max);
        assert!(connected_peak > 0.0, "{loop_name} branch should deliver");
        for row in connected_rows["rows"].as_array().unwrap() {
            assert_water_branch_balance(row);
        }
        for row in bypass_rows["rows"].as_array().unwrap() {
            assert_water_branch_balance(row);
            assert!(
                row[format!("{loop_name}_water_actual_flow_kg_s")]
                    .as_f64()
                    .unwrap()
                    > 0.0
            );
            assert!(row[demand].as_f64().unwrap() > 0.0);
            assert_eq!(row[delivery], json!(0.0));
            let bypass_flow = row[format!("{loop_name}_water_bypass_flow_kg_s")]
                .as_f64()
                .unwrap();
            let total_flow = row[format!("{loop_name}_water_actual_flow_kg_s")]
                .as_f64()
                .unwrap();
            assert!((bypass_flow - total_flow).abs() < 1e-9);
            assert_water_heat_balance(row);
            for ahu in 1..=2 {
                assert_eq!(
                    row[format!("ahu_{ahu}_heating_coil_delivered_kw")],
                    json!(0.0)
                );
                assert_eq!(
                    row[format!("ahu_{ahu}_cooling_coil_delivered_kw")],
                    json!(0.0)
                );
            }
        }
    }
}

#[test]
fn conventional_cdl_pump_sizing_restores_reheat_delivery_with_branch_limits() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
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
    let mut previous_graph: Value = serde_json::from_str(graph).unwrap();
    let previous_plant = previous_graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "plant")
        .unwrap();
    let previous_gain = previous_plant["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "hot_water_pump_gain")
        .unwrap();
    assert_eq!(previous_gain["parameters"][0]["value"], json!(1.0 / 90.0));
    previous_gain["parameters"][0]["value"] = json!(1.0 / 180.0);
    let mut stock = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut old_gain = Simulation::new_test(&previous_graph.to_string(), &weather, "{}").unwrap();
    for sim in [&mut stock, &mut old_gain] {
        start_present_window(sim);
        sim.temps = [16.0; 10];
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
    }
    let stock_rows = run_weather_intervals(&mut stock, 4);
    let previous_rows = run_weather_intervals(&mut old_gain, 4);
    let stock_last = &stock_rows["rows"][3];
    let previous_last = &previous_rows["rows"][3];
    assert!(
        stock_last["hot_water_pump_speed"].as_f64().unwrap()
            > previous_last["hot_water_pump_speed"].as_f64().unwrap() + 0.05
    );
    assert!(
        stock_last["hot_water_reheat_flow_kg_s"].as_f64().unwrap()
            > previous_last["hot_water_reheat_flow_kg_s"]
                .as_f64()
                .unwrap()
                + 0.05
    );
    assert!(
        stock_last["avg_zone_c"].as_f64().unwrap() > previous_last["avg_zone_c"].as_f64().unwrap()
    );
    for row in stock_rows["rows"].as_array().unwrap() {
        assert_water_heat_balance(row);
        assert_water_branch_balance(row);
    }
}

#[test]
fn physical_hot_water_pump_failure_reaches_cdl_source_proof() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
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
    let scenario = json!({
        "step_seconds": 60,
        "water_pump_faults": [{"loop_name": "hot", "start_hour": 7.75, "duration_hours": 0.5}]
    });
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    start_present_window(&mut sim);
    sim.temps = [16.0; 10];
    sim.last_heat_request = 100.0;
    sim.last_water_flow_kg_s = [2.0, 2.0];
    sim.last_water_pump_running = [true, true];
    let proof = "FourPipeG36Controls|output.plant_hot_source_proven";
    let fault = "FourPipeG36Controls|output.plant_hot_water_pump_start_fault";
    sim.set_traces_json(&json!([proof, fault]).to_string())
        .unwrap();
    let result = run_weather_intervals(&mut sim, 3);
    let rows = result["rows"].as_array().unwrap();
    for row in &rows[..2] {
        assert!(row["hot_water_pump_speed"].as_f64().unwrap() > 0.0);
        assert_eq!(row["hot_water_pump_running"], json!(0.0));
        assert_eq!(row["hot_water_flow_kg_s"], json!(0.0));
        assert_eq!(row["hot_water_differential_pressure_pa"], json!(0.0));
        assert_eq!(row["traces"][proof], json!(0.0));
        assert_eq!(row["traces"][fault], json!(1.0));
    }
    assert!(rows[2]["hot_water_flow_kg_s"].as_f64().unwrap() > 0.0);
    assert_eq!(rows[2]["hot_water_pump_running"], json!(1.0));
    assert!(rows[2]["traces"][proof].as_f64().unwrap() > 0.0);
    assert_eq!(rows[2]["traces"][fault], json!(0.0));
}

#[test]
fn biased_water_flow_sensor_changes_cdl_proof_without_changing_hydraulics() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    for (loop_index, loop_name, outdoor_c, zone_c) in
        [(0, "hot", -5.0, 16.0), (1, "chilled", 35.0, 32.0)]
    {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let scenario = json!({
            "step_seconds": 60,
            "water_flow_sensor_events": [{
                "loop_name": loop_name,
                "bias_kg_s": -20.0,
                "start_hour": 7.75,
                "duration_hours": 0.5
            }],
            "plant_alarm_acknowledgements": [{"start_hour": 8.0, "duration_hours": 0.25}]
        });
        let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
        start_present_window(&mut sim);
        sim.temps = [zone_c; 10];
        if loop_index == 0 {
            sim.last_heat_request = 100.0;
        } else {
            sim.last_cool_request = 100.0;
        }
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        let proof = format!("FourPipeG36Controls|output.plant_{loop_name}_source_proven");
        let flow_fault = format!("FourPipeG36Controls|output.plant_{loop_name}_water_flow_fault");
        let acknowledged =
            format!("FourPipeG36Controls|output.plant_{loop_name}_water_flow_fault_acknowledged");
        sim.set_traces_json(&json!([proof, flow_fault, acknowledged]).to_string())
            .unwrap();
        let result = run_weather_intervals(&mut sim, 3);
        let rows = result["rows"].as_array().unwrap();
        for row in &rows[..2] {
            assert_water_heat_balance(row);
            assert_eq!(row[format!("{loop_name}_water_flow_kg_s")], json!(0.0));
            assert!(
                row[format!("{loop_name}_water_actual_flow_kg_s")]
                    .as_f64()
                    .unwrap()
                    > 0.0
            );
            assert_eq!(row["traces"][&proof], json!(0.0));
            assert_eq!(row["traces"][&flow_fault], json!(1.0));
        }
        assert_eq!(rows[0]["traces"][&acknowledged], json!(0.0));
        assert_eq!(rows[1]["traces"][&acknowledged], json!(1.0));
        assert_water_heat_balance(&rows[2]);
        assert!(
            rows[2][format!("{loop_name}_water_flow_kg_s")]
                .as_f64()
                .unwrap()
                > 0.0
        );
        assert_eq!(rows[2]["traces"][&proof], json!(1.0));
        assert_eq!(rows[2]["traces"][&flow_fault], json!(0.0));
        assert_eq!(rows[2]["traces"][&acknowledged], json!(0.0));
    }
}

#[test]
fn physical_pump_startup_delay_exercises_cdl_start_diagnostic_and_source_proof() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    for (loop_index, outdoor_c, zone_c, loop_name) in
        [(0, -5.0, 16.0, "hot"), (1, 35.0, 32.0, "chilled")]
    {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut startup = [0.0; 2];
        startup[loop_index] = 1200.0;
        let scenario = json!({
            "step_seconds": 60,
            "water_pump_startup_seconds": startup,
        });
        let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
        start_present_window(&mut sim);
        sim.temps = [zone_c; 10];
        if loop_index == 0 {
            sim.last_heat_request = 100.0;
        } else {
            sim.last_cool_request = 100.0;
        }
        let fault = format!("FourPipeG36Controls|output.plant_{loop_name}_water_pump_start_fault");
        let proof = format!("FourPipeG36Controls|output.plant_{loop_name}_source_proven");
        sim.set_traces_json(&json!([fault, proof]).to_string())
            .unwrap();
        let rows = run_weather_intervals(&mut sim, 3);
        let rows = rows["rows"].as_array().unwrap();
        let (speed, running, flow) = if loop_index == 0 {
            (
                "hot_water_pump_speed",
                "hot_water_pump_running",
                "hot_water_flow_kg_s",
            )
        } else {
            (
                "chilled_water_pump_speed",
                "chilled_water_pump_running",
                "chilled_water_flow_kg_s",
            )
        };
        assert!(
            rows[0][speed].as_f64().unwrap() > 0.05,
            "{loop_name} CDL speed command"
        );
        assert_eq!(rows[0][running], json!(0.0));
        assert_eq!(rows[0][flow], json!(0.0));
        assert_eq!(rows[0]["traces"][&fault], json!(1.0));
        assert_eq!(rows[0]["traces"][&proof], json!(0.0));
        assert_eq!(rows[2][running], json!(1.0));
        assert!(rows[2][flow].as_f64().unwrap() > 0.0);
        assert_eq!(rows[2]["traces"][&fault], json!(0.0));
        assert_eq!(rows[2]["traces"][&proof], json!(1.0));
    }
}

#[test]
fn g36_pollution_mode_is_cdl_timed_and_disables_both_ahu_economizers() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    assert!(validate(&building).is_empty());
    let mode = building
        .blocks
        .iter()
        .find(|block| block.name == "outdoor_air_pollution_mode")
        .unwrap();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(mode).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("operator".into(), json!(2)),
        ("on_duration_s".into(), json!(3600.0)),
        ("local_day_seconds".into(), json!(0.0)),
        ("pm25".into(), json!(120.0)),
        ("pm25_available".into(), json!(true)),
        ("ozone".into(), json!(0.0)),
        ("ozone_available".into(), json!(false)),
        ("aqi".into(), json!(0.0)),
        ("aqi_available".into(), json!(false)),
    ]);
    for step in 0..121 {
        let slow = dynamic.execute(mode, &inputs, 15.0, "").unwrap();
        let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
        assert_eq!(slow, fast, "pollution step {step}");
        assert_eq!(slow["active"], json!(step == 120));
        assert_eq!(
            slow["level_one_alarm"],
            json!(if step == 120 { 1 } else { 0 })
        );
    }
    inputs.insert("operator".into(), json!(0));
    let off = dynamic.execute(mode, &inputs, 15.0, "").unwrap();
    assert_eq!(off["active"], json!(false));
    assert_eq!(off, compiled.execute(&inputs, 15.0, false).unwrap().0);
    inputs.insert("operator".into(), json!(2));
    inputs.insert("pm25".into(), json!(100.0));
    for step in 0..121 {
        let slow = dynamic.execute(mode, &inputs, 15.0, "").unwrap();
        let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
        assert_eq!(slow, fast, "pollution clear step {step}");
        assert_eq!(slow["active"], json!(step < 120));
    }
    for ahu_name in ["ahu_1", "ahu_2"] {
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == ahu_name)
            .unwrap();
        assert!(ahu
            .connections
            .iter()
            .any(|edge| edge.from_path == "economizer_pollution_gate.y"
                && edge.to_path == "economizer_enabled.u2"));
        assert!(ahu
            .connections
            .iter()
            .any(|edge| edge.from_path == "pollution_clear.y"
                && edge.to_path == "pollution_clear_hold.u"));
        assert!(ahu
            .connections
            .iter()
            .any(|edge| edge.from_path == "pollution_clear_hold.y"
                && edge.to_path == "economizer_pollution_gate.u2"));
        assert!(ahu
            .connections
            .iter()
            .any(|edge| edge.from_path == "minimum_oa_mode_enabled.y"
                && edge.to_path == "minimum_outdoor_air_enabled"));
    }
}

#[test]
fn g36_ahus_have_independent_editable_pm25_limits() {
    let mut building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let first = building
        .blocks
        .iter()
        .find(|block| block.name == "outdoor_air_pollution_mode")
        .unwrap();
    let second = building
        .blocks
        .iter()
        .find(|block| block.name == "outdoor_air_pollution_mode_2")
        .unwrap();
    assert_eq!(
        first
            .blocks
            .iter()
            .find(|block| block.name == "pm25_limit")
            .unwrap()
            .parameters[0]
            .value,
        json!(113.0)
    );
    assert_eq!(
        second
            .blocks
            .iter()
            .find(|block| block.name == "pm25_limit")
            .unwrap()
            .parameters[0]
            .value,
        json!(113.0)
    );
    building
        .blocks
        .iter_mut()
        .find(|block| block.name == "outdoor_air_pollution_mode_2")
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.name == "pm25_limit")
        .unwrap()
        .parameters[0]
        .value = json!(75.0);
    assert!(validate(&building).is_empty());
    for (index, mode_name) in [
        (1, "outdoor_air_pollution_mode"),
        (2, "outdoor_air_pollution_mode_2"),
    ] {
        let key = format!("ahu_{index}.pollution_active");
        assert!(building
            .connections
            .iter()
            .any(|edge| edge.from_path == format!("{mode_name}.active") && edge.to_path == key));
    }
    let inputs = HashMap::from([
        ("operator".into(), json!(2)),
        ("on_duration_s".into(), json!(3600.0)),
        ("local_day_seconds".into(), json!(0.0)),
        ("pm25".into(), json!(100.0)),
        ("pm25_available".into(), json!(true)),
        ("ozone".into(), json!(0.0)),
        ("ozone_available".into(), json!(false)),
        ("aqi".into(), json!(0.0)),
        ("aqi_available".into(), json!(false)),
    ]);
    for (name, expected) in [
        ("outdoor_air_pollution_mode", false),
        ("outdoor_air_pollution_mode_2", true),
    ] {
        let mode = building
            .blocks
            .iter()
            .find(|block| block.name == name)
            .unwrap();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(mode).unwrap().unwrap();
        for step in 0..121 {
            let slow = dynamic.execute(mode, &inputs, 15.0, "").unwrap();
            let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
            assert_eq!(slow, fast, "{name} step {step}");
            if step == 120 {
                assert_eq!(slow["active"], json!(expected));
            }
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
    let mut sim = Simulation::new_test(
        &serde_json::to_string(&building).unwrap(),
        &weather,
        r#"{"step_seconds":60,"pollution_operator":2,"outdoor_pm25_ug_m3":100.0}"#,
    )
    .unwrap();
    start_present_window(&mut sim);
    sim.temps = [24.0; 10];
    sim.last_mixed_air_c = [20.0; 2];
    let building_active = "FourPipeG36Controls|output.pollution_active";
    let ahu1 = "FourPipeG36Controls|output.ahu_1_pollution_active";
    let ahu2 = "FourPipeG36Controls|output.ahu_2_pollution_active";
    let direct1 = "FourPipeG36Controls|output.ahu_1_minimum_oa_return_direct";
    let direct2 = "FourPipeG36Controls|output.ahu_2_minimum_oa_return_direct";
    let minimum1 = "FourPipeG36Controls|output.ahu_1_minimum_outdoor_airflow";
    let minimum2 = "FourPipeG36Controls|output.ahu_2_minimum_outdoor_airflow";
    let outdoor2 = "FourPipeG36Controls|output.ahu_2_outdoor_air_damper_position";
    let return2 = "FourPipeG36Controls|output.ahu_2_return_air_damper_position";
    sim.set_traces_json(
        &json!([
            building_active,
            ahu1,
            ahu2,
            direct1,
            direct2,
            minimum1,
            minimum2,
            outdoor2,
            return2
        ])
        .to_string(),
    )
    .unwrap();
    let rows = run_weather_intervals(&mut sim, 8);
    let last = rows["rows"].as_array().unwrap().last().unwrap();
    assert_eq!(last["traces"][ahu1], json!(0.0));
    assert_eq!(last["traces"][ahu2], json!(1.0));
    assert_eq!(last["traces"][building_active], json!(1.0));
    assert_eq!(last["traces"][direct1], json!(0.0));
    assert_eq!(last["traces"][direct2], json!(1.0));
    assert!(
        last["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
            > last["ahu_2_outdoor_air_fraction"].as_f64().unwrap()
    );
    let ahu1_target = last["traces"][minimum1].as_f64().unwrap();
    let ahu2_target = last["traces"][minimum2].as_f64().unwrap();
    let ahu1_flow = last["ahu_1_outdoor_airflow"].as_f64().unwrap();
    let ahu2_flow = last["ahu_2_outdoor_airflow"].as_f64().unwrap();
    let ahu2_command = last["traces"][outdoor2].as_f64().unwrap();
    assert!(ahu1_target > 0.0 && ahu2_target > 0.0);
    assert!(ahu1_flow > ahu2_flow * 2.0);
    assert!(
        ahu2_command < 0.2,
        "the direct CDL branch must release the fixed outdoor opening floor"
    );
    assert_eq!(last["traces"][return2], json!(1.0));
    assert!(
        (last["ahu_2_actual_outdoor_damper_position"]
            .as_f64()
            .unwrap()
            - ahu2_command)
            .abs()
            < 1e-9
    );
    assert!(
        (ahu2_flow - ahu2_target).abs() / ahu2_target < 0.2,
        "AHU 2 physical outdoor flow {ahu2_flow} missed its CDL target {ahu2_target}"
    );
}

#[test]
fn g36_pollution_operator_changes_physical_outdoor_air_through_cdl() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 15.0,
            solar_kw: 0.0,
            internal_kw: 2.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut off = Simulation::new_test(
        graph,
        &weather,
        r#"{"step_seconds":60,"pollution_operator":0}"#,
    )
    .unwrap();
    let mut on = Simulation::new_test(
        graph,
        &weather,
        r#"{"step_seconds":60,"pollution_operator":1}"#,
    )
    .unwrap();
    for sim in [&mut off, &mut on] {
        start_present_window(sim);
        sim.temps = [24.0; 10];
        sim.last_mixed_air_c = [20.0; 2];
        sim.set_traces_json(
            &json!([
                "FourPipeG36Controls|output.pollution_active",
                "FourPipeG36Controls|output.ahu_1_economizer_mode_enabled",
                "FourPipeG36Controls|output.ahu_1_minimum_outdoor_airflow",
                "FourPipeG36Controls|output.ahu_1_minimum_oa_return_direct",
            ])
            .to_string(),
        )
        .unwrap();
    }
    let baseline = run_weather_intervals(&mut off, 4);
    let polluted = run_weather_intervals(&mut on, 4);
    let normal = &baseline["rows"][3];
    let limited = &polluted["rows"][3];
    assert_eq!(
        normal["traces"]["FourPipeG36Controls|output.pollution_active"],
        json!(0.0)
    );
    assert_eq!(
        limited["traces"]["FourPipeG36Controls|output.pollution_active"],
        json!(1.0)
    );
    assert_eq!(
        limited["traces"]["FourPipeG36Controls|output.ahu_1_economizer_mode_enabled"],
        json!(0.0)
    );
    assert_eq!(
        normal["traces"]["FourPipeG36Controls|output.ahu_1_minimum_oa_return_direct"],
        json!(0.0)
    );
    assert_eq!(
        limited["traces"]["FourPipeG36Controls|output.ahu_1_minimum_oa_return_direct"],
        json!(1.0)
    );
    assert!(
        limited["traces"]["FourPipeG36Controls|output.ahu_1_minimum_outdoor_airflow"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        normal["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
            > limited["ahu_1_outdoor_air_fraction"].as_f64().unwrap(),
        "normal fraction={}, pollution fraction={}, normal economizer={}, minimum={}",
        normal["ahu_1_outdoor_air_fraction"],
        limited["ahu_1_outdoor_air_fraction"],
        normal["traces"]["FourPipeG36Controls|output.ahu_1_economizer_mode_enabled"],
        limited["traces"]["FourPipeG36Controls|output.ahu_1_minimum_outdoor_airflow"]
    );
    assert!(limited["ahu_1_outdoor_airflow"].as_f64().unwrap() > 0.0);
}

#[test]
fn g36_common_damper_direct_return_follows_pollution_and_qualified_high_limit() {
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
        ("freeze_stat_trip".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
        ("freeze_reset".into(), json!(false)),
        ("pollution_active".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let clear_trace = "ahu_1|pollution_clear_hold.y".to_string();
    let direct_active_trace = "ahu_1|minimum_oa_direct_return_delay.y".to_string();
    dynamic.trace_targets = vec![
        "ahu_1|return_damper_pi_max.y".into(),
        clear_trace.clone(),
        direct_active_trace.clone(),
    ];
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    compiled
        .set_traces(&[clear_trace.clone(), direct_active_trace.clone()])
        .unwrap();
    let mut step_index = 0;
    let mut step = |inputs: &HashMap<String, Value>, expected: bool| {
        step_index += 1;
        let slow = dynamic.execute(ahu, inputs, 60.0, "").unwrap();
        let (fast, fast_traces) = compiled.execute(inputs, 60.0, true).unwrap();
        assert_eq!(
            slow["return_air_damper_position"],
            fast["return_air_damper_position"]
        );
        let direct_active = dynamic.trace_values[&direct_active_trace]
            .as_bool()
            .unwrap();
        assert_eq!(
            slow["minimum_oa_return_direct"],
            json!(direct_active),
            "step {step_index}"
        );
        assert_eq!(
            slow["minimum_oa_return_direct"],
            fast["minimum_oa_return_direct"]
        );
        assert!(!direct_active || expected);
        assert_eq!(
            fast_traces[&direct_active_trace],
            json!(if direct_active { 1.0 } else { 0.0 })
        );
        if direct_active {
            assert_eq!(
                slow["return_air_damper_position"],
                dynamic.trace_values["ahu_1|return_damper_pi_max.y"]
            );
        } else if expected {
            assert_eq!(slow["return_air_damper_position"], json!(1.0));
        }
        let clear = dynamic.trace_values[&clear_trace].as_bool().unwrap();
        assert_eq!(
            fast_traces[&clear_trace],
            json!(if clear { 1.0 } else { 0.0 })
        );
        clear
    };
    assert!(step(&inputs, false));
    inputs.insert("pollution_active".into(), json!(true));
    assert!(!step(&inputs, true));
    inputs.insert("pollution_active".into(), json!(false));
    assert!(!step(&inputs, false));
    inputs.insert("outdoor_c".into(), json!(30.0));
    let mut clear = false;
    for _ in 0..10 {
        clear = step(&inputs, false);
    }
    assert!(
        clear,
        "pollution release must mature after ten clear minutes"
    );
    assert!(step(&inputs, true));
    inputs.insert("outdoor_c".into(), json!(0.0));
    for _ in 0..10 {
        step(&inputs, true);
    }
    step(&inputs, false);
}

#[test]
fn g36_pollution_fan_proof_and_freeze_transitions_stay_in_cdl() {
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
        ("freeze_stat_trip".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
        ("freeze_reset".into(), json!(false)),
        ("pollution_active".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    let mut step = |inputs: &HashMap<String, Value>| {
        let slow = dynamic.execute(ahu, inputs, 15.0, "").unwrap();
        let (fast, _) = compiled.execute(inputs, 15.0, false).unwrap();
        for name in [
            "economizer_mode_enabled",
            "minimum_outdoor_air_enabled",
            "minimum_oa_return_direct",
            "freeze_stage_three",
            "freeze_alarm_level",
            "fan_speed",
            "outdoor_air_damper_position",
        ] {
            assert_eq!(slow[name], fast[name], "dynamic/compiled {name}");
        }
        slow
    };
    let normal = step(&inputs);
    assert_eq!(normal["freeze_stage_three"], json!(false));
    assert_eq!(normal["economizer_mode_enabled"], json!(true));
    inputs.insert("pollution_active".into(), json!(true));
    for _ in 0..12 {
        step(&inputs);
    }
    let polluted = step(&inputs);
    assert_eq!(polluted["economizer_mode_enabled"], json!(false));
    assert_eq!(polluted["minimum_outdoor_air_enabled"], json!(true));
    assert_eq!(polluted["minimum_oa_return_direct"], json!(true));

    inputs.insert("supply_fan_proven".into(), json!(false));
    let no_proof = step(&inputs);
    assert_eq!(no_proof["minimum_outdoor_air_enabled"], json!(false));
    assert_eq!(no_proof["minimum_oa_return_direct"], json!(false));
    inputs.insert("supply_fan_proven".into(), json!(true));
    let proof_restored = step(&inputs);
    assert_eq!(proof_restored["minimum_outdoor_air_enabled"], json!(true));
    assert_eq!(proof_restored["minimum_oa_return_direct"], json!(false));

    inputs.insert("freeze_stat_trip".into(), json!(true));
    let frozen = step(&inputs);
    assert_eq!(frozen["freeze_stage_three"], json!(true));
    assert_eq!(frozen["fan_speed"], json!(0.0));
    assert_eq!(frozen["outdoor_air_damper_position"], json!(0.0));
    assert_eq!(frozen["economizer_mode_enabled"], json!(false));
    inputs.insert("freeze_stat_trip".into(), json!(false));
    assert_eq!(step(&inputs)["freeze_stage_three"], json!(true));
    inputs.insert("freeze_reset".into(), json!(true));
    let reset = step(&inputs);
    assert_eq!(reset["freeze_stage_three"], json!(false));
    assert_eq!(reset["economizer_mode_enabled"], json!(false));
    inputs.insert("freeze_reset".into(), json!(false));
    inputs.insert("pollution_active".into(), json!(false));
    for _ in 0..40 {
        assert_eq!(step(&inputs)["economizer_mode_enabled"], json!(false));
    }
    let released = step(&inputs);
    assert_eq!(released["economizer_mode_enabled"], json!(true));
    assert_eq!(released["freeze_stage_three"], json!(false));
    inputs.insert("outdoor_c".into(), json!(30.0));
    for _ in 0..60 {
        step(&inputs);
    }
    let high_limit = step(&inputs);
    assert_eq!(high_limit["economizer_mode_enabled"], json!(false));
    assert_eq!(high_limit["minimum_oa_return_direct"], json!(true));
    inputs.insert("outdoor_c".into(), json!(0.0));
    for _ in 0..45 {
        step(&inputs);
    }
    let high_limit_cleared = step(&inputs);
    assert_eq!(high_limit_cleared["minimum_oa_return_direct"], json!(false));
    assert_eq!(high_limit_cleared["economizer_mode_enabled"], json!(true));
}

#[test]
fn g36_direct_minimum_air_stages_outdoor_and_return_dampers_in_cdl() {
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
        ("freeze_stat_trip".into(), json!(false)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
        ("occupied_mode".into(), json!(true)),
        ("freeze_reset".into(), json!(false)),
        ("pollution_active".into(), json!(false)),
    ]);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
    }
    let trace_names = [
        "ahu_1|minimum_oa_direct_outdoor_delay.y",
        "ahu_1|minimum_oa_direct_return_delay.y",
        "ahu_1|minimum_oa_direct_base_select.y",
        "ahu_1|outdoor_air_select.y",
        "ahu_1|minimum_oa_off_command.y",
        "ahu_1|return_damper_pi_max.y",
    ];
    let mut dynamic = Executor::default();
    dynamic.trace_targets = trace_names.iter().map(|name| (*name).into()).collect();
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    compiled
        .set_traces(
            &trace_names
                .iter()
                .map(|name| (*name).into())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
    compiled.execute(&inputs, 15.0, true).unwrap();
    inputs.insert("pollution_active".into(), json!(true));
    for tick in 0..=12 {
        let slow = dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
        let (fast, traces) = compiled.execute(&inputs, 15.0, true).unwrap();
        assert_eq!(slow, fast, "pollution step {tick}");
        let outdoor_delay = dynamic.trace_values[trace_names[0]].as_bool().unwrap();
        let return_delay = dynamic.trace_values[trace_names[1]].as_bool().unwrap();
        assert_eq!(outdoor_delay, tick >= 1, "outdoor stage at step {tick}");
        assert_eq!(return_delay, tick >= 12, "return stage at step {tick}");
        for (name, value) in [
            (trace_names[0], outdoor_delay),
            (trace_names[1], return_delay),
        ] {
            assert_eq!(traces[name], json!(if value { 1.0 } else { 0.0 }));
        }
        let base = &dynamic.trace_values[trace_names[2]];
        assert_eq!(
            base,
            &dynamic.trace_values[trace_names[if outdoor_delay { 4 } else { 3 }]]
        );
        assert_eq!(slow["minimum_oa_return_direct"], json!(return_delay));
        if return_delay {
            assert_eq!(
                slow["return_air_damper_position"],
                dynamic.trace_values[trace_names[5]]
            );
        } else {
            assert_eq!(slow["return_air_damper_position"], json!(1.0));
        }
    }
    inputs.insert("pollution_active".into(), json!(false));
    let slow = dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
    let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
    assert_eq!(slow, fast);
    assert_eq!(slow["minimum_oa_return_direct"], json!(false));
    assert_eq!(dynamic.trace_values[trace_names[0]], json!(false));
    assert_eq!(dynamic.trace_values[trace_names[1]], json!(false));
    let mut edited = ahu.clone();
    for (name, seconds) in [
        ("minimum_oa_direct_outdoor_delay", 30.0),
        ("minimum_oa_direct_return_delay", 240.0),
    ] {
        edited
            .blocks
            .iter_mut()
            .find(|block| block.name == name)
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "delayTime")
            .unwrap()
            .value = json!(seconds);
    }
    inputs.insert("pollution_active".into(), json!(true));
    let mut edited_dynamic = Executor::default();
    edited_dynamic.trace_targets = trace_names[..2].iter().map(|name| (*name).into()).collect();
    let mut edited_compiled = CompiledExecutor::try_new(&edited).unwrap().unwrap();
    edited_compiled
        .set_traces(
            &trace_names[..2]
                .iter()
                .map(|name| (*name).into())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    for tick in 0..=16 {
        let slow = edited_dynamic.execute(&edited, &inputs, 15.0, "").unwrap();
        let fast = edited_compiled.execute(&inputs, 15.0, false).unwrap().0;
        assert_eq!(slow, fast, "edited transition step {tick}");
        assert_eq!(
            edited_dynamic.trace_values[trace_names[0]],
            json!(tick >= 2)
        );
        assert_eq!(
            edited_dynamic.trace_values[trace_names[1]],
            json!(tick >= 16)
        );
        assert_eq!(slow["minimum_oa_return_direct"], json!(tick >= 16));
    }
}

#[test]
fn g36_pollution_on_duration_and_optional_sensors_are_editable_cdl() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mode = building
        .blocks
        .iter()
        .find(|block| block.name == "outdoor_air_pollution_mode")
        .unwrap()
        .clone();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&mode).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("operator".into(), json!(1)),
        ("on_duration_s".into(), json!(30.0)),
        ("local_day_seconds".into(), json!(0.0)),
        ("pm25".into(), json!(0.0)),
        ("pm25_available".into(), json!(false)),
        ("ozone".into(), json!(0.2)),
        ("ozone_available".into(), json!(false)),
        ("aqi".into(), json!(150.0)),
        ("aqi_available".into(), json!(false)),
    ]);
    for expected in [true, true, false] {
        let slow = dynamic.execute(&mode, &inputs, 15.0, "").unwrap();
        assert_eq!(slow, compiled.execute(&inputs, 15.0, false).unwrap().0);
        assert_eq!(slow["active"], json!(expected));
    }
    inputs.insert("operator".into(), json!(0));
    let off = dynamic.execute(&mode, &inputs, 15.0, "").unwrap();
    assert_eq!(off, compiled.execute(&inputs, 15.0, false).unwrap().0);
    assert_eq!(off["active"], json!(false));
    inputs.insert("operator".into(), json!(1));
    for expected in [true, true, false] {
        let slow = dynamic.execute(&mode, &inputs, 15.0, "").unwrap();
        assert_eq!(slow, compiled.execute(&inputs, 15.0, false).unwrap().0);
        assert_eq!(slow["active"], json!(expected));
    }
    inputs.insert("operator".into(), json!(2));
    for _ in 0..122 {
        let slow = dynamic.execute(&mode, &inputs, 15.0, "").unwrap();
        assert_eq!(slow, compiled.execute(&inputs, 15.0, false).unwrap().0);
        assert_eq!(slow["active"], json!(false));
    }
    inputs.insert("ozone_available".into(), json!(true));
    let mut ozone = HashMap::new();
    for step in 0..121 {
        ozone = dynamic.execute(&mode, &inputs, 15.0, "").unwrap();
        assert_eq!(ozone["active"], json!(step == 120));
        assert_eq!(ozone, compiled.execute(&inputs, 15.0, false).unwrap().0);
    }
    assert_eq!(ozone["active"], json!(true));
    inputs.insert("ozone".into(), json!(0.0));
    for _ in 0..121 {
        let slow = dynamic.execute(&mode, &inputs, 15.0, "").unwrap();
        assert_eq!(slow, compiled.execute(&inputs, 15.0, false).unwrap().0);
    }
    inputs.insert("aqi_available".into(), json!(true));
    let mut aqi = HashMap::new();
    for step in 0..121 {
        aqi = dynamic.execute(&mode, &inputs, 15.0, "").unwrap();
        assert_eq!(aqi["active"], json!(step == 120));
        assert_eq!(aqi, compiled.execute(&inputs, 15.0, false).unwrap().0);
    }
    assert_eq!(aqi["active"], json!(true));
}

#[test]
fn g36_pollution_sensor_timers_reset_when_reading_or_availability_interrupts() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mode = building
        .blocks
        .iter()
        .find(|block| block.name == "outdoor_air_pollution_mode")
        .unwrap()
        .clone();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&mode).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("operator".into(), json!(2)),
        ("on_duration_s".into(), json!(3600.0)),
        ("local_day_seconds".into(), json!(0.0)),
        ("pm25".into(), json!(0.0)),
        ("pm25_available".into(), json!(false)),
        ("ozone".into(), json!(0.2)),
        ("ozone_available".into(), json!(true)),
        ("aqi".into(), json!(0.0)),
        ("aqi_available".into(), json!(false)),
    ]);
    let mut step = |inputs: &HashMap<String, Value>, expected: bool| {
        let slow = dynamic.execute(&mode, inputs, 15.0, "").unwrap();
        let fast = compiled.execute(inputs, 15.0, false).unwrap().0;
        assert_eq!(slow, fast);
        assert_eq!(slow["active"], json!(expected));
    };

    for _ in 0..60 {
        step(&inputs, false);
    }
    inputs.insert("ozone".into(), json!(0.0));
    step(&inputs, false);
    inputs.insert("ozone".into(), json!(0.2));
    for _ in 0..120 {
        step(&inputs, false);
    }
    step(&inputs, true);

    for _ in 0..60 {
        inputs.insert("ozone".into(), json!(0.0));
        step(&inputs, true);
    }
    inputs.insert("ozone".into(), json!(0.2));
    step(&inputs, true);
    inputs.insert("ozone".into(), json!(0.0));
    for _ in 0..120 {
        step(&inputs, true);
    }
    step(&inputs, false);

    inputs.insert("ozone".into(), json!(0.2));
    for _ in 0..60 {
        step(&inputs, false);
    }
    inputs.insert("ozone_available".into(), json!(false));
    step(&inputs, false);
    inputs.insert("ozone_available".into(), json!(true));
    for _ in 0..120 {
        step(&inputs, false);
    }
    step(&inputs, true);
}

#[test]
fn g36_pollution_daily_level_three_reminder_is_cdl_scheduled() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut mode = building
        .blocks
        .iter()
        .find(|block| block.name == "outdoor_air_pollution_mode")
        .unwrap()
        .clone();
    let clock = mode
        .blocks
        .iter_mut()
        .find(|block| block.name == "daily_reminder_clock_time")
        .unwrap();
    clock
        .parameters
        .iter_mut()
        .find(|p| p.name == "k")
        .unwrap()
        .value = json!(15.0);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&mode).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("operator".into(), json!(1)),
        ("on_duration_s".into(), json!(600.0)),
        ("local_day_seconds".into(), json!(0.0)),
        ("pm25".into(), json!(0.0)),
        ("pm25_available".into(), json!(false)),
        ("ozone".into(), json!(0.0)),
        ("ozone_available".into(), json!(false)),
        ("aqi".into(), json!(0.0)),
        ("aqi_available".into(), json!(false)),
    ]);
    for step in 0..10 {
        if step == 6 {
            inputs.insert("operator".into(), json!(0));
        }
        inputs.insert("local_day_seconds".into(), json!((step % 4) * 15));
        let slow = dynamic.execute(&mode, &inputs, 15.0, "").unwrap();
        let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
        assert_eq!(slow, fast);
        assert_eq!(
            slow["level_three_reminder"],
            json!(if step == 1 || step == 5 { 3 } else { 0 })
        );
        assert_eq!(slow["level_one_alarm"], json!(if step < 6 { 1 } else { 0 }));
    }
}

#[test]
fn g36_pollution_reminder_fires_once_per_active_day_at_shipped_clock_time() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mode = building
        .blocks
        .iter()
        .find(|block| block.name == "outdoor_air_pollution_mode")
        .unwrap()
        .clone();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&mode).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("operator".into(), json!(2)),
        ("on_duration_s".into(), json!(3600.0)),
        ("local_day_seconds".into(), json!(0.0)),
        ("pm25".into(), json!(200.0)),
        ("pm25_available".into(), json!(true)),
        ("ozone".into(), json!(0.0)),
        ("ozone_available".into(), json!(false)),
        ("aqi".into(), json!(0.0)),
        ("aqi_available".into(), json!(false)),
    ]);
    let mut reminders = Vec::new();
    for minute in 0..(3 * 24 * 60) {
        if minute == 2 * 24 * 60 + 8 * 60 {
            inputs.insert("operator".into(), json!(0));
        }
        inputs.insert("local_day_seconds".into(), json!((minute % (24 * 60)) * 60));
        let slow = dynamic.execute(&mode, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow, fast, "minute {minute}");
        if slow["level_three_reminder"] == json!(3) {
            reminders.push(minute);
        }
    }
    assert_eq!(reminders, vec![9 * 60, 24 * 60 + 9 * 60]);
}

#[test]
fn g36_pollution_reminder_follows_local_clock_across_offset_changes() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mode = building
        .blocks
        .iter()
        .find(|block| block.name == "outdoor_air_pollution_mode")
        .unwrap();
    let events = [
        ClockOffsetEvent {
            start_hour: 32.0,
            utc_offset_hours: -5.0,
        },
        ClockOffsetEvent {
            start_hour: 79.0,
            utc_offset_hours: -6.0,
        },
    ];
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(mode).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("operator".into(), json!(1)),
        ("on_duration_s".into(), json!(604800.0)),
        ("local_day_seconds".into(), json!(0.0)),
        ("pm25".into(), json!(0.0)),
        ("pm25_available".into(), json!(false)),
        ("ozone".into(), json!(0.0)),
        ("ozone_available".into(), json!(false)),
        ("aqi".into(), json!(0.0)),
        ("aqi_available".into(), json!(false)),
    ]);
    let mut reminders = Vec::new();
    for step in 0..4 * 24 * 12 {
        let utc_hour = step as f64 / 12.0;
        inputs.insert(
            "local_day_seconds".into(),
            json!(local_day_seconds(utc_hour, -6.0, &events)),
        );
        let slow = dynamic.execute(mode, &inputs, 300.0, "").unwrap();
        let fast = compiled.execute(&inputs, 300.0, false).unwrap().0;
        assert_eq!(slow, fast, "UTC hour {utc_hour}");
        if slow["level_three_reminder"] == json!(3) {
            reminders.push(utc_hour);
        }
    }
    assert_eq!(reminders, vec![15.0, 38.0, 62.0, 87.0]);
}

#[test]
fn g36_pollution_reminder_handles_exact_start_without_late_start_pulse() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    for name in ["outdoor_air_pollution_mode", "outdoor_air_pollution_mode_2"] {
        let mode = building
            .blocks
            .iter()
            .find(|block| block.name == name)
            .unwrap();
        for (start, expected) in [(32_400.0, 3), (32_415.0, 0), (32_385.0, 0)] {
            let inputs = HashMap::from([
                ("operator".into(), json!(1)),
                ("on_duration_s".into(), json!(3600.0)),
                ("local_day_seconds".into(), json!(start)),
                ("pm25".into(), json!(0.0)),
                ("pm25_available".into(), json!(false)),
                ("ozone".into(), json!(0.0)),
                ("ozone_available".into(), json!(false)),
                ("aqi".into(), json!(0.0)),
                ("aqi_available".into(), json!(false)),
            ]);
            let slow = Executor::default()
                .execute(mode, &inputs, 15.0, "")
                .unwrap();
            let fast = CompiledExecutor::try_new(mode)
                .unwrap()
                .unwrap()
                .execute(&inputs, 15.0, false)
                .unwrap()
                .0;
            assert_eq!(slow, fast, "{name} local second {start}");
            assert_eq!(slow["level_three_reminder"], json!(expected));
        }
    }
}

#[test]
fn g36_pollution_limit_equality_and_sensor_loss_follow_2024_thresholds() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    for mode_name in ["outdoor_air_pollution_mode", "outdoor_air_pollution_mode_2"] {
        let mode = building
            .blocks
            .iter()
            .find(|block| block.name == mode_name)
            .unwrap();
        for (sensor, limit, high, low) in [
            ("pm25", 113.0, 114.0, 112.0),
            ("ozone", 0.1, 0.11, 0.09),
            ("aqi", 100.0, 101.0, 99.0),
        ] {
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(mode).unwrap().unwrap();
            let mut inputs = HashMap::from([
                ("operator".into(), json!(2)),
                ("on_duration_s".into(), json!(3600.0)),
                ("local_day_seconds".into(), json!(0.0)),
                ("pm25".into(), json!(0.0)),
                ("pm25_available".into(), json!(false)),
                ("ozone".into(), json!(0.0)),
                ("ozone_available".into(), json!(false)),
                ("aqi".into(), json!(0.0)),
                ("aqi_available".into(), json!(false)),
            ]);
            let availability = format!("{sensor}_available");
            inputs.insert(availability.clone(), json!(true));
            let mut step = |reading: f64, available: bool, expected: bool| {
                inputs.insert(sensor.into(), json!(reading));
                inputs.insert(availability.clone(), json!(available));
                let slow = dynamic.execute(mode, &inputs, 60.0, "").unwrap();
                let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
                assert_eq!(slow, fast, "{sensor} at {reading} available={available}");
                assert_eq!(slow["active"], json!(expected), "{sensor} at {reading}");
                assert_eq!(slow["level_one_alarm"], json!(if expected { 1 } else { 0 }));
            };
            // Section 5.1.17.1(c) says strictly above and strictly below.
            // Equality must hold the current latch state and cannot mature
            // either 30-minute timer.
            for _ in 0..35 {
                step(limit, true, false);
            }
            for minute in 0..=30 {
                step(high, true, minute == 30);
            }
            for _ in 0..35 {
                step(limit, true, true);
            }
            for minute in 0..=30 {
                step(low, true, minute != 30);
            }
            for _ in 0..35 {
                step(limit, true, false);
            }
            for minute in 0..=30 {
                step(high, true, minute == 30);
            }
            step(high, false, false);
            for _ in 0..35 {
                step(high, false, false);
            }
            for minute in 0..=30 {
                step(high, true, minute == 30);
            }
        }
    }
}

#[test]
fn g36_pollution_reminder_fires_when_mode_starts_at_schedule_boundary() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    for mode_name in ["outdoor_air_pollution_mode", "outdoor_air_pollution_mode_2"] {
        let mut mode = building
            .blocks
            .iter()
            .find(|block| block.name == mode_name)
            .unwrap()
            .clone();
        let clock = mode
            .blocks
            .iter_mut()
            .find(|block| block.name == "daily_reminder_clock_time")
            .unwrap();
        clock
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(1800.0);
        let mut inputs = HashMap::from([
            ("operator".into(), json!(2)),
            ("on_duration_s".into(), json!(3600.0)),
            ("local_day_seconds".into(), json!(0.0)),
            ("pm25".into(), json!(114.0)),
            ("pm25_available".into(), json!(true)),
            ("ozone".into(), json!(0.0)),
            ("ozone_available".into(), json!(false)),
            ("aqi".into(), json!(0.0)),
            ("aqi_available".into(), json!(false)),
        ]);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&mode).unwrap().unwrap();
        for minute in 0..33 {
            inputs.insert("local_day_seconds".into(), json!(minute * 60));
            let slow = dynamic.execute(&mode, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(slow, fast, "{mode_name} minute {minute}");
            assert_eq!(slow["active"], json!(minute >= 30));
            assert_eq!(
                slow["level_three_reminder"],
                json!(if minute == 30 { 3 } else { 0 }),
                "{mode_name} minute {minute}"
            );
        }
    }
}

#[test]
fn g36_pollution_combined_2024_transition_trace_is_cdl_driven() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mode = building
        .blocks
        .iter()
        .find(|block| block.name == "outdoor_air_pollution_mode")
        .unwrap()
        .clone();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&mode).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("operator".into(), json!(2)),
        ("on_duration_s".into(), json!(1800.0)),
        ("local_day_seconds".into(), json!(0.0)),
        ("pm25".into(), json!(0.0)),
        ("pm25_available".into(), json!(true)),
        ("ozone".into(), json!(0.0)),
        ("ozone_available".into(), json!(true)),
        ("aqi".into(), json!(0.0)),
        ("aqi_available".into(), json!(false)),
    ]);
    // Minute boundaries are derived from Section 5.1.17: each sensor
    // asserts after 30 continuous minutes above its limit and clears
    // after 30 continuous minutes below. Off overrides an already
    // active sensor; an On selection expires after its selected period.
    for minute in 0..190 {
        let pm25_high = (1..21).contains(&minute) || (22..53).contains(&minute);
        let ozone_high = (41..72).contains(&minute);
        let operator = if (90..95).contains(&minute) || (145..150).contains(&minute) {
            0
        } else if (110..145).contains(&minute) || (150..190).contains(&minute) {
            1
        } else {
            2
        };
        inputs.insert("pm25".into(), json!(if pm25_high { 200.0 } else { 0.0 }));
        inputs.insert("ozone".into(), json!(if ozone_high { 0.2 } else { 0.0 }));
        inputs.insert("operator".into(), json!(operator));
        let expected = (52..90).contains(&minute)
            || (95..102).contains(&minute)
            || (110..140).contains(&minute)
            || (150..180).contains(&minute);
        let slow = dynamic.execute(&mode, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow, fast, "minute {minute}");
        assert_eq!(slow["active"], json!(expected), "minute {minute}");
        assert_eq!(slow["level_one_alarm"], json!(if expected { 1 } else { 0 }));
    }
}

#[test]
fn g36_pollution_three_sensor_overlap_and_operator_switch_follow_2024_addendum() {
    // ASHRAE Guideline 36-2021 Addendum a (2024), 5.1.17.1-2:
    // each provided sensor needs 30 continuous minutes above/below its
    // limit, Off overrides Auto, and timed On is independent of sensors.
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mode = building
        .blocks
        .iter()
        .find(|block| block.name == "outdoor_air_pollution_mode")
        .unwrap();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(mode).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("operator".into(), json!(2)),
        ("on_duration_s".into(), json!(1800.0)),
        ("local_day_seconds".into(), json!(0.0)),
        ("pm25".into(), json!(0.0)),
        ("pm25_available".into(), json!(true)),
        ("ozone".into(), json!(0.0)),
        ("ozone_available".into(), json!(true)),
        ("aqi".into(), json!(0.0)),
        ("aqi_available".into(), json!(true)),
    ]);
    for minute in 0..251 {
        inputs.insert(
            "pm25".into(),
            json!(if (1..70).contains(&minute) {
                200.0
            } else {
                0.0
            }),
        );
        inputs.insert(
            "ozone".into(),
            json!(if (50..110).contains(&minute) {
                0.2
            } else {
                0.0
            }),
        );
        inputs.insert(
            "aqi".into(),
            json!(if (120..180).contains(&minute) {
                150.0
            } else {
                0.0
            }),
        );
        inputs.insert(
            "operator".into(),
            json!(if (100..105).contains(&minute) {
                0
            } else if (210..246).contains(&minute) {
                1
            } else {
                2
            }),
        );
        let auto_active = (31..140).contains(&minute) || (150..210).contains(&minute);
        let expected =
            ((auto_active && !(100..105).contains(&minute)) || (210..240).contains(&minute)) as i32;
        let slow = dynamic.execute(mode, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow, fast, "minute {minute}");
        assert_eq!(slow["active"], json!(expected != 0), "minute {minute}");
        assert_eq!(slow["level_one_alarm"], json!(expected), "minute {minute}");
    }
}

#[test]
fn timed_raw_pm25_event_changes_only_cdl_pollution_decision() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 15.0,
            solar_kw: 0.0,
            internal_kw: 1.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let scenario = json!({
        "step_seconds": 60, "pollution_operator": 2, "outdoor_pm25_ug_m3": 0.0,
        "air_quality_events": [{"pollutant":"pm25","value":150.0,"start_hour":0.0,"duration_hours":0.75}]
    });
    assert!(Simulation::new_test(graph, &weather, r#"{"pollution_on_duration_s":604800}"#).is_ok());
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    let active = "FourPipeG36Controls|output.pollution_active";
    sim.set_traces_json(&json!([active]).to_string()).unwrap();
    let result = run_weather_intervals(&mut sim, 6);
    let rows = result["rows"].as_array().unwrap();
    assert_eq!(rows[0]["traces"][active], json!(0.0));
    assert_eq!(rows[2]["traces"][active], json!(1.0));
    assert_eq!(rows[5]["traces"][active], json!(0.0));
}

#[test]
fn g36_start_fault_graph_compiles() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    assert!(validate(&graph).is_empty());
    assert!(g36_control_contract(&graph).is_empty());
    assert!(sampled_step(&graph, 900).is_ok());
    assert!(CompiledExecutor::try_new(&graph).is_ok());
}
