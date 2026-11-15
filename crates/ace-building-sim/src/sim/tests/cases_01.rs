use super::*;

#[test]
fn physical_source_start_follows_editable_cdl_request() {
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
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    let plant = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "plant")
        .unwrap();
    plant["blocks"].as_array_mut().unwrap().push(json!({
        "name": "operator_heat_pump_start_off",
        "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.Constant",
        "category": "elementary",
        "outputs": [{"name": "y", "type": "Boolean"}],
        "parameters": [{"name": "k", "type": "Boolean", "value": false}]
    }));
    let edges = plant["connections"].as_array_mut().unwrap();
    edges.retain(|edge| edge["to"] != "heat_pump_start_command");
    edges.push(json!({"from":"operator_heat_pump_start_off.y", "to":"heat_pump_start_command"}));
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut disabled = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    let heat_trace = "FourPipeBuildingControls|output.plant_heat_pump_heating_kw";
    let start_trace = "FourPipeBuildingControls|output.plant_heat_pump_start_command";
    let targets = json!([heat_trace, start_trace]).to_string();
    for sim in [&mut normal, &mut disabled] {
        sim.set_traces_json(&targets).unwrap();
        start_present_window(sim);
        sim.temps = [16.0; 10];
        sim.last_heat_request = 100.0;
    }
    let normal_row = run_weather_intervals(&mut normal, 1)["rows"][0].clone();
    let disabled_row = run_weather_intervals(&mut disabled, 1)["rows"][0].clone();
    assert!(normal_row["traces"][heat_trace].as_f64().unwrap() > 0.0);
    assert_eq!(normal_row["traces"][start_trace], json!(1.0));
    assert_eq!(disabled_row["traces"][start_trace], json!(0.0));
    assert_eq!(
        normal_row["traces"][heat_trace],
        disabled_row["traces"][heat_trace]
    );
    assert!(normal.source_running[0]);
    assert!(!disabled.source_running[0]);
    assert!(normal_row["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
    assert_eq!(disabled_row["heat_pump_heating_used_kw"], json!(0.0));
}

#[test]
fn g36_heat_pump_failed_start_moves_load_to_boiler_until_operator_reset() {
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
    set_equipment_ready(&mut inputs);
    inputs.insert("heat_pump_running".into(), json!(false));
    for (name, value) in [
        ("heat_request_kw", 100.0),
        ("heat_pump_available_capacity_kw", 80.0),
        ("hot_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
    let mut fault = HashMap::new();
    for _ in 0..12 {
        fault = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            fault["heat_pump_start_fault"].as_bool().unwrap(),
            fast["heat_pump_start_fault"].as_bool().unwrap()
        );
        assert_eq!(fault["heat_pump_heating_kw"], fast["heat_pump_heating_kw"]);
    }
    assert_eq!(fault["heat_pump_start_fault"], json!(true));
    assert_eq!(fault["heat_pump_heating_kw"], json!(0.0));
    assert!(fault["boiler_kw"].as_f64().unwrap() > 0.0);
    inputs.insert("heat_pump_running".into(), json!(true));
    inputs.insert("heat_pump_fault_reset".into(), json!(true));
    let recovered = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
    let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(recovered["heat_pump_start_fault"], json!(false));
    assert_eq!(
        recovered["heat_pump_start_fault"].as_bool().unwrap(),
        fast["heat_pump_start_fault"].as_bool().unwrap()
    );
    assert!(recovered["heat_pump_heating_kw"].as_f64().unwrap() > 0.0);
}

#[test]
fn g36_backup_start_faults_are_latched_cdl_decisions() {
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
    for (equipment, request, flow, supply, command) in [
        (
            "boiler",
            "heat_request_kw",
            "hot_water_flow_kg_s",
            "hot_water_supply_temp_c",
            "boiler_kw",
        ),
        (
            "chiller",
            "cool_request_kw",
            "chilled_water_flow_kg_s",
            "chilled_water_supply_temp_c",
            "chiller_kw",
        ),
    ] {
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
        set_equipment_ready(&mut inputs);
        inputs.insert("heat_pump_available".into(), json!(false));
        inputs.insert("heat_pump_running".into(), json!(false));
        inputs.insert(format!("{equipment}_running"), json!(false));
        inputs.insert(request.into(), json!(100.0));
        inputs.insert(flow.into(), json!(2.0));
        inputs.insert(
            supply.into(),
            json!(if equipment == "boiler" { 45.0 } else { 7.0 }),
        );
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        let fault_name = format!("{equipment}_start_fault");
        let mut fault = HashMap::new();
        for _ in 0..12 {
            fault = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                fault[&fault_name].as_bool().unwrap(),
                fast[&fault_name].as_bool().unwrap()
            );
            assert_eq!(fault[command], fast[command]);
        }
        assert_eq!(fault[&fault_name], json!(true), "{equipment}");
        assert_eq!(fault[command], json!(0.0), "{equipment}");
        assert_eq!(
            fault[&format!("{equipment}_start_command")],
            json!(false),
            "{equipment}"
        );
        inputs.insert(format!("{equipment}_running"), json!(true));
        inputs.insert(format!("{equipment}_fault_reset"), json!(true));
        let recovered = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let recovered_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(recovered[&fault_name], json!(false), "{equipment}");
        assert!(recovered[command].as_f64().unwrap() > 0.0, "{equipment}");
        assert_eq!(
            recovered[&format!("{equipment}_start_command")],
            json!(true),
            "{equipment}"
        );
        assert_eq!(
            recovered[&format!("{equipment}_start_command")],
            recovered_fast[&format!("{equipment}_start_command")]
        );
        inputs.insert(format!("{equipment}_fault_reset"), json!(false));
        let mut restarted = false;
        for _ in 0..15 {
            let regular = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(regular[&fault_name], json!(false), "{equipment}");
            assert_eq!(
                regular[&format!("{equipment}_start_command")],
                fast[&format!("{equipment}_start_command")]
            );
            if regular[&format!("{equipment}_start_command")] == json!(true) {
                restarted = true;
                break;
            }
        }
        assert!(
            restarted,
            "{equipment} did not restart after CDL stage qualification"
        );
    }
}

#[test]
fn boiler_start_fault_timeout_is_set_by_its_cdl_parameter() {
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
    let mut standard_plant = plant.clone();
    set_backup_stage_delay(&mut standard_plant, 0.0);
    let mut tuned = standard_plant.clone();
    tuned
        .blocks
        .iter_mut()
        .find(|block| block.name == "boiler_start_timeout")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "delayTime")
        .unwrap()
        .value = json!(120.0);
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
    set_equipment_ready(&mut inputs);
    inputs.insert("heat_pump_available".into(), json!(false));
    inputs.insert("heat_pump_running".into(), json!(false));
    inputs.insert("boiler_running".into(), json!(false));
    inputs.insert("heat_request_kw".into(), json!(100.0));
    inputs.insert("hot_water_flow_kg_s".into(), json!(2.0));
    inputs.insert("hot_water_supply_temp_c".into(), json!(45.0));
    let mut standard = Executor::default();
    let mut shortened = Executor::default();
    let mut original_fault = false;
    let mut tuned_fault = false;
    for _ in 0..5 {
        original_fault = standard
            .execute(&standard_plant, &inputs, 60.0, "")
            .unwrap()["boiler_start_fault"]
            .as_bool()
            .unwrap();
        tuned_fault = shortened.execute(&tuned, &inputs, 60.0, "").unwrap()["boiler_start_fault"]
            .as_bool()
            .unwrap();
    }
    assert!(!original_fault);
    assert!(tuned_fault);
}

#[test]
fn physical_ahu_coil_valve_travel_limits_heat_transfer_but_not_cdl_command() {
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let ahu = graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ahu_1")
        .unwrap();
    ahu["blocks"].as_array_mut().unwrap().push(json!({
        "name":"forced_heat_valve", "block_type":"Constant", "category":"elementary",
        "outputs":[{"name":"y","type":"Real","unit":"fraction (0–1)"}],
        "parameters":[{"name":"k","type":"Real","value":1.0,"unit":"fraction (0–1)"}]
    }));
    ahu["connections"]
        .as_array_mut()
        .unwrap()
        .retain(|edge| edge["to"] != "heating_coil_valve_position");
    ahu["connections"].as_array_mut().unwrap().push(json!({
        "from":"forced_heat_valve.y", "to":"heating_coil_valve_position"
    }));
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
    let mut direct = Simulation::new_test(&graph, &weather, "{}").unwrap();
    let mut slow = Simulation::new_test(
        &graph,
        &weather,
        r#"{"ahu_heating_coil_valve_stroke_seconds":[3600,0]}"#,
    )
    .unwrap();
    for sim in [&mut direct, &mut slow] {
        start_present_window(sim);
        sim.temps = [15.0; 10];
        sim.last_zone_airflow = [0.4; 10];
        sim.last_heat_request = 200.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
    }
    let direct = run_weather_intervals(&mut direct, 1)["rows"][0].clone();
    let slow = run_weather_intervals(&mut slow, 1)["rows"][0].clone();
    let command = direct["ahu_1_heating_coil_valve_position"]
        .as_f64()
        .unwrap();
    assert_eq!(
        direct["ahu_1_heating_coil_valve_position"],
        slow["ahu_1_heating_coil_valve_position"]
    );
    assert!(command > 0.25);
    assert_eq!(
        direct["ahu_1_heating_coil_valve_actual_position"],
        json!(command)
    );
    assert!(
        slow["ahu_1_heating_coil_valve_actual_position"]
            .as_f64()
            .unwrap()
            <= 0.25 + 1e-9
    );
    assert!(
        slow["ahu_1_heating_coil_delivered_kw"].as_f64().unwrap()
            < direct["ahu_1_heating_coil_delivered_kw"].as_f64().unwrap()
    );
}

#[test]
fn physical_ahu_cooling_valve_travel_limits_cooling_but_not_cdl_command() {
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let ahu = graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "ahu_2")
        .unwrap();
    ahu["blocks"].as_array_mut().unwrap().push(json!({
        "name":"forced_cool_valve", "block_type":"Constant", "category":"elementary",
        "outputs":[{"name":"y","type":"Real","unit":"fraction (0–1)"}],
        "parameters":[{"name":"k","type":"Real","value":1.0,"unit":"fraction (0–1)"}]
    }));
    ahu["connections"]
        .as_array_mut()
        .unwrap()
        .retain(|edge| edge["to"] != "cooling_coil_valve_position");
    ahu["connections"].as_array_mut().unwrap().push(json!({
        "from":"forced_cool_valve.y", "to":"cooling_coil_valve_position"
    }));
    let graph = graph.to_string();
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
    let mut direct = Simulation::new_test(&graph, &weather, "{}").unwrap();
    let mut slow = Simulation::new_test(
        &graph,
        &weather,
        r#"{"ahu_cooling_coil_valve_stroke_seconds":[0,3600]}"#,
    )
    .unwrap();
    for sim in [&mut direct, &mut slow] {
        start_present_window(sim);
        sim.temps = [30.0; 10];
        sim.last_zone_airflow = [0.4; 10];
        sim.last_cool_request = 200.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
    }
    let direct = run_weather_intervals(&mut direct, 1)["rows"][0].clone();
    let slow = run_weather_intervals(&mut slow, 1)["rows"][0].clone();
    assert_eq!(direct["ahu_2_cooling_coil_valve_position"], json!(1.0));
    assert_eq!(slow["ahu_2_cooling_coil_valve_position"], json!(1.0));
    assert_eq!(
        direct["ahu_2_cooling_coil_valve_actual_position"],
        json!(1.0)
    );
    assert!(
        slow["ahu_2_cooling_coil_valve_actual_position"]
            .as_f64()
            .unwrap()
            <= 0.25 + 1e-9
    );
    assert!(
        slow["ahu_2_cooling_coil_delivered_kw"].as_f64().unwrap()
            < direct["ahu_2_cooling_coil_delivered_kw"].as_f64().unwrap()
    );
}

#[test]
fn physical_reheat_valve_travel_delays_delivery_without_changing_cdl_command() {
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
    let slow_scenario = json!({
        "terminal_reheat_valve_stroke_seconds": [3600.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0]
    });
    let mut immediate = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut slow = Simulation::new_test(graph, &weather, &slow_scenario.to_string()).unwrap();
    let trace = "FourPipeBuildingControls|output.zone_01_reheat_valve_position";
    for sim in [&mut immediate, &mut slow] {
        start_present_window(sim);
        sim.temps = [15.0; 10];
        sim.last_zone_airflow = [0.4; 10];
        sim.last_heat_request = 200.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        sim.set_traces_json(&json!([trace]).to_string()).unwrap();
    }
    let direct = run_weather_intervals(&mut immediate, 1);
    let delayed = run_weather_intervals(&mut slow, 1);
    let direct = &direct["rows"][0];
    let delayed = &delayed["rows"][0];
    assert_eq!(direct["traces"][trace], delayed["traces"][trace]);
    assert!(direct["traces"][trace].as_f64().unwrap() > 0.25);
    assert!(
        delayed["zone_actual_reheat_valve_position"][0]
            .as_f64()
            .unwrap()
            <= 0.25 + 1e-9
    );
    assert!(
        direct["zone_actual_reheat_valve_position"][0]
            .as_f64()
            .unwrap()
            > delayed["zone_actual_reheat_valve_position"][0]
                .as_f64()
                .unwrap()
    );
    assert!(
        direct["zone_discharge_c"][0].as_f64().unwrap()
            > delayed["zone_discharge_c"][0].as_f64().unwrap()
    );
}

#[test]
fn physical_vav_design_flow_changes_delivery_without_changing_cdl_flow_request() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 32.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut default = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut smaller = Simulation::new_test(
        graph,
        &weather,
        &json!({"terminal_design_flow": [0.25,0.95,0.95,0.95,0.95,0.95,0.95,0.95,0.95,0.95]})
            .to_string(),
    )
    .unwrap();
    start_present_window(&mut default);
    start_present_window(&mut smaller);
    let default_rows = run_weather_intervals(&mut default, 4);
    let smaller_rows = run_weather_intervals(&mut smaller, 4);
    let baseline = &default_rows["rows"][0];
    let reduced = &smaller_rows["rows"][0];
    assert_eq!(baseline["requested_airflow"], reduced["requested_airflow"]);
    assert!(
        baseline["zone_airflow"][0].as_f64().unwrap()
            > reduced["zone_airflow"][0].as_f64().unwrap() + 0.01
    );
    assert_ne!(
        default_rows["rows"][3]["zone_c"][0],
        smaller_rows["rows"][3]["zone_c"][0]
    );
}

#[test]
fn physical_fan_design_flow_changes_delivery_and_cdl_pressure_requests() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 32.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let target = "FourPipeG36Controls|output.zone_01_pressure_reset_request";
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut undersized = Simulation::new_test(
        graph,
        &weather,
        &json!({"ahu_fan_design_airflow": [1.0,5.0]}).to_string(),
    )
    .unwrap();
    for sim in [&mut normal, &mut undersized] {
        sim.set_traces_json(&json!([target]).to_string()).unwrap();
        start_present_window(sim);
        sim.temps = [28.0; 10];
    }
    let normal_rows = run_weather_intervals(&mut normal, 16);
    let undersized_rows = run_weather_intervals(&mut undersized, 16);
    let normal_first = &normal_rows["rows"][0];
    let undersized_first = &undersized_rows["rows"][0];
    assert_eq!(
        normal_first["ahu_1_actual_fan_speed"],
        undersized_first["ahu_1_actual_fan_speed"]
    );
    assert!(
        normal_first["zone_airflow"][0].as_f64().unwrap()
            > undersized_first["zone_airflow"][0].as_f64().unwrap() + 0.01
    );
    let normal_requests: Vec<f64> = normal_rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["traces"][target].as_f64().unwrap())
        .collect();
    let undersized_requests: Vec<f64> = undersized_rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["traces"][target].as_f64().unwrap())
        .collect();
    assert!(
        undersized_requests
            .iter()
            .zip(&normal_requests)
            .any(|(small, design)| small > design),
        "normal={normal_requests:?}, undersized={undersized_requests:?}"
    );
}

#[test]
fn g36_requires_cdl_driven_occupancy_and_preconditioning_outputs() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    assert!(g36_control_contract(&graph).is_empty());
    graph.connections.retain(|edge| {
        !matches!(
            edge.target().0.as_str(),
            "zone_01_occupancy_confirmed" | "preconditioning_mode"
        )
    });
    let errors = g36_control_contract(&graph);
    assert_eq!(errors.len(), 2);
    assert!(errors
        .iter()
        .any(|error| error.contains("zone_01_occupancy_confirmed")));
    assert!(errors
        .iter()
        .any(|error| error.contains("preconditioning_mode")));
}

#[test]
fn g36_requires_graph_outputs_for_every_physical_actuator() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    assert!(g36_control_contract(&graph).is_empty());
    let missing = [
        "plant_heat_pump_heating_kw",
        "plant_heat_pump_start_command",
        "plant_boiler_start_command",
        "plant_chiller_start_command",
        "ahu_1_outdoor_air_damper_position",
        "ahu_2_return_air_damper_position",
        "zone_01_damper_position",
    ];
    graph
        .connections
        .retain(|edge| !missing.contains(&edge.target().0.as_str()));
    let errors = g36_control_contract(&graph);
    assert_eq!(errors.len(), missing.len());
    for name in missing {
        assert!(errors.iter().any(|error| error.contains(name)));
    }
}

#[test]
fn conventional_requires_cdl_driven_physical_commands() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    assert!(conventional_control_contract(&graph).is_empty());
    let missing = [
        "preconditioning_mode",
        "zone_01_occupancy_confirmed",
        "plant_heat_pump_heating_kw",
        "plant_heat_pump_requested_mode",
        "plant_heat_pump_start_command",
        "plant_boiler_start_command",
        "plant_chiller_start_command",
        "plant_heat_pump_start_fault",
        "ahu_1_fan_speed",
        "zone_01_reheat_valve_position",
    ];
    graph
        .connections
        .retain(|edge| !missing.contains(&edge.target().0.as_str()));
    let errors = conventional_control_contract(&graph);
    assert_eq!(errors.len(), missing.len());
    for name in missing {
        assert!(errors.iter().any(|error| error.contains(name)), "{name}");
    }

    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let edge = graph
        .connections
        .iter_mut()
        .find(|edge| edge.target().0 == "zone_01_damper_position")
        .unwrap();
    edge.from_path = "zone_01_supply_pressure".into();
    edge.from_block.clear();
    edge.from_output.clear();
    assert!(conventional_control_contract(&graph)
        .iter()
        .any(|error| error.contains("zone_01_damper_position")));
}

#[test]
fn conventional_heat_pump_mode_is_computed_by_editable_cdl() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    assert!(!is_g36_graph(&building));
    let plant = building
        .blocks
        .iter()
        .find(|block| block.name == "plant")
        .unwrap();
    for (heat, cool, expected) in [
        (0.0, 0.0, 0),
        (100.0, 0.0, 1),
        (0.0, 100.0, 2),
        (100.0, 100.0, 3),
    ] {
        let mut inputs: HashMap<String, Value> = plant
            .inputs
            .iter()
            .map(|port| (port.name.clone(), json!(0.0)))
            .collect();
        inputs.insert("heat_request_kw".into(), json!(heat));
        inputs.insert("cool_request_kw".into(), json!(cool));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        let slow = dynamic.execute(plant, &inputs, 900.0, "").unwrap();
        let fast = compiled.execute(&inputs, 900.0, false).unwrap().0;
        assert_eq!(slow["heat_pump_requested_mode"], json!(expected));
        assert_eq!(fast["heat_pump_requested_mode"], json!(expected));
    }
}

#[test]
fn g36_cdl_flow_override_changes_physical_zone_airflow() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "zone_01_flow_override_none")
        .unwrap()["parameters"][0]["value"] = json!(1);
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 20.0,
            solar_kw: 0.0,
            internal_kw: 16.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut forced = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    for sim in [&mut normal, &mut forced] {
        start_present_window(sim);
        sim.temps = [27.0; 10];
    }
    let baseline = run_weather_intervals(&mut normal, 2);
    let override_rows = run_weather_intervals(&mut forced, 2);
    let normal_row = &baseline["rows"][1];
    let forced_row = &override_rows["rows"][1];
    assert!(
        normal_row["requested_airflow"].as_f64().unwrap()
            > forced_row["requested_airflow"].as_f64().unwrap()
    );
    assert!(
        normal_row["zone_airflow"][0].as_f64().unwrap()
            > forced_row["zone_airflow"][0].as_f64().unwrap()
    );
    assert!(
        forced_row["zone_airflow"][0].as_f64().unwrap() < 0.005,
        "the OBC-style PI should nearly close the damper after a zero-flow override"
    );
}

#[test]
fn conventional_status_follows_edited_cdl() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    assert!(!is_g36_graph(&graph));
    assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
    assert!(conventional_control_contract(&graph).is_empty());
    assert!(
        undriven_building_outputs(&graph).is_empty(),
        "{:?}",
        undriven_building_outputs(&graph)
    );
    let inputs: HashMap<String, Value> = graph
        .inputs
        .iter()
        .map(|port| {
            (
                port.name.clone(),
                if port.kind == "Boolean" {
                    json!(false)
                } else {
                    json!(22.0)
                },
            )
        })
        .collect();
    let mut executor = Executor::default();
    let normal = executor.execute(&graph, &inputs, 900.0, "").unwrap();
    assert_eq!(normal["preconditioning_mode"], false);
    assert_eq!(normal["zone_01_occupancy_confirmed"], false);

    let mut edited = graph;
    edited
        .blocks
        .iter_mut()
        .find(|block| block.name == "conventional_preconditioning_off")
        .unwrap()
        .parameters[0]
        .value = json!(true);
    let mut executor = Executor::default();
    let changed = executor.execute(&edited, &inputs, 900.0, "").unwrap();
    assert_eq!(changed["preconditioning_mode"], true);
    assert_eq!(changed["zone_01_occupancy_confirmed"], false);
    edited
        .blocks
        .iter_mut()
        .find(|block| block.name == "conventional_preconditioning_off")
        .unwrap()
        .parameters[0]
        .value = json!(false);
    edited
        .blocks
        .iter_mut()
        .find(|block| block.name == "conventional_occupancy_false")
        .unwrap()
        .parameters[0]
        .value = json!(true);
    let mut executor = Executor::default();
    let changed = executor.execute(&edited, &inputs, 900.0, "").unwrap();
    let mut compiled = CompiledExecutor::try_new(&edited).unwrap().unwrap();
    let faster = compiled.execute(&inputs, 900.0, false).unwrap().0;
    assert_eq!(changed["preconditioning_mode"], false);
    assert_eq!(changed["zone_01_occupancy_confirmed"], true);
    for name in ["preconditioning_mode", "zone_01_occupancy_confirmed"] {
        assert_eq!(changed[name], faster[name]);
    }
}

#[test]
fn building_rejects_undriven_alarm_and_request_outputs() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    assert!(undriven_building_outputs(&graph).is_empty());
    let alarm = "zone_01_low_airflow_alarm_level";
    let edge = graph
        .connections
        .iter_mut()
        .find(|edge| edge.target().0 == alarm)
        .unwrap();
    edge.from_path = "zone_01_room_temp".into();
    edge.from_block.clear();
    edge.from_output.clear();
    assert!(undriven_building_outputs(&graph)
        .iter()
        .any(|error| error.contains(alarm)));

    let mut conventional: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    assert!(undriven_building_outputs(&conventional).is_empty());
    conventional
        .connections
        .retain(|edge| edge.target().0 != "zone_01_cooling_demand");
    assert!(undriven_building_outputs(&conventional)
        .iter()
        .any(|error| error.contains("zone_01_cooling_demand")));
}

#[test]
fn renaming_a_g36_program_does_not_disable_its_control_contract() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    graph.name = "CustomBuildingSequence".into();
    assert!(is_g36_graph(&graph));
    assert!(g36_control_contract(&graph).is_empty());
    graph
        .connections
        .retain(|edge| edge.target().0 != "plant_heat_pump_requested_mode");
    assert!(g36_control_contract(&graph)
        .iter()
        .any(|error| error.contains("plant_heat_pump_requested_mode")));
    graph
        .blocks
        .iter_mut()
        .find(|block| block.name == "plant")
        .unwrap()
        .name = "custom_plant".into();
    graph
        .blocks
        .iter_mut()
        .find(|block| block.name == "zone_group_1")
        .unwrap()
        .name = "custom_group".into();
    assert!(is_g36_graph(&graph));
}

#[test]
fn removing_g36_signature_outputs_does_not_enable_raw_occupancy_fallback() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    graph.name = "EditedBuildingControls".into();
    for name in [
        "plant_heat_pump_requested_mode",
        "zone_01_occupancy_confirmed",
        "preconditioning_mode",
    ] {
        graph.outputs.retain(|output| output.name != name);
        graph.connections.retain(|edge| edge.target().0 != name);
    }
    assert!(is_g36_graph(&graph));
    let errors = g36_control_contract(&graph);
    for name in [
        "plant_heat_pump_requested_mode",
        "zone_01_occupancy_confirmed",
        "preconditioning_mode",
    ] {
        assert!(errors.iter().any(|error| error.contains(name)), "{name}");
    }
}

#[test]
fn g36_rejects_raw_sensor_passthrough_as_an_actuator_command() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let edge = graph
        .connections
        .iter_mut()
        .find(|edge| edge.target().0 == "zone_01_airflow_request")
        .unwrap();
    edge.from_path = "zone_01_room_temp".into();
    edge.from_block.clear();
    edge.from_output.clear();
    assert!(g36_control_contract(&graph)
        .iter()
        .any(|error| error.contains("zone_01_airflow_request")));
}

#[test]
fn g36_rejects_raw_passthrough_inside_a_composite_output() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    assert!(g36_control_contract(&graph).is_empty());
    let zone = graph
        .blocks
        .iter_mut()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let edge = zone
        .connections
        .iter_mut()
        .find(|edge| edge.target().0 == "damper_position")
        .unwrap();
    edge.from_path = "room_temp".into();
    assert!(g36_control_contract(&graph)
        .iter()
        .any(|error| error.contains("zone_01_damper_position")));
    let zone = graph
        .blocks
        .iter_mut()
        .find(|block| block.name == "zone_01")
        .unwrap();
    zone.blocks.clear();
    zone.connections.clear();
    zone.category = None;
    assert!(g36_control_contract(&graph)
        .iter()
        .any(|error| error.contains("zone_01_damper_position")));
}

#[test]
fn g36_source_commands_require_motor_and_water_flow_proof_in_both_executors() {
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
        .map(|port| (port.name.clone(), json!(0.0)))
        .collect();
    set_equipment_ready(&mut inputs);
    for (name, value) in [
        ("heat_request_kw", 100.0),
        ("cool_request_kw", 100.0),
        ("heat_pump_available_capacity_kw", 80.0),
        ("hot_water_supply_temp_c", 45.0),
        ("chilled_water_supply_temp_c", 7.0),
        ("chilled_water_flow_kg_s", 2.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    inputs.insert("hot_water_pump_running".into(), json!(true));
    inputs.insert("chilled_water_pump_running".into(), json!(true));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
    let cold_only = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
    let cold_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(cold_only["hot_source_proven_output"], json!(false));
    assert_eq!(cold_only["chilled_source_proven_output"], json!(true));
    assert_eq!(cold_only["heat_pump_heating_kw"], json!(0.0));
    assert_eq!(cold_only["boiler_kw"], json!(0.0));
    assert!(cold_only["heat_pump_cooling_kw"].as_f64().unwrap() > 0.0);
    assert!(cold_only["chiller_kw"].as_f64().unwrap() > 0.0);
    for name in [
        "heat_pump_heating_kw",
        "boiler_kw",
        "heat_pump_cooling_kw",
        "chiller_kw",
    ] {
        assert_eq!(cold_fast[name], cold_only[name]);
    }
    inputs.insert("hot_water_flow_kg_s".into(), json!(2.0));
    inputs.insert("hot_water_pump_running".into(), json!(false));
    let failed_motor = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
    let failed_motor_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(failed_motor["hot_source_proven_output"], json!(false));
    assert_eq!(failed_motor["heat_pump_heating_kw"], json!(0.0));
    assert_eq!(failed_motor_fast["hot_source_proven_output"], json!(false));
    inputs.insert("hot_water_pump_running".into(), json!(true));
    inputs.insert("hot_water_flow_kg_s".into(), json!(2.0));
    inputs.insert("chilled_water_flow_kg_s".into(), json!(0.0));
    let hot_only = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
    let hot_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(hot_only["hot_source_proven_output"], json!(true));
    assert_eq!(hot_only["chilled_source_proven_output"], json!(false));
    assert!(hot_only["heat_pump_heating_kw"].as_f64().unwrap() > 0.0);
    assert!(hot_only["boiler_kw"].as_f64().unwrap() > 0.0);
    assert_eq!(hot_only["heat_pump_cooling_kw"], json!(0.0));
    assert_eq!(hot_only["chiller_kw"], json!(0.0));
    for name in [
        "heat_pump_heating_kw",
        "boiler_kw",
        "heat_pump_cooling_kw",
        "chiller_kw",
    ] {
        assert_eq!(hot_fast[name], hot_only[name]);
    }
}

#[test]
fn g36_pump_start_fault_is_timed_by_editable_cdl_in_both_executors() {
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
        .map(|port| (port.name.clone(), json!(0.0)))
        .collect();
    set_equipment_ready(&mut inputs);
    inputs.insert("heat_request_kw".into(), json!(100.0));
    inputs.insert("hot_water_pump_running".into(), json!(false));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
    for step in 1..=6 {
        let slow = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        let expected = json!(step == 6);
        assert_eq!(
            slow["hot_water_pump_start_fault"], expected,
            "step {step}, speed {:?}",
            slow["hot_water_pump_speed"]
        );
        assert_eq!(fast["hot_water_pump_start_fault"], expected, "step {step}");
    }
    let mut edited = plant.clone();
    edited
        .blocks
        .iter_mut()
        .find(|block| block.name == "hot_pump_fault_delay")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "delayTime")
        .unwrap()
        .value = json!(600.0);
    let mut edited_dynamic = Executor::default();
    let mut edited_compiled = CompiledExecutor::try_new(&edited).unwrap().unwrap();
    for _ in 0..6 {
        assert_eq!(
            edited_dynamic.execute(&edited, &inputs, 60.0, "").unwrap()
                ["hot_water_pump_start_fault"],
            json!(false)
        );
        assert_eq!(
            edited_compiled.execute(&inputs, 60.0, false).unwrap().0["hot_water_pump_start_fault"],
            json!(false)
        );
    }
}

#[test]
fn g36_water_flow_fault_is_timed_and_editable_in_both_executors() {
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
    for (loop_name, request) in [("hot", "heat_request_kw"), ("chilled", "cool_request_kw")] {
        let mut inputs: HashMap<String, Value> = plant
            .inputs
            .iter()
            .map(|port| (port.name.clone(), json!(0.0)))
            .collect();
        set_equipment_ready(&mut inputs);
        inputs.insert(request.into(), json!(100.0));
        inputs.insert(format!("{loop_name}_water_pump_running"), json!(true));
        let output = format!("{loop_name}_water_flow_fault");
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        for step in 1..=6 {
            let slow = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            let expected = json!(step == 6);
            assert_eq!(slow[&output], expected, "{loop_name} step {step}");
            assert_eq!(fast[&output], expected, "{loop_name} step {step}");
        }
        inputs.insert(format!("{loop_name}_water_flow_kg_s"), json!(2.0));
        assert_eq!(
            dynamic.execute(plant, &inputs, 60.0, "").unwrap()[&output],
            json!(false)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0[&output],
            json!(false)
        );

        let mut edited = plant.clone();
        edited
            .blocks
            .iter_mut()
            .find(|block| block.name == format!("{loop_name}_flow_fault_delay"))
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "delayTime")
            .unwrap()
            .value = json!(600.0);
        inputs.insert(format!("{loop_name}_water_flow_kg_s"), json!(0.0));
        let mut edited_dynamic = Executor::default();
        let mut edited_compiled = CompiledExecutor::try_new(&edited).unwrap().unwrap();
        for _ in 0..6 {
            assert_eq!(
                edited_dynamic.execute(&edited, &inputs, 60.0, "").unwrap()[&output],
                json!(false)
            );
            assert_eq!(
                edited_compiled.execute(&inputs, 60.0, false).unwrap().0[&output],
                json!(false)
            );
        }
    }
}

#[test]
fn g36_hybrid_dispatch_allocates_capacity_and_backup_in_cdl() {
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
    set_equipment_ready(&mut inputs);
    for (name, value) in [
        ("heat_request_kw", 100.0),
        ("cool_request_kw", 100.0),
        ("heat_pump_available_capacity_kw", 40.0),
        ("hot_water_flow_kg_s", 2.0),
        ("chilled_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
        ("chilled_water_supply_temp_c", 7.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
    let dual = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
    let dual_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    for name in [
        "heat_pump_heating_kw",
        "heat_pump_cooling_kw",
        "boiler_kw",
        "chiller_kw",
        "heat_pump_requested_mode",
        "heat_pump_start_command",
        "boiler_start_command",
        "chiller_start_command",
    ] {
        assert_eq!(dual[name], dual_fast[name], "{name}");
    }
    assert_eq!(dual["heat_pump_requested_mode"].as_f64().unwrap(), 3.0);
    assert_eq!(dual["heat_pump_start_command"], json!(true));
    assert_eq!(dual["boiler_start_command"], json!(false));
    assert_eq!(dual["chiller_start_command"], json!(false));
    let hp_total = dual["heat_pump_heating_kw"].as_f64().unwrap()
        + dual["heat_pump_cooling_kw"].as_f64().unwrap();
    assert!(
        (hp_total - 40.0).abs() < 1e-8,
        "allocated heat pump={hp_total}"
    );
    assert!(dual["boiler_kw"].as_f64().unwrap() > 0.0);
    assert!(dual["chiller_kw"].as_f64().unwrap() > 0.0);
    for tick in 1..=5 {
        let staged = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let staged_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for source in ["boiler", "chiller"] {
            let command = format!("{source}_start_command");
            assert_eq!(staged[&command], json!(tick == 5), "{source} tick {tick}");
            assert_eq!(staged[&command], staged_fast[&command]);
        }
    }
    inputs.insert("chilled_water_flow_kg_s".into(), json!(0.0));
    let hot_only = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
    let hot_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(hot_only["heat_pump_requested_mode"].as_f64().unwrap(), 1.0);
    assert_eq!(hot_only["heat_pump_cooling_kw"], json!(0.0));
    assert_eq!(hot_only["chiller_kw"], json!(0.0));
    assert_eq!(hot_only["chiller_start_command"], json!(true));
    assert_eq!(
        hot_only["chiller_start_command"],
        hot_fast["chiller_start_command"]
    );
    assert_eq!(
        hot_only["heat_pump_heating_kw"],
        hot_fast["heat_pump_heating_kw"]
    );
    assert!((hot_only["heat_pump_heating_kw"].as_f64().unwrap() - 40.0).abs() < 1e-8);
    let mut tuned = plant.clone();
    tuned
        .blocks
        .iter_mut()
        .find(|block| block.name == "heat_pump_utilization_limit")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|p| p.name == "k")
        .unwrap()
        .value = json!(0.25);
    inputs.insert("chilled_water_flow_kg_s".into(), json!(2.0));
    let limited = Executor::default()
        .execute(&tuned, &inputs, 60.0, "")
        .unwrap();
    let limited_hp = limited["heat_pump_heating_kw"].as_f64().unwrap()
        + limited["heat_pump_cooling_kw"].as_f64().unwrap();
    assert!(
        (limited_hp - 10.0).abs() < 1e-8,
        "edited capacity limit={limited_hp}"
    );
    assert!(limited["boiler_kw"].as_f64().unwrap() > dual["boiler_kw"].as_f64().unwrap());
    assert!(limited["chiller_kw"].as_f64().unwrap() > dual["chiller_kw"].as_f64().unwrap());
}

#[test]
fn g36_source_minimum_run_holds_starts_after_thermal_demand_clears() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut plant = building
        .blocks
        .iter()
        .find(|block| block.name == "plant")
        .unwrap()
        .clone();
    set_backup_stage_delay(&mut plant, 0.0);
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
    set_equipment_ready(&mut inputs);
    for (name, value) in [
        ("heat_request_kw", 100.0),
        ("cool_request_kw", 100.0),
        ("heat_pump_available_capacity_kw", 40.0),
        ("hot_water_flow_kg_s", 2.0),
        ("chilled_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
        ("chilled_water_supply_temp_c", 7.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&plant).unwrap().unwrap();
    for tick in 0..7 {
        if tick == 1 {
            inputs.insert("heat_request_kw".into(), json!(0.0));
            inputs.insert("cool_request_kw".into(), json!(0.0));
            inputs.insert("hot_water_supply_temp_c".into(), json!(60.0));
            inputs.insert("chilled_water_supply_temp_c".into(), json!(4.0));
        }
        let regular = dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for source in ["heat_pump", "boiler", "chiller"] {
            let name = format!("{source}_start_command");
            assert_eq!(regular[&name], fast[&name], "tick {tick} {name}");
            assert_eq!(regular[&name], json!(tick < 5), "tick {tick} {name}");
        }
    }
    inputs.insert("heat_request_kw".into(), json!(100.0));
    inputs.insert("cool_request_kw".into(), json!(100.0));
    inputs.insert("hot_water_supply_temp_c".into(), json!(45.0));
    inputs.insert("chilled_water_supply_temp_c".into(), json!(7.0));
    for source in ["heat_pump", "boiler", "chiller"] {
        inputs.insert(format!("{source}_running"), json!(false));
    }
    for tick in 7..13 {
        let regular = dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for source in ["heat_pump", "boiler", "chiller"] {
            let name = format!("{source}_start_command");
            assert_eq!(regular[&name], fast[&name], "restart tick {tick} {name}");
            assert_eq!(
                regular[&name],
                json!(tick >= 12),
                "restart tick {tick} {name}"
            );
            if tick < 12 {
                assert_eq!(regular[&format!("{source}_start_fault")], json!(false));
            }
        }
    }
    inputs.insert("heat_request_kw".into(), json!(100.0));
    inputs.insert("cool_request_kw".into(), json!(100.0));
    inputs.insert("hot_water_supply_temp_c".into(), json!(45.0));
    inputs.insert("chilled_water_supply_temp_c".into(), json!(7.0));
    for source in ["heat_pump", "boiler", "chiller"] {
        inputs.insert(format!("{source}_running"), json!(true));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&plant).unwrap().unwrap();
    dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
    compiled.execute(&inputs, 60.0, false).unwrap();
    for source in ["heat_pump", "boiler", "chiller"] {
        inputs.insert(format!("{source}_available"), json!(false));
    }
    let regular = dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
    let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    for source in ["heat_pump", "boiler", "chiller"] {
        let name = format!("{source}_start_command");
        assert_eq!(regular[&name], json!(false), "unavailable {name}");
        assert_eq!(regular[&name], fast[&name], "unavailable {name}");
    }
}

#[test]
fn g36_backup_stage_starts_only_after_editable_sustained_residual_load() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let reference = building
        .blocks
        .iter()
        .find(|block| block.name == "plant")
        .unwrap();
    for (delay, expected_tick) in [(300.0, 5), (120.0, 2)] {
        let mut plant = reference.clone();
        set_backup_stage_delay(&mut plant, delay);
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
        set_equipment_ready(&mut inputs);
        for (name, value) in [
            ("heat_request_kw", 100.0),
            ("cool_request_kw", 100.0),
            ("heat_pump_available_capacity_kw", 40.0),
            ("hot_water_flow_kg_s", 2.0),
            ("chilled_water_flow_kg_s", 2.0),
            ("hot_water_supply_temp_c", 45.0),
            ("chilled_water_supply_temp_c", 7.0),
        ] {
            inputs.insert(name.into(), json!(value));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&plant).unwrap().unwrap();
        for tick in 0..=expected_tick {
            let regular = dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            for source in ["boiler", "chiller"] {
                let command = format!("{source}_start_command");
                assert_eq!(
                    regular[&command],
                    json!(tick == expected_tick),
                    "{source} tick {tick}"
                );
                assert_eq!(regular[&command], fast[&command], "{source} tick {tick}");
                assert!(regular[&format!("{source}_kw")].as_f64().unwrap() > 0.0);
            }
        }
    }
}

#[test]
fn g36_backup_failover_bypasses_stage_delay_after_heat_pump_loss() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut plant = building
        .blocks
        .iter()
        .find(|block| block.name == "plant")
        .unwrap()
        .clone();
    set_backup_stage_delay(&mut plant, 600.0);
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
    set_equipment_ready(&mut inputs);
    for (name, value) in [
        ("heat_request_kw", 100.0),
        ("cool_request_kw", 100.0),
        ("heat_pump_available_capacity_kw", 40.0),
        ("hot_water_flow_kg_s", 2.0),
        ("chilled_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
        ("chilled_water_supply_temp_c", 7.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    inputs.insert("heat_pump_available".into(), json!(false));
    let unavailable = Executor::default()
        .execute(&plant, &inputs, 60.0, "")
        .unwrap();
    let unavailable_fast = CompiledExecutor::try_new(&plant)
        .unwrap()
        .unwrap()
        .execute(&inputs, 60.0, false)
        .unwrap()
        .0;
    for source in ["boiler", "chiller"] {
        let command = format!("{source}_start_command");
        assert_eq!(unavailable[&command], json!(true), "unavailable {source}");
        assert_eq!(unavailable[&command], unavailable_fast[&command]);
    }
    inputs.insert("heat_pump_available".into(), json!(true));
    inputs.insert("heat_pump_running".into(), json!(false));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&plant).unwrap().unwrap();
    let mut saw_fault = false;
    for tick in 0..9 {
        let regular = dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        let fault = regular["heat_pump_start_fault"] == json!(true);
        assert_eq!(
            regular["heat_pump_start_fault"],
            fast["heat_pump_start_fault"]
        );
        for source in ["boiler", "chiller"] {
            let command = format!("{source}_start_command");
            assert_eq!(regular[&command], json!(fault), "{source} tick {tick}");
            assert_eq!(regular[&command], fast[&command]);
        }
        if fault {
            saw_fault = true;
            break;
        }
    }
    assert!(saw_fault, "heat-pump start fault did not mature");
}

#[test]
fn g36_physical_stop_holds_restart_after_brief_outage() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut plant = building
        .blocks
        .iter()
        .find(|block| block.name == "plant")
        .unwrap()
        .clone();
    set_backup_stage_delay(&mut plant, 0.0);
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
    set_equipment_ready(&mut inputs);
    for (name, value) in [
        ("heat_request_kw", 100.0),
        ("cool_request_kw", 100.0),
        ("heat_pump_available_capacity_kw", 40.0),
        ("hot_water_flow_kg_s", 2.0),
        ("chilled_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
        ("chilled_water_supply_temp_c", 7.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&plant).unwrap().unwrap();
    let first = dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
    let first_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    for source in ["heat_pump", "boiler", "chiller"] {
        let name = format!("{source}_start_command");
        assert_eq!(first[&name], json!(true), "initial {name}");
        assert_eq!(first[&name], first_fast[&name]);
        inputs.insert(format!("{source}_available"), json!(false));
    }
    let stopped = dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
    let stopped_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    for source in ["heat_pump", "boiler", "chiller"] {
        let name = format!("{source}_start_command");
        assert_eq!(stopped[&name], json!(false), "outage {name}");
        assert_eq!(stopped[&name], stopped_fast[&name]);
        inputs.insert(format!("{source}_available"), json!(true));
        inputs.insert(format!("{source}_running"), json!(false));
    }
    for tick in 0..6 {
        let regular = dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for source in ["heat_pump", "boiler", "chiller"] {
            let name = format!("{source}_start_command");
            assert_eq!(
                regular[&name],
                json!(tick == 5),
                "restart tick {tick} {name}"
            );
            assert_eq!(regular[&name], fast[&name], "restart tick {tick} {name}");
            if tick < 5 {
                assert_eq!(regular[&format!("{source}_start_fault")], json!(false));
            }
        }
    }
}

#[test]
fn g36_physical_stop_lockout_duration_is_editable_cdl() {
    let building: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut plant = building
        .blocks
        .iter()
        .find(|block| block.name == "plant")
        .unwrap()
        .clone();
    let lockout = plant
        .blocks
        .iter_mut()
        .find(|block| block.name == "heat_pump_forced_stop_lockout")
        .unwrap();
    lockout
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "trueHoldDuration")
        .unwrap()
        .value = json!(120.0);
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
    set_equipment_ready(&mut inputs);
    for (name, value) in [
        ("heat_request_kw", 100.0),
        ("heat_pump_available_capacity_kw", 40.0),
        ("hot_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&plant).unwrap().unwrap();
    assert_eq!(
        dynamic.execute(&plant, &inputs, 60.0, "").unwrap()["heat_pump_start_command"],
        json!(true)
    );
    compiled.execute(&inputs, 60.0, false).unwrap();
    inputs.insert("heat_pump_available".into(), json!(false));
    dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
    compiled.execute(&inputs, 60.0, false).unwrap();
    inputs.insert("heat_pump_available".into(), json!(true));
    inputs.insert("heat_pump_running".into(), json!(false));
    for tick in 0..3 {
        let regular = dynamic.execute(&plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(regular["heat_pump_start_command"], json!(tick == 2));
        assert_eq!(
            regular["heat_pump_start_command"],
            fast["heat_pump_start_command"]
        );
    }
}
