use super::*;

#[test]
fn hot_water_plant_proof_follows_running_boiler_even_without_thermal_delivery() {
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
    plant["blocks"].as_array_mut().unwrap().push(json!({
        "name": "keep_boiler_running",
        "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
        "category": "elementary",
        "inputs": [],
        "outputs": [{"name": "y", "type": "Boolean"}],
        "parameters": [{"name": "k", "type": "Boolean", "value": true}]
    }));
    let edges = plant["connections"].as_array_mut().unwrap();
    edges.retain(|edge| edge["to"] != "boiler_start_command");
    edges.push(json!({"from": "keep_boiler_running.y", "to": "boiler_start_command"}));
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
    let mut sim = Simulation::new_test(&graph.to_string(), &weather, "{}").unwrap();
    let chunk: Value = serde_json::from_str(&sim.step_chunk_json(1).unwrap()).unwrap();
    assert!(sim.source_running[1]);
    assert!(sim.last_hot_water_plant_proven);
    assert_eq!(chunk["rows"][0]["boiler_used_kw"], json!(0.0));
    assert_eq!(chunk["rows"][0]["hot_water_plant_proven"], json!(1.0));
    let mut dynamic = Simulation::new_test(&graph.to_string(), &weather, "{}").unwrap();
    dynamic.oracle = Some(cdl_runtime_oracle::Executor::default());
    let dynamic_chunk: Value = serde_json::from_str(&dynamic.step_chunk_json(1).unwrap()).unwrap();
    assert_eq!(dynamic_chunk["rows"][0]["boiler_used_kw"], json!(0.0));
    assert_eq!(
        dynamic_chunk["rows"][0]["hot_water_plant_proven"],
        json!(1.0)
    );
}

#[test]
fn physical_vav_faults_change_delivery_and_cdl_reports_leakage() {
    let mut graph_value: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    // Isolate terminal faults from the optional cold-weather economizer mode.
    for ahu in graph_value["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|block| block["name"] == "ahu_1" || block["name"] == "ahu_2")
    {
        ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "economizer_use_teaching_low_limit")
            .unwrap()["parameters"][0]["value"] = json!(true);
    }
    let graph = graph_value.to_string();
    let warm_weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 30.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let window = r#"{"window_openings":[{"zone":1,"start_hour":0,"duration_hours":24}]}"#;
    let stuck = r#"{"window_openings":[{"zone":1,"start_hour":0,"duration_hours":24}],"terminal_faults":[{"zone":1,"kind":"damper_stuck_open","start_hour":0,"duration_hours":24}]}"#;
    let mut normal = Simulation::new_test(&graph, &warm_weather, window).unwrap();
    let mut fault = Simulation::new_test(&graph, &warm_weather, stuck).unwrap();
    for sim in [&mut normal, &mut fault] {
        start_present_window(sim);
        sim.temps = [29.0; 10];
    }
    let damper_alarm = "FourPipeG36Controls|output.zone_01_leaking_damper_alarm_level";
    let damper_command = "FourPipeG36Controls|output.zone_01_damper_position";
    fault
        .set_traces_json(&json!([damper_alarm, damper_command]).to_string())
        .unwrap();
    let base = run_weather_intervals(&mut normal, 2);
    let failed = run_weather_intervals(&mut fault, 2);
    let base_row = &base["rows"][1];
    let fault_row = &failed["rows"][1];
    assert_eq!(fault_row["traces"][damper_command], json!(0.0));
    assert!(base_row["zone_actual_damper_position"][0].as_f64().unwrap() < 1.0);
    assert_eq!(fault_row["zone_actual_damper_position"][0], json!(1.0));
    assert!(
        fault_row["zone_airflow"][0].as_f64().unwrap()
            > base_row["zone_airflow"][0].as_f64().unwrap() + 0.09
    );
    assert_eq!(fault_row["traces"][damper_alarm], json!(4.0));

    // The physical fault and airflow stay the same when only the editable
    // CDL alarm threshold changes. The harness must not set the level.
    let mut raised_threshold: Value = serde_json::from_str(&graph).unwrap();
    let zone = raised_threshold["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "zone_01")
        .unwrap();
    zone["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "sensor_ten_percent")
        .unwrap()["parameters"][0]["value"] = json!(2.0);
    let mut tuned =
        Simulation::new_test(&raised_threshold.to_string(), &warm_weather, stuck).unwrap();
    start_present_window(&mut tuned);
    tuned.temps = [29.0; 10];
    tuned
        .set_traces_json(&json!([damper_alarm, damper_command]).to_string())
        .unwrap();
    let tuned_rows = run_weather_intervals(&mut tuned, 2);
    let tuned_row = &tuned_rows["rows"][1];
    assert_eq!(
        tuned_row["zone_actual_damper_position"][0],
        fault_row["zone_actual_damper_position"][0]
    );
    assert_eq!(tuned_row["zone_airflow"][0], fault_row["zone_airflow"][0]);
    assert_eq!(tuned_row["traces"][damper_command], json!(0.0));
    assert_eq!(tuned_row["traces"][damper_alarm], json!(0.0));

    let cold_weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: -5.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let leak_scenario = r#"{"terminal_faults":[{"zone":1,"kind":"reheat_valve_leak","leak_fraction":0.5,"start_hour":0,"duration_hours":24}],"alarm_acknowledgements":[{"zone":1,"start_hour":8.75,"duration_hours":0.25}]}"#;
    let mut normal = Simulation::new_test(&graph, &cold_weather, "{}").unwrap();
    let mut leak = Simulation::new_test(&graph, &cold_weather, leak_scenario).unwrap();
    for sim in [&mut normal, &mut leak] {
        start_present_window(sim);
        sim.temps = [19.0; 10];
        sim.temps[0] = 24.0;
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
    }
    let valve_alarm = "FourPipeG36Controls|output.zone_01_leaking_reheat_valve_alarm_level";
    let valve_acknowledged =
        "FourPipeG36Controls|output.zone_01_leaking_reheat_valve_alarm_acknowledged";
    let valve_command = "FourPipeG36Controls|output.zone_01_reheat_valve_position";
    let valve_condition = "FourPipeG36Controls.zone_01|leak_valve_condition.y";
    let valve_delay = "FourPipeG36Controls.zone_01|leak_valve_closed_delay.y";
    leak.set_traces_json(
        &json!([
            valve_alarm,
            valve_acknowledged,
            valve_command,
            valve_condition,
            valve_delay
        ])
        .to_string(),
    )
    .unwrap();
    let base = run_weather_intervals(&mut normal, 3);
    let failed = run_weather_intervals(&mut leak, 5);
    let base_row = &base["rows"][2];
    let fault_row = &failed["rows"][2];
    assert_eq!(fault_row["traces"][valve_command], json!(0.0));
    assert_eq!(
        fault_row["zone_actual_reheat_valve_position"][0],
        json!(0.0)
    );
    assert!(
        fault_row["zone_discharge_c"][0].as_f64().unwrap()
            > base_row["zone_discharge_c"][0].as_f64().unwrap() + 3.0,
        "leak discharge {} vs base {}",
        fault_row["zone_discharge_c"][0],
        base_row["zone_discharge_c"][0]
    );
    assert_eq!(fault_row["traces"][valve_condition], json!(1.0));
    assert_eq!(fault_row["traces"][valve_delay], json!(0.0));
    assert_eq!(fault_row["traces"][valve_alarm], json!(0.0));
    assert_eq!(fault_row["traces"][valve_acknowledged], json!(0.0));
    assert_eq!(failed["rows"][4]["traces"][valve_delay], json!(1.0));
    assert_eq!(failed["rows"][4]["traces"][valve_alarm], json!(4.0));
    assert_eq!(failed["rows"][4]["traces"][valve_acknowledged], json!(1.0));

    let biased_scenario = r#"{"terminal_faults":[{"zone":1,"kind":"reheat_valve_leak","leak_fraction":0.5,"start_hour":0,"duration_hours":24}],"terminal_valve_sensor_events":[{"zone":1,"bias":0.8,"start_hour":8,"duration_hours":1}]}"#;
    let mut biased = Simulation::new_test(&graph, &cold_weather, biased_scenario).unwrap();
    start_present_window(&mut biased);
    biased.temps = [19.0; 10];
    biased.temps[0] = 24.0;
    biased.last_heat_request = 100.0;
    biased.last_water_flow_kg_s = [2.0, 2.0];
    biased.last_water_pump_running = [true, true];
    biased
        .set_traces_json(&json!([valve_alarm, valve_command]).to_string())
        .unwrap();
    let biased_rows = run_weather_intervals(&mut biased, 3);
    let biased_row = &biased_rows["rows"][2];
    assert_eq!(
        biased_row["zone_actual_reheat_valve_position"][0],
        fault_row["zone_actual_reheat_valve_position"][0]
    );
    assert_eq!(
        biased_row["zone_discharge_c"][0],
        fault_row["zone_discharge_c"][0]
    );
    assert!(
        biased_row["zone_measured_reheat_valve_position"][0]
            .as_f64()
            .unwrap()
            > 0.7
    );
    assert_eq!(biased_row["traces"][valve_alarm], json!(0.0));
}

#[test]
fn raw_local_override_reaches_ahu_and_zone_physics() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 32.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false
        };
        35_040
    ])
    .unwrap();
    let mut baseline = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut overridden = Simulation::new_test(
        graph,
        &weather,
        r#"{"occupancy_overrides":[{"zone":1,"start_hour":0,"duration_hours":2}]}"#,
    )
    .unwrap();
    baseline.temps = [29.0; 10];
    overridden.temps = [29.0; 10];
    let idle = run_weather_intervals(&mut baseline, 3);
    let active = run_weather_intervals(&mut overridden, 3);
    assert!(
        active["rows"][2]["zone_airflow"][0].as_f64().unwrap()
            > idle["rows"][2]["zone_airflow"][0].as_f64().unwrap(),
        "timed local override should deliver primary air while the raw presence sensor is empty"
    );
    assert!(
        active["rows"][2]["zone_c"][0].as_f64().unwrap()
            < idle["rows"][2]["zone_c"][0].as_f64().unwrap()
    );
}

#[test]
fn occupancy_sensor_events_change_cdl_without_adding_physical_people() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 22.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false,
        };
        35_040
    ])
    .unwrap();
    let mut sim = Simulation::new_test(
        graph,
        &weather,
        r#"{"occupancy_sensor_events":[{"zone":1,"present":true,"start_hour":0,"duration_hours":0.5},{"zone":1,"present":false,"start_hour":0.25,"duration_hours":0.25}]}"#,
    )
    .unwrap();
    let rows = run_weather_intervals(&mut sim, 2);
    assert_eq!(rows["rows"][0]["schedule_occupied"], json!(0.0));
    assert_eq!(rows["rows"][0]["zone_presence_raw"][0], json!(1.0));
    assert_eq!(rows["rows"][0]["zone_occupied"][0], json!(1.0));
    assert_eq!(rows["rows"][0]["zone_occupant_count_actual"][0], json!(0.0));
    assert!((rows["rows"][0]["zone_co2_ppm"][0].as_f64().unwrap() - 420.0).abs() < 1e-8);
    assert_eq!(rows["rows"][1]["zone_presence_raw"][0], json!(0.0));
    assert_eq!(rows["rows"][1]["zone_occupied"][0], json!(0.0));
}

#[test]
fn physical_occupants_remain_present_when_binary_sensor_fails_absent() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
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
    let scenario = json!({
        "step_seconds": 60,
        "occupant_count_events": [{"zone":1,"count":8.0,"start_hour":7.75,"duration_hours":0.25}],
        "occupancy_sensor_events": [{"zone":1,"present":false,"start_hour":7.75,"duration_hours":0.25}],
    });
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    start_present_window(&mut sim);
    sim.temps = [0.0; 10];
    let rows = run_weather_intervals(&mut sim, 1);
    let row = &rows["rows"][0];
    assert_eq!(row["zone_presence_raw"][0], json!(0.0));
    assert_eq!(row["zone_occupant_count_actual"][0], json!(8.0));
    assert!(row["zone_co2_ppm"][0].as_f64().unwrap() > 420.0);
    let summary: serde_json::Value = serde_json::from_str(&sim.summary_json()).unwrap();
    assert!(summary["occupied_unmet_zone_hours"].as_f64().unwrap() >= 0.25);
}

#[test]
fn physical_occupant_count_changes_sensible_heat_without_changing_cdl_inputs() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
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
    let scenario = |heat: f64| {
        json!({
        "step_seconds":60,
        "occupant_sensible_heat_kw_per_person":heat,
        "occupant_count_events":[{"zone":1,"count":8.0,"start_hour":0.0,"duration_hours":1.0}],
        "occupancy_sensor_events":[{"zone":1,"present":false,"start_hour":0.0,"duration_hours":1.0}]
    }).to_string()
    };
    let mut without_heat = Simulation::new_test(graph, &weather, &scenario(0.0)).unwrap();
    let mut with_heat = Simulation::new_test(graph, &weather, &scenario(0.075)).unwrap();
    without_heat.step_chunk_json(1).unwrap();
    with_heat.step_chunk_json(1).unwrap();
    assert_eq!(without_heat.actual_occupant_count[0], 8.0);
    assert_eq!(with_heat.raw_occupancy[0], false);
    assert_eq!(with_heat.occupant_sensible_heat_kw[0], 0.6);
    let expected = 8.0 * 0.075 / (60.0 * 5.0);
    assert!((with_heat.temps[0] - without_heat.temps[0] - expected).abs() < 1e-10);
    for zone in 1..10 {
        assert!((with_heat.temps[zone] - without_heat.temps[zone]).abs() < 1e-10);
    }
    let row: Value = serde_json::from_str(&with_heat.step_chunk_json(14).unwrap()).unwrap();
    assert_eq!(
        row["rows"][0]["zone_occupant_sensible_heat_kw"][0],
        json!(0.6)
    );

    let occupied_weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 22.0,
            solar_kw: 0.0,
            internal_kw: 16.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut synthetic_without = Simulation::new_test(
        graph,
        &occupied_weather,
        r#"{"step_seconds":60,"occupant_sensible_heat_kw_per_person":0}"#,
    )
    .unwrap();
    let mut synthetic_with = Simulation::new_test(
        graph,
        &occupied_weather,
        r#"{"step_seconds":60,"occupant_sensible_heat_kw_per_person":0.075}"#,
    )
    .unwrap();
    synthetic_without.step_chunk_json(1).unwrap();
    synthetic_with.step_chunk_json(1).unwrap();
    for zone in 0..10 {
        assert_eq!(synthetic_with.temps[zone], synthetic_without.temps[zone]);
    }
    let mut increased_design = Simulation::new_test(
        graph,
        &occupied_weather,
        &json!({
            "step_seconds":60,
            "zone_design_occupants":[16,8,8,8,8,8,8,8,8,8],
            "ahu_fan_startup_seconds":[1200,1200]
        })
        .to_string(),
    )
    .unwrap();
    let mut original_design = Simulation::new_test(
        graph,
        &occupied_weather,
        r#"{"step_seconds":60,"ahu_fan_startup_seconds":[1200,1200]}"#,
    )
    .unwrap();
    increased_design.cursor = 32 * (900 / increased_design.effective_step_seconds);
    original_design.cursor = 32 * (900 / original_design.effective_step_seconds);
    increased_design.step_chunk_json(1).unwrap();
    original_design.step_chunk_json(1).unwrap();
    assert_eq!(increased_design.synthetic_occupant_count[0], 8.0);
    assert_eq!(increased_design.actual_occupant_count[0], 16.0);
    assert_eq!(increased_design.occupant_sensible_heat_kw[0], 0.6);
    assert!((increased_design.temps[0] - original_design.temps[0] - expected).abs() < 1e-10);
}

#[test]
fn local_override_button_is_timed_by_editable_cdl() {
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
    let active = "FourPipeG36Controls|output.zone_01_override_active";
    let group = "FourPipeG36Controls|output.ahu_1_group_occupied";
    let run = |graph_json: &str, duration_hours: f64| {
        let scenario = json!({"occupancy_overrides":[{"zone":1,"start_hour":0,"duration_hours":duration_hours}]}).to_string();
        let mut sim = Simulation::new_test(graph_json, &weather, &scenario).unwrap();
        sim.set_traces_json(&json!([active, group]).to_string())
            .unwrap();
        run_weather_intervals(&mut sim, 10)["rows"]
            .as_array()
            .unwrap()
            .clone()
    };
    let short_press = run(graph, 0.25);
    assert_eq!(short_press[0]["traces"][active], json!(1.0));
    assert_eq!(
        short_press[1]["traces"][active],
        json!(1.0),
        "a released button retains the timed occupied request"
    );
    assert_eq!(short_press[7]["traces"][active], json!(1.0));
    assert_eq!(short_press[8]["traces"][active], json!(0.0));
    assert_eq!(short_press[8]["traces"][group], json!(0.0));
    let held_button = run(graph, 3.0);
    assert_eq!(
        held_button[8]["traces"][active],
        json!(0.0),
        "a held button cannot extend one episode past two hours"
    );
    let mut tuned: Value = serde_json::from_str(graph).unwrap();
    tuned["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "zone_01_override_hold")
        .unwrap()["parameters"][0]["value"] = json!(3600.0);
    let tuned_rows = run(&tuned.to_string(), 0.25);
    assert_eq!(tuned_rows[3]["traces"][active], json!(1.0));
    assert_eq!(
        tuned_rows[4]["traces"][active],
        json!(0.0),
        "editing the CDL duration shortens the override"
    );
}

#[test]
fn cdl_group_setback_changes_physical_heating_and_zone_temperature() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 0.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false
        };
        35_040
    ])
    .unwrap();
    let mut default = Simulation::new_test(graph, &weather, "{}").unwrap();
    default.temps = [19.0; 10];
    let normal = run_weather_intervals(&mut default, 4);
    let mut changed: Value = serde_json::from_str(graph).unwrap();
    for zone in changed["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|block| block["block_type"] == "VAVBoxController")
    {
        zone["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "unoccupied_heat")
            .unwrap()["parameters"][0]["value"] = json!(20.0);
    }
    let mut tuned = Simulation::new_test(&changed.to_string(), &weather, "{}").unwrap();
    tuned.temps = [19.0; 10];
    let setback = run_weather_intervals(&mut tuned, 4);
    assert!(
        setback["rows"][3]["heat_request_kw"].as_f64().unwrap()
            > normal["rows"][3]["heat_request_kw"].as_f64().unwrap() + 1.0
    );
    assert!(
        setback["rows"][3]["avg_zone_c"].as_f64().unwrap()
            > normal["rows"][3]["avg_zone_c"].as_f64().unwrap() + 0.1
    );
}

#[test]
fn cdl_warmup_mode_changes_physical_zone_temperature() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let mut weather = vec![
        Weather {
            outdoor_c: -5.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false
        };
        35_040
    ];
    for sample in &mut weather[87..] {
        sample.occupied = true;
    }
    let weather = serde_json::to_string(&weather).unwrap();
    let mut without: Value = serde_json::from_str(graph).unwrap();
    without["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "preconditioning_horizon")
        .unwrap()["parameters"][0]["value"] = json!(1.0);
    let mut active = Simulation::new_test(graph, &weather, r#"{"step_seconds":60}"#).unwrap();
    let mut inactive =
        Simulation::new_test(&without.to_string(), &weather, r#"{"step_seconds":60}"#).unwrap();
    for sim in [&mut active, &mut inactive] {
        run_weather_intervals(sim, 80);
        sim.temps = [19.0; 10];
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
    }
    let warm = run_weather_intervals(&mut active, 8);
    let coast = run_weather_intervals(&mut inactive, 8);
    assert_eq!(warm["rows"][0]["preconditioning_mode"], json!(1.0));
    assert_eq!(coast["rows"][0]["preconditioning_mode"], json!(0.0));
    let warm_temp = warm["rows"][7]["avg_zone_c"].as_f64().unwrap();
    let coast_temp = coast["rows"][7]["avg_zone_c"].as_f64().unwrap();
    assert!(
        warm_temp > coast_temp + 0.2,
        "warm-up {warm_temp} °C, coasting {coast_temp} °C"
    );
}

#[test]
fn g36_heating_damper_gain_changes_delivered_airflow() {
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
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    normal.temps = [15.0; 10];
    let open = run_weather_intervals(&mut normal, 3);
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    for zone in edited["blocks"].as_array_mut().unwrap() {
        if zone["block_type"] != "VAVBoxController" {
            continue;
        }
        zone["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "heating_damper_gain")
            .unwrap()["parameters"][0]["value"] = json!(0.0);
    }
    let mut minimum = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    minimum.temps = [15.0; 10];
    let held = run_weather_intervals(&mut minimum, 3);
    assert!(
        open["rows"][2]["delivered_airflow"].as_f64().unwrap()
            > held["rows"][2]["delivered_airflow"].as_f64().unwrap()
    );
}

#[test]
fn g36_discharge_limit_reduces_reheat_energy_with_airflow_feedback() {
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    for zone in graph["blocks"].as_array_mut().unwrap() {
        if zone["block_type"] != "VAVBoxController" {
            continue;
        }
        zone["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "ventilation_standby_permitted")
            .unwrap()["parameters"][0]["value"] = json!(false);
    }
    let graph = graph.to_string();
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
    let scenario =
        r#"{"heat_pump_capacity_kw":1000,"boiler_capacity_kw":1000,"chiller_capacity_kw":1000}"#;
    let mut normal = Simulation::new_test(&graph, &weather, scenario).unwrap();
    normal.temps = [15.0; 10];
    normal.last_zone_airflow = [0.4; 10];
    normal.last_heat_request = 200.0;
    let base = run_weather_intervals(&mut normal, 10);
    let mut dynamic = Simulation::new_test(&graph, &weather, scenario).unwrap();
    dynamic.oracle = Some(cdl_runtime_oracle::Executor::default());
    dynamic.temps = [15.0; 10];
    dynamic.last_zone_airflow = [0.4; 10];
    dynamic.last_heat_request = 200.0;
    let dynamic_rows = run_weather_intervals(&mut dynamic, 10);
    assert_eq!(
        base["rows"][9]["zone_discharge_c"],
        dynamic_rows["rows"][9]["zone_discharge_c"]
    );
    let mut edited: Value = serde_json::from_str(&graph).unwrap();
    for zone in edited["blocks"].as_array_mut().unwrap() {
        if zone["block_type"] != "VAVBoxController" {
            continue;
        }
        let limit = zone["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "maximum_discharge_temp")
            .unwrap();
        limit["parameters"][0]["value"] = json!(18.0);
    }
    let mut restricted = Simulation::new_test(&edited.to_string(), &weather, scenario).unwrap();
    restricted.temps = [15.0; 10];
    restricted.last_zone_airflow = [0.4; 10];
    restricted.last_heat_request = 200.0;
    let low = run_weather_intervals(&mut restricted, 10);
    let base_row = &base["rows"][9];
    let low_row = &low["rows"][9];
    let reheat_energy = |row: &Value| {
        (row["zone_discharge_c"][0].as_f64().unwrap()
            - row["ahu_1_supply_actual_c"].as_f64().unwrap())
            * 1.2
            * row["zone_airflow"][0].as_f64().unwrap()
    };
    // Discharge temperature alone is not monotonic when the edited valve
    // also changes airflow. Compare the delivered sensible heat instead.
    assert!(
        low_row["zone_actual_reheat_valve_position"][0]
            .as_f64()
            .unwrap()
            < base_row["zone_actual_reheat_valve_position"][0]
                .as_f64()
                .unwrap()
                - 0.2
    );
    assert!(
        reheat_energy(base_row) > reheat_energy(low_row) + 2.0,
        "base reheat={} kW, limited reheat={} kW",
        reheat_energy(base_row),
        reheat_energy(low_row)
    );
    assert!(
        low_row["zone_discharge_c"][0].as_f64().unwrap()
            >= low_row["ahu_1_supply_actual_c"].as_f64().unwrap() - 0.1
    );
    for row in [base_row, low_row] {
        let total: f64 = row["zone_airflow"]
            .as_array()
            .unwrap()
            .iter()
            .map(|flow| flow.as_f64().unwrap())
            .sum();
        assert!((total - row["delivered_airflow"].as_f64().unwrap()).abs() < 1e-9);
    }
}

#[test]
fn g36_discharge_feedback_gain_changes_physical_reheat() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 0.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let scenario = r#"{"heat_pump_capacity_kw":1000,"boiler_capacity_kw":1000}"#;
    let mut normal = Simulation::new_test(graph, &weather, scenario).unwrap();
    normal.temps = [15.0; 10];
    normal.last_zone_airflow = [0.4; 10];
    normal.last_heat_request = 200.0;
    let base = run_weather_intervals(&mut normal, 10);
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    for zone in edited["blocks"].as_array_mut().unwrap() {
        if zone["block_type"] != "VAVBoxController" {
            continue;
        }
        zone["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "discharge_valve_pi")
            .unwrap()["parameters"][0]["value"] = json!(1e-6);
    }
    let mut disabled = Simulation::new_test(&edited.to_string(), &weather, scenario).unwrap();
    disabled.temps = [15.0; 10];
    disabled.last_zone_airflow = [0.4; 10];
    disabled.last_heat_request = 200.0;
    let no_feedback = run_weather_intervals(&mut disabled, 10);
    assert!(
        base["rows"][9]["zone_discharge_c"][0].as_f64().unwrap()
            > no_feedback["rows"][9]["zone_discharge_c"][0]
                .as_f64()
                .unwrap()
                + 1.0,
        "base discharge {}, zero-gain discharge {}, base flow {}, zero-gain flow {}",
        base["rows"][9]["zone_discharge_c"][0],
        no_feedback["rows"][9]["zone_discharge_c"][0],
        base["rows"][9]["zone_airflow"][0],
        no_feedback["rows"][9]["zone_airflow"][0]
    );
    assert!(
        base["rows"][9]["zone_c"][0].as_f64().unwrap()
            > no_feedback["rows"][9]["zone_c"][0].as_f64().unwrap()
    );
}

#[test]
fn g36_outdoor_air_command_changes_mixing_and_coil_load() {
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
    let mut free_cooling = Simulation::new_test(graph, &weather, "{}").unwrap();
    start_present_window(&mut free_cooling);
    let free = run_weather_intervals(&mut free_cooling, 3);
    let mut fixed: Value = serde_json::from_str(graph).unwrap();
    for ahu in 1..=2 {
        let controller = fixed["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == format!("ahu_{ahu}"))
            .unwrap();
        let outdoor = controller["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "economizer_mixed_air_pi")
            .unwrap();
        outdoor["parameters"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|parameter| parameter["name"] == "yMax")
            .unwrap()["value"] = json!(0.2);
    }
    let mut minimum_only = Simulation::new_test(&fixed.to_string(), &weather, "{}").unwrap();
    start_present_window(&mut minimum_only);
    let minimum = run_weather_intervals(&mut minimum_only, 3);
    let free_row = &free["rows"][2];
    let minimum_row = &minimum["rows"][2];
    assert!(free_row["ahu_1_outdoor_air_fraction"].as_f64().unwrap() > 0.2);
    assert!(minimum_row["ahu_1_outdoor_air_fraction"].as_f64().unwrap() <= 0.2 + 1e-9);
    assert!(
        free_row["ahu_1_mixed_air_c"].as_f64().unwrap()
            < minimum_row["ahu_1_mixed_air_c"].as_f64().unwrap()
    );
    assert!(
        free_row["cool_request_kw"].as_f64().unwrap()
            < minimum_row["cool_request_kw"].as_f64().unwrap()
    );
}

#[test]
fn mixed_air_sensor_bias_reaches_cdl_without_directly_changing_physical_mixing() {
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
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut biased = Simulation::new_test(graph, &weather,
        r#"{"ahu_mixed_air_sensor_events":[{"ahu":1,"bias_c":5,"start_hour":8,"duration_hours":2}]}"#).unwrap();
    let damper = "FourPipeG36Controls|output.ahu_1_outdoor_air_damper_position";
    for sim in [&mut normal, &mut biased] {
        sim.temps = [24.0; 10];
        sim.cursor = 32 * 900 / sim.effective_step_seconds;
        sim.set_traces_json(&json!([damper]).to_string()).unwrap();
    }
    let ordinary = run_weather_intervals(&mut normal, 4);
    let fault = run_weather_intervals(&mut biased, 4);
    let first_normal = &ordinary["rows"][0];
    let first_fault = &fault["rows"][0];
    assert!(
        (first_fault["ahu_1_measured_mixed_air_c"].as_f64().unwrap()
            - first_fault["ahu_1_mixed_air_c"].as_f64().unwrap()
            - 5.0)
            .abs()
            < 1e-9
    );
    assert!(
        (first_normal["traces"][damper].as_f64().unwrap()
            - first_fault["traces"][damper].as_f64().unwrap())
        .abs()
            > 1e-4,
        "normal={first_normal} biased={first_fault}"
    );
}

#[test]
fn g36_economizer_modulates_from_measured_mixed_air() {
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
        ("duct_pressure".into(), json!(0.4)),
        ("outdoor_airflow".into(), json!(0.3)),
        ("mixed_air_temp".into(), json!(20.0)),
        ("actual_supply_air_temp".into(), json!(16.0)),
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
    let mut dynamic_high = Executor::default();
    let mut high = 0.0;
    for _ in 0..20 {
        high = dynamic_high.execute(ahu, &inputs, 60.0, "").unwrap()["outdoor_air_fraction"]
            .as_f64()
            .unwrap();
    }
    inputs.insert("mixed_air_temp".into(), json!(15.0));
    let mut dynamic_low = Executor::default();
    let mut low = 0.0;
    let mut fast = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    let mut fast_low = HashMap::new();
    for _ in 0..20 {
        low = dynamic_low.execute(ahu, &inputs, 60.0, "").unwrap()["outdoor_air_fraction"]
            .as_f64()
            .unwrap();
        fast_low = fast.execute(&inputs, 60.0, false).unwrap().0;
    }
    assert!(
        high > low + 0.1,
        "mixed-air PI should modulate: {high} > {low}"
    );
    assert!((fast_low["outdoor_air_fraction"].as_f64().unwrap() - low).abs() < 1e-9);
}

#[test]
fn g36_economizer_high_limit_requires_fan_proof_and_no_freeze() {
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
    let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    for (outdoor, fan, stat, expected) in [
        (22.0, true, false, true),
        (23.5, true, false, true),
        (24.1, true, false, true),
        (23.5, true, false, true),
        (22.9, true, false, true),
        (22.9, false, false, false),
        (22.9, true, false, true),
        (22.9, true, true, false),
        (22.9, true, false, false),
    ] {
        inputs.insert("outdoor_c".into(), json!(outdoor));
        inputs.insert("supply_fan_proven".into(), json!(fan));
        inputs.insert("freeze_stat_trip".into(), json!(stat));
        let actual = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(actual["economizer_mode_enabled"], json!(expected));
        assert_eq!(fast["economizer_mode_enabled"], json!(expected));
    }
    let mut tuned = ahu.clone();
    tuned
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_high_limit_hysteresis")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "uLow")
        .unwrap()
        .value = json!(-2.0);
    for parameter in &mut tuned
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_high_limit_hold")
        .unwrap()
        .parameters
    {
        parameter.value = json!(0.0);
    }
    let mut executor = Executor::default();
    inputs.insert("freeze_stat_trip".into(), json!(false));
    inputs.insert("outdoor_c".into(), json!(22.9));
    assert_eq!(
        executor.execute(&tuned, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
        json!(true)
    );
    inputs.insert("outdoor_c".into(), json!(24.1));
    assert_eq!(
        executor.execute(&tuned, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
        json!(false)
    );
    inputs.insert("outdoor_c".into(), json!(22.9));
    assert_eq!(
        executor.execute(&tuned, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
        json!(false)
    );
    // OBC's economizer high-limit switch holds each state for 600 s;
    // fan and freeze proof remain separate immediate safety gates.
    let mut held_dynamic = Executor::default();
    let mut held_compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
    inputs.insert("outdoor_c".into(), json!(22.0));
    for _ in 0..4 {
        assert_eq!(
            held_dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
            json!(true)
        );
        assert_eq!(
            held_compiled.execute(&inputs, 60.0, false).unwrap().0["economizer_mode_enabled"],
            json!(true)
        );
    }
    inputs.insert("outdoor_c".into(), json!(24.1));
    for step in 0..7 {
        let expected = step < 6;
        assert_eq!(
            held_dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
            json!(expected)
        );
        assert_eq!(
            held_compiled.execute(&inputs, 60.0, false).unwrap().0["economizer_mode_enabled"],
            json!(expected)
        );
    }
    inputs.insert("outdoor_c".into(), json!(22.0));
    for step in 0..10 {
        let expected = step == 9;
        assert_eq!(
            held_dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
            json!(expected)
        );
        assert_eq!(
            held_compiled.execute(&inputs, 60.0, false).unwrap().0["economizer_mode_enabled"],
            json!(expected)
        );
    }
}

#[test]
fn g36_fixed_dry_bulb_selection_is_a_typed_cdl_decision() {
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
    let mut differential = ahu.clone();
    let hold = differential
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_high_limit_hold")
        .unwrap();
    for parameter in &mut hold.parameters {
        parameter.value = json!(0.0);
    }
    let mut fixed = differential.clone();
    fixed
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_use_fixed_dry_bulb")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "k")
        .unwrap()
        .value = json!(true);
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(24.5)),
        ("flow_request".into(), json!(5.0)),
        ("return_air_temp".into(), json!(29.0)),
        ("mixed_air_temp".into(), json!(25.0)),
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
    let mut outdoor_commands = Vec::new();
    for (graph, expected) in [(&differential, true), (&fixed, false)] {
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(graph).unwrap().unwrap();
        let mut slow = HashMap::new();
        let mut fast = HashMap::new();
        for _ in 0..60 {
            slow = dynamic.execute(graph, &inputs, 15.0, "").unwrap();
            fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
        }
        assert_eq!(slow["economizer_mode_enabled"], json!(expected));
        assert_eq!(fast["economizer_mode_enabled"], json!(expected));
        outdoor_commands.push(slow["outdoor_air_damper_position"].as_f64().unwrap());
    }
    assert!(outdoor_commands[0] > outdoor_commands[1]);
}

#[test]
fn g36_combined_dry_bulb_requires_both_high_limits_in_editable_cdl() {
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
    for parameter in &mut ahu
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_high_limit_hold")
        .unwrap()
        .parameters
    {
        parameter.value = json!(0.0);
    }
    ahu.blocks
        .iter_mut()
        .find(|block| block.name == "economizer_use_combined_dry_bulb")
        .unwrap()
        .parameters[0]
        .value = json!(true);
    let mut inputs = HashMap::from([
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
    let mut outdoor_commands = Vec::new();
    for (outdoor, return_air, expected) in [
        (24.5, 29.0, false), // differential permits, fixed rejects
        (23.0, 22.0, false), // fixed permits, differential rejects
        (21.0, 29.0, true),  // both permit
    ] {
        inputs.insert("outdoor_c".into(), json!(outdoor));
        inputs.insert("return_air_temp".into(), json!(return_air));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&ahu).unwrap().unwrap();
        let mut slow = HashMap::new();
        let mut fast = HashMap::new();
        for _ in 0..60 {
            slow = dynamic.execute(&ahu, &inputs, 15.0, "").unwrap();
            fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
        }
        assert_eq!(
            slow["economizer_mode_enabled"],
            json!(expected),
            "outdoor={outdoor}, return={return_air}"
        );
        assert_eq!(
            fast["economizer_mode_enabled"],
            json!(expected),
            "outdoor={outdoor}, return={return_air}"
        );
        outdoor_commands.push(slow["outdoor_air_damper_position"].as_f64().unwrap());
    }
    assert!(outdoor_commands[2] > outdoor_commands[0]);
}

#[test]
fn g36_enthalpy_device_trips_only_when_selected_in_cdl() {
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
    assert!(validate(&building).is_empty());
    for parameter in &mut ahu
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_high_limit_hold")
        .unwrap()
        .parameters
    {
        parameter.value = json!(0.0);
    }
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(21.0)),
        ("return_air_temp".into(), json!(27.0)),
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
    for (selected, enthalpy, expected) in
        [(false, 70.0, true), (true, 70.0, false), (true, 64.0, true)]
    {
        ahu.blocks
            .iter_mut()
            .find(|block| block.name == "economizer_use_enthalpy_device")
            .unwrap()
            .parameters[0]
            .value = json!(selected);
        inputs.insert("outdoor_air_enthalpy_kj_kg".into(), json!(enthalpy));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&ahu).unwrap().unwrap();
        for _ in 0..60 {
            let slow = dynamic.execute(&ahu, &inputs, 15.0, "").unwrap();
            let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
            assert_eq!(
                slow["economizer_mode_enabled"],
                fast["economizer_mode_enabled"]
            );
        }
        assert_eq!(
            dynamic.execute(&ahu, &inputs, 15.0, "").unwrap()["economizer_mode_enabled"],
            json!(expected)
        );
    }
    assert!(outdoor_air_enthalpy_kj_kg(21.0, 0.9) > outdoor_air_enthalpy_kj_kg(21.0, 0.1));
}

#[test]
fn g36_differential_enthalpy_device_uses_return_input_in_both_executors() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    assert!(validate(&building).is_empty());
    let mut ahu = building
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1")
        .unwrap()
        .clone();
    ahu.blocks
        .iter_mut()
        .find(|block| block.name == "economizer_use_differential_enthalpy_device")
        .unwrap()
        .parameters[0]
        .value = json!(true);
    for parameter in &mut ahu
        .blocks
        .iter_mut()
        .find(|block| block.name == "economizer_high_limit_hold")
        .unwrap()
        .parameters
    {
        parameter.value = json!(0.0);
    }
    let mut inputs = HashMap::from([
        ("outdoor_c".into(), json!(21.0)),
        ("outdoor_air_enthalpy_kj_kg".into(), json!(60.0)),
        ("return_air_enthalpy_kj_kg".into(), json!(70.0)),
        ("return_air_temp".into(), json!(27.0)),
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
    for (outdoor_c, return_enthalpy, expected) in
        [(21.0, 70.0, true), (21.0, 50.0, false), (25.0, 70.0, false)]
    {
        inputs.insert("outdoor_c".into(), json!(outdoor_c));
        inputs.insert("return_air_enthalpy_kj_kg".into(), json!(return_enthalpy));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&ahu).unwrap().unwrap();
        for _ in 0..60 {
            let slow = dynamic.execute(&ahu, &inputs, 15.0, "").unwrap();
            let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
            assert_eq!(
                slow["economizer_mode_enabled"],
                fast["economizer_mode_enabled"]
            );
        }
        assert_eq!(
            dynamic.execute(&ahu, &inputs, 15.0, "").unwrap()["economizer_mode_enabled"],
            json!(expected)
        );
    }
    ahu.blocks
        .iter_mut()
        .find(|block| block.name == "economizer_differential_enthalpy_hysteresis")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "uHigh")
        .unwrap()
        .value = json!(2.0);
    inputs.insert("outdoor_c".into(), json!(21.0));
    inputs.insert("return_air_enthalpy_kj_kg".into(), json!(59.0));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&ahu).unwrap().unwrap();
    for _ in 0..60 {
        let slow = dynamic.execute(&ahu, &inputs, 15.0, "").unwrap();
        let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
        assert_eq!(
            slow["economizer_mode_enabled"],
            fast["economizer_mode_enabled"]
        );
    }
    assert_eq!(
        dynamic.execute(&ahu, &inputs, 15.0, "").unwrap()["economizer_mode_enabled"],
        json!(true)
    );
}

#[test]
fn quarter_hour_humidity_and_override_supply_raw_cdl_enthalpy_input() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let mut weather = serde_json::to_value(vec![
        Weather {
            outdoor_c: 23.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    weather[0]["relative_humidity"] = json!(0.9);
    let weather = weather.to_string();
    for (scenario, humidity) in [
        (json!({}), 0.9),
        (json!({"outdoor_relative_humidity": 0.1}), 0.1),
    ] {
        let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
        let trace = "FourPipeG36Controls|outdoor_air_enthalpy_kj_kg";
        sim.set_traces_json(&json!([trace]).to_string()).unwrap();
        let chunk = run_weather_intervals(&mut sim, 1);
        let actual = chunk["rows"][0]["traces"][trace].as_f64().unwrap();
        let expected = outdoor_air_enthalpy_kj_kg(23.0, humidity);
        assert!((actual - expected).abs() < 1e-9);
    }
}

#[test]
fn saturated_zone_records_condensate_and_warms_relative_to_dry_zone() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 16.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false,
        };
        35_040
    ])
    .unwrap();
    let scenario = r#"{"step_seconds":60}"#;
    let mut wet = Simulation::new_test(graph, &weather, scenario).unwrap();
    wet.scenario.terminal_design_flow = [0.0; 10];
    wet.scenario.zone_infiltration_m3_s = 0.0;
    wet.temps = [16.0; 10];
    wet.zone_humidity_ratio = [humidity_ratio(25.0, 0.8); 10];
    let mut dry = Simulation::new_test(graph, &weather, scenario).unwrap();
    dry.scenario.terminal_design_flow = [0.0; 10];
    dry.scenario.zone_infiltration_m3_s = 0.0;
    dry.temps = [16.0; 10];
    dry.zone_humidity_ratio = [humidity_ratio(16.0, 0.5); 10];
    let wet_row: Value = serde_json::from_str(&wet.step_chunk_json(15).unwrap()).unwrap();
    let dry_row: Value = serde_json::from_str(&dry.step_chunk_json(15).unwrap()).unwrap();
    let wet_row = &wet_row["rows"][0];
    let dry_row = &dry_row["rows"][0];
    assert!(wet_row["zone_condensate_kg_s"][0].as_f64().unwrap() > 0.0);
    assert!(wet_row["zone_condensation_heat_kw"][0].as_f64().unwrap() > 0.0);
    assert!(wet_row["zone_relative_humidity"][0].as_f64().unwrap() <= 1.0 + 1e-9);
    assert!(wet_row["zone_c"][0].as_f64().unwrap() > dry_row["zone_c"][0].as_f64().unwrap());
    assert_eq!(dry_row["zone_condensate_kg_s"][0], json!(0.0));
    let mut split = Simulation::new_test(graph, &weather, scenario).unwrap();
    split.scenario.terminal_design_flow = [0.0; 10];
    split.scenario.zone_infiltration_m3_s = 0.0;
    split.temps = [16.0; 10];
    split.zone_humidity_ratio = [humidity_ratio(25.0, 0.8); 10];
    let first: Value = serde_json::from_str(&split.step_chunk_json(7).unwrap()).unwrap();
    assert!(first["rows"].as_array().unwrap().is_empty());
    let second: Value = serde_json::from_str(&split.step_chunk_json(8).unwrap()).unwrap();
    assert_eq!(
        second["rows"][0]["zone_condensate_kg_s"][0],
        wet_row["zone_condensate_kg_s"][0]
    );
    assert_eq!(
        second["rows"][0]["zone_condensation_heat_kw"][0],
        wet_row["zone_condensation_heat_kw"][0]
    );
}

#[test]
fn humid_weather_produces_physical_condensate_on_a_water_limited_coil() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = serde_json::to_string(&vec![
        json!({"outdoor_c": 35.0, "relative_humidity": 0.9,
            "solar_kw": 0.0, "internal_kw": 0.0, "occupied": true});
        35_040
    ])
    .unwrap();
    let mut sim = Simulation::new_test(graph, &weather, "{}").unwrap();
    sim.temps = [28.0; 10];
    let rows = run_weather_intervals(&mut sim, 24);
    let wet = rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["ahu_1_cooling_coil_condensate_kg_s"].as_f64().unwrap() > 0.0)
        .expect("humid weather and a cooling coil should produce condensate");
    assert_coil_air_balance(wet, 1);
    assert!(wet["ahu_1_cooling_coil_delivered_kw"].as_f64().unwrap() > 0.0);
    let total_coil_kw = wet["ahu_1_cooling_coil_delivered_kw"].as_f64().unwrap()
        + wet["ahu_2_cooling_coil_delivered_kw"].as_f64().unwrap();
    assert!((wet["cool_kw"].as_f64().unwrap() - total_coil_kw).abs() < 1e-9);
    assert!(wet["ahu_1_supply_air_relative_humidity"].as_f64().unwrap() <= 1.0 + 1e-8);
}

#[test]
fn bundled_weather_exercises_enthalpy_limit_below_dry_bulb_limit() {
    let weather: Vec<WeatherRecord> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/weather_2025.json"
    )))
    .unwrap();
    assert_eq!(weather.len(), 35_040);
    let humid_below_dry_bulb = weather
        .iter()
        .filter(|record| {
            let humidity = record.relative_humidity.unwrap();
            record.weather.outdoor_c < 24.0
                && outdoor_air_enthalpy_kj_kg(record.weather.outdoor_c, humidity) > 66.0
        })
        .count();
    assert!(humid_below_dry_bulb > 0);
}

#[test]
fn building_economizer_follows_cdl_enthalpy_selection_and_raw_humidity() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        json!({
            "outdoor_c": 23.5, "relative_humidity": 0.95,
            "solar_kw": 0.0, "internal_kw": 0.0, "occupied": true,
        });
        35_040
    ])
    .unwrap();
    let mut enthalpy_graph: Value = serde_json::from_str(graph).unwrap();
    for ahu in enthalpy_graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|block| block["name"] == "ahu_1" || block["name"] == "ahu_2")
    {
        ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "economizer_use_enthalpy_device")
            .unwrap()["parameters"][0]["value"] = json!(true);
    }
    let enthalpy_graph = enthalpy_graph.to_string();
    let trace = "FourPipeG36Controls|output.ahu_1_economizer_mode_enabled";
    let mut results = Vec::new();
    for (graph, scenario) in [
        (enthalpy_graph.as_str(), "{}"),
        (
            enthalpy_graph.as_str(),
            r#"{"outdoor_relative_humidity":0.2}"#,
        ),
        (graph, "{}"),
    ] {
        let mut sim = Simulation::new_test(graph, &weather, scenario).unwrap();
        start_present_window(&mut sim);
        sim.temps = [27.0; 10];
        sim.set_traces_json(&json!([trace]).to_string()).unwrap();
        let rows = run_weather_intervals(&mut sim, 4);
        results.push(rows["rows"][3]["traces"][trace].clone());
    }
    assert_eq!(results, vec![json!(0.0), json!(1.0), json!(1.0)]);
}

#[test]
fn building_differential_enthalpy_uses_raw_return_humidity_without_harness_policy() {
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    for ahu in graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|block| block["name"] == "ahu_1" || block["name"] == "ahu_2")
    {
        ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "economizer_use_differential_enthalpy_device")
            .unwrap()["parameters"][0]["value"] = json!(true);
    }
    let weather = serde_json::to_string(&vec![
        json!({
            "outdoor_c": 23.5, "relative_humidity": 0.7,
            "solar_kw": 0.0, "internal_kw": 0.0, "occupied": true,
        });
        35_040
    ])
    .unwrap();
    let mode = "FourPipeG36Controls|output.ahu_1_economizer_mode_enabled";
    let return_enthalpy = "FourPipeG36Controls|ahu_1_return_air_enthalpy_kj_kg";
    let mut results = Vec::new();
    for return_rh in [0.8, 0.2] {
        let mut sim = Simulation::new_test(
            &graph.to_string(),
            &weather,
            &json!({"return_air_relative_humidity":return_rh}).to_string(),
        )
        .unwrap();
        start_present_window(&mut sim);
        sim.temps = [27.0; 10];
        sim.set_traces_json(&json!([mode, return_enthalpy]).to_string())
            .unwrap();
        let rows = run_weather_intervals(&mut sim, 4);
        results.push(rows["rows"][3].clone());
    }
    assert!(
        results[0]["traces"][return_enthalpy].as_f64().unwrap()
            > results[1]["traces"][return_enthalpy].as_f64().unwrap()
    );
    assert_eq!(results[0]["traces"][mode], json!(1.0));
    assert_eq!(results[1]["traces"][mode], json!(0.0));
}

#[test]
fn modeled_return_enthalpy_responds_to_outdoor_moisture_and_preserves_fixed_override() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        json!({"outdoor_c": 24.0, "relative_humidity": 0.5,
            "solar_kw": 0.0, "internal_kw": 0.0, "occupied": true});
        35_040
    ])
    .unwrap();
    let trace = "FourPipeG36Controls|ahu_1_return_air_enthalpy_kj_kg";
    let return_temp_trace = "FourPipeG36Controls|ahu_1_return_air_temp";
    let mut modeled = Vec::new();
    for outdoor_rh in [0.1, 0.9] {
        let scenario = json!({
            "outdoor_relative_humidity": outdoor_rh,
            "window_openings": [{"zone": 1, "start_hour": 0, "duration_hours": 3}],
            "open_window_additional_infiltration_m3_s": 0.5
        });
        let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
        sim.set_traces_json(&json!([trace, return_temp_trace]).to_string())
            .unwrap();
        let rows = run_weather_intervals(&mut sim, 4);
        modeled.push(rows["rows"][3].clone());
    }
    let low = modeled[0]["traces"][trace].as_f64().unwrap();
    let high = modeled[1]["traces"][trace].as_f64().unwrap();
    assert!(
        high > low + 0.2,
        "outdoor moisture must reach the return-air CDL input: {low} vs {high}"
    );
    assert!(
        modeled[1]["zone_relative_humidity"][0].as_f64().unwrap()
            > modeled[0]["zone_relative_humidity"][0].as_f64().unwrap()
    );
    for row in &modeled {
        assert!(row["traces"][trace].as_f64().unwrap().is_finite());
    }

    let mut fixed =
        Simulation::new_test(graph, &weather, r#"{"return_air_relative_humidity":0.5}"#).unwrap();
    fixed
        .set_traces_json(&json!([trace, "FourPipeG36Controls|ahu_1_return_air_temp"]).to_string())
        .unwrap();
    let rows = run_weather_intervals(&mut fixed, 4);
    let row = &rows["rows"][3];
    let return_c = row["traces"]["FourPipeG36Controls|ahu_1_return_air_temp"]
        .as_f64()
        .unwrap();
    assert!(
        (row["traces"][trace].as_f64().unwrap() - outdoor_air_enthalpy_kj_kg(return_c, 0.5)).abs()
            < 1e-9
    );
}

#[test]
fn ahu_return_temperature_uses_delivered_airflow_and_reaches_cdl() {
    let temperatures = [10.0, 30.0, 20.0, 20.0, 20.0, 22.0, 22.0, 22.0, 22.0, 22.0];
    let flow = [0.1, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    assert!((ahu_return_value(&temperatures, &flow, 0) - 28.0).abs() < 1e-12);
    assert!((ahu_return_value(&temperatures, &[0.0; 10], 0) - 20.0).abs() < 1e-12);

    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        json!({"outdoor_c":28.0,"solar_kw":0.0,"internal_kw":100.0,"occupied":true});
        35_040
    ])
    .unwrap();
    let mut sim =
        Simulation::new_test(graph, &weather, r#"{"return_air_relative_humidity":0.5}"#).unwrap();
    assert_eq!(sim.effective_step_seconds(), 15);
    // Begin at a scheduled, physically present hour. The graph still
    // starts from its own initial state and determines the fan command.
    sim.cursor = 40 * 60;
    sim.step_chunk_json(59).unwrap();
    sim.temps = temperatures;
    sim.last_zone_airflow = flow;
    let return_humidity = sim.zone_humidity_ratio;
    sim.scenario.terminal_design_flow[..5].copy_from_slice(&[0.05, 5.0, 0.05, 0.05, 0.05]);
    sim.scenario.terminal_faults.push(TerminalFault {
        zone: 2,
        kind: "damper_stuck_open".into(),
        leak_fraction: None,
        start_hour: 0.0,
        duration_hours: 24.0,
    });
    let temp_trace = "FourPipeG36Controls|ahu_1_return_air_temp";
    let enthalpy_trace = "FourPipeG36Controls|ahu_1_return_air_enthalpy_kj_kg";
    sim.set_traces_json(&json!([temp_trace, enthalpy_trace]).to_string())
        .unwrap();
    let result: Value = serde_json::from_str(&sim.step_chunk_json(1).unwrap()).unwrap();
    let row = &result["rows"][0];
    assert_eq!(row["traces"][temp_trace], json!(28.0));
    assert!(
        (row["traces"][enthalpy_trace].as_f64().unwrap() - outdoor_air_enthalpy_kj_kg(28.0, 0.5))
            .abs()
            < 1e-9
    );
    let zone_flow = row["zone_airflow"].as_array().unwrap();
    let total_flow: f64 = zone_flow[..5].iter().map(|v| v.as_f64().unwrap()).sum();
    assert!(total_flow > 0.0);
    let weighted_return = (0..5)
        .map(|zone| temperatures[zone] * zone_flow[zone].as_f64().unwrap())
        .sum::<f64>()
        / total_flow;
    let outdoor_fraction = row["ahu_1_outdoor_air_fraction"].as_f64().unwrap();
    assert!((weighted_return - 20.0).abs() > 1.0);
    assert!(outdoor_fraction < 0.95);
    assert!((row["ahu_1_return_air_c"].as_f64().unwrap() - weighted_return).abs() < 1e-9);
    let flow = std::array::from_fn(|zone| zone_flow[zone].as_f64().unwrap());
    let return_enthalpy = ahu_return_enthalpy_kj_kg(&temperatures, &return_humidity, &flow, 0);
    let return_ratio = ahu_return_value(&return_humidity, &flow, 0);
    let outdoor_c = row["outdoor_c"].as_f64().unwrap();
    let outdoor_ratio = humidity_ratio(outdoor_c, 0.5);
    let expected_mixed_enthalpy = (1.0 - outdoor_fraction) * return_enthalpy
        + outdoor_fraction * moist_air_enthalpy_kj_kg(outdoor_c, outdoor_ratio);
    let expected_mixed_ratio =
        (1.0 - outdoor_fraction) * return_ratio + outdoor_fraction * outdoor_ratio;
    let mixed_c = row["ahu_1_mixed_air_c"].as_f64().unwrap();
    assert!(
        (moist_air_enthalpy_kj_kg(mixed_c, expected_mixed_ratio) - expected_mixed_enthalpy).abs()
            < 1e-9
    );
    assert!(
        (row["ahu_1_mixed_air_enthalpy_kj_kg"].as_f64().unwrap() - expected_mixed_enthalpy).abs()
            < 1e-9
    );
}
