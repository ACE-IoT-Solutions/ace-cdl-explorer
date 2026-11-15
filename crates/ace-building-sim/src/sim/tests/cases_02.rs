use super::*;

#[test]
fn g36_start_fault_overrides_minimum_run_until_cdl_reset() {
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
        ("heat_pump_available_capacity_kw", 40.0),
        ("hot_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
    let mut saw_fault = false;
    for tick in 0..10 {
        let regular = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            regular["heat_pump_start_fault"],
            fast["heat_pump_start_fault"]
        );
        assert_eq!(
            regular["heat_pump_start_command"],
            fast["heat_pump_start_command"]
        );
        if regular["heat_pump_start_fault"] == json!(true) {
            saw_fault = true;
            assert_eq!(
                regular["heat_pump_start_command"],
                json!(false),
                "tick {tick}"
            );
            break;
        }
        assert_eq!(
            regular["heat_pump_start_command"],
            json!(true),
            "tick {tick}"
        );
    }
    assert!(saw_fault, "failed start never latched a CDL fault");
    inputs.insert("heat_pump_fault_reset".into(), json!(true));
    inputs.insert("heat_pump_running".into(), json!(true));
    let regular = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
    let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(regular["heat_pump_start_fault"], json!(false));
    assert_eq!(regular["heat_pump_start_command"], json!(false));
    assert_eq!(
        regular["heat_pump_start_command"],
        fast["heat_pump_start_command"]
    );
    inputs.insert("heat_pump_fault_reset".into(), json!(false));
    let mut restarted = false;
    for _ in 0..8 {
        let regular = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(regular["heat_pump_start_fault"], json!(false));
        assert_eq!(
            regular["heat_pump_start_command"],
            fast["heat_pump_start_command"]
        );
        restarted |= regular["heat_pump_start_command"] == json!(true);
        if restarted {
            break;
        }
    }
    assert!(restarted, "source did not restart after CDL off lockout");
}

#[test]
fn g36_measured_service_capacity_changes_cdl_dispatch_and_backup_residuals() {
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
        ("heat_pump_available_capacity_kw", 80.0),
        ("heat_pump_heating_available_capacity_kw", 10.0),
        ("heat_pump_cooling_available_capacity_kw", 30.0),
        ("hot_water_flow_kg_s", 2.0),
        ("chilled_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
        ("chilled_water_supply_temp_c", 7.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    let dynamic = Executor::default()
        .execute(plant, &inputs, 60.0, "")
        .unwrap();
    let compiled = CompiledExecutor::try_new(plant)
        .unwrap()
        .unwrap()
        .execute(&inputs, 60.0, false)
        .unwrap()
        .0;
    for (output, expected) in [
        ("heat_pump_heating_kw", 10.0),
        ("heat_pump_cooling_kw", 30.0),
        ("boiler_kw", 90.0),
        ("chiller_kw", 70.0),
        ("heat_pump_requested_mode", 3.0),
    ] {
        let actual = dynamic[output].as_f64().unwrap();
        assert!((actual - expected).abs() < 1e-8, "{output}={actual}");
        assert_eq!(dynamic[output], compiled[output], "{output}");
    }
}

#[test]
fn g36_cdl_dispatch_respects_joint_map_capacity_envelopes() {
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
    for (demand_heat, demand_cool, shared, heat_capacity, cool_capacity) in [
        (0.0, 0.0, 80.0, 60.0, 40.0),
        (100.0, 0.0, 60.0, 40.0, 50.0),
        (0.0, 100.0, 60.0, 50.0, 40.0),
        (100.0, 100.0, 80.0, 10.0, 30.0),
        (100.0, 100.0, 80.0, 10.0, 70.0),
        (100.0, 100.0, 80.0, 70.0, 10.0),
        (100.0, 100.0, 0.0, 0.0, 0.0),
    ]
        as [(f64, f64, f64, f64, f64); 7]
    {
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
            ("heat_request_kw", demand_heat),
            ("cool_request_kw", demand_cool),
            ("heat_pump_available_capacity_kw", shared),
            ("heat_pump_heating_available_capacity_kw", heat_capacity),
            ("heat_pump_cooling_available_capacity_kw", cool_capacity),
            ("hot_water_flow_kg_s", 2.0),
            ("chilled_water_flow_kg_s", 2.0),
            ("hot_water_supply_temp_c", 45.0),
            ("chilled_water_supply_temp_c", 7.0),
        ] {
            inputs.insert(name.into(), json!(value));
        }
        let dynamic = Executor::default()
            .execute(plant, &inputs, 60.0, "")
            .unwrap();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for key in [
            "heat_pump_heating_kw",
            "heat_pump_cooling_kw",
            "boiler_kw",
            "chiller_kw",
            "heat_pump_requested_mode",
        ] {
            assert_eq!(
                dynamic[key], fast[key],
                "{key} at {demand_heat}/{demand_cool}"
            );
        }
        let heat = dynamic["heat_pump_heating_kw"].as_f64().unwrap();
        let cool = dynamic["heat_pump_cooling_kw"].as_f64().unwrap();
        let boiler = dynamic["boiler_kw"].as_f64().unwrap();
        let chiller = dynamic["chiller_kw"].as_f64().unwrap();
        assert!(heat >= 0.0 && heat <= demand_heat.min(heat_capacity) + 1e-8);
        assert!(cool >= 0.0 && cool <= demand_cool.min(cool_capacity) + 1e-8);
        assert!(heat + cool <= shared + 1e-8);
        let useful_limit = demand_heat.min(heat_capacity) + demand_cool.min(cool_capacity);
        assert!((heat + cool - shared.min(useful_limit)).abs() < 1e-8);
        assert!((boiler - (demand_heat - heat)).abs() < 1e-8);
        assert!((chiller - (demand_cool - cool)).abs() < 1e-8);
        let expected_mode =
            (if heat > 0.0 { 1.0 } else { 0.0 }) + (if cool > 0.0 { 2.0 } else { 0.0 });
        assert_eq!(
            dynamic["heat_pump_requested_mode"].as_f64(),
            Some(expected_mode)
        );
    }
}

#[test]
fn g36_equipment_availability_gates_dispatch_in_cdl() {
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
        ("heat_pump_available_capacity_kw", 80.0),
        ("hot_water_flow_kg_s", 2.0),
        ("chilled_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
        ("chilled_water_supply_temp_c", 7.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    let execute = |inputs: &HashMap<String, Value>| {
        let dynamic = Executor::default()
            .execute(plant, inputs, 60.0, "")
            .unwrap();
        let fast = CompiledExecutor::try_new(plant)
            .unwrap()
            .unwrap()
            .execute(inputs, 60.0, false)
            .unwrap()
            .0;
        for output in [
            "heat_pump_heating_kw",
            "heat_pump_cooling_kw",
            "boiler_kw",
            "chiller_kw",
            "heat_pump_requested_mode",
        ] {
            assert_eq!(dynamic[output], fast[output], "{output}");
        }
        dynamic
    };
    let normal = execute(&inputs);
    assert_eq!(normal["heat_pump_requested_mode"].as_i64(), Some(3));
    inputs.insert("heat_pump_available".into(), json!(false));
    let no_heat_pump = execute(&inputs);
    assert_eq!(no_heat_pump["heat_pump_heating_kw"], json!(0.0));
    assert_eq!(no_heat_pump["heat_pump_cooling_kw"], json!(0.0));
    assert_eq!(no_heat_pump["heat_pump_requested_mode"].as_i64(), Some(0));
    assert!(no_heat_pump["boiler_kw"].as_f64().unwrap() > normal["boiler_kw"].as_f64().unwrap());
    assert!(no_heat_pump["chiller_kw"].as_f64().unwrap() > normal["chiller_kw"].as_f64().unwrap());
    inputs.insert("boiler_available".into(), json!(false));
    assert_eq!(execute(&inputs)["boiler_kw"], json!(0.0));
    inputs.insert("chiller_available".into(), json!(false));
    assert_eq!(execute(&inputs)["chiller_kw"], json!(0.0));
}

#[test]
fn raw_equipment_outage_reaches_graph_and_physical_delivery() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
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
    let scenario = json!({"step_seconds":60,"equipment_outages":[{
        "equipment":"heat_pump","start_hour":7.75,"duration_hours":0.25
    }]});
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    start_present_window(&mut sim);
    sim.temps = [16.0; 10];
    sim.last_heat_request = 100.0;
    sim.last_water_flow_kg_s = [2.0, 2.0];
    sim.last_water_pump_running = [true, true];
    sim.step_chunk_json(1).unwrap();
    assert!(
        sim.source_running[1],
        "CDL backup start should reach the physical boiler on the first outage step"
    );
    let rows = run_weather_intervals(&mut sim, 2);
    let outage = &rows["rows"][0];
    let restored = &rows["rows"][1];
    assert_eq!(outage["heat_pump_available"], json!(0.0));
    assert_eq!(outage["heat_pump_heating_used_kw"], json!(0.0));
    assert_eq!(outage["heat_pump_cooling_used_kw"], json!(0.0));
    assert!(outage["boiler_used_kw"].as_f64().unwrap() > 0.0);
    assert_eq!(restored["heat_pump_available"], json!(1.0));
    assert!(restored["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
}

#[test]
fn raw_heat_pump_outage_starts_physical_chiller_on_first_step() {
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
    let scenario = json!({"step_seconds":60,"equipment_outages":[{
        "equipment":"heat_pump","start_hour":7.75,"duration_hours":0.25
    }]});
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    start_present_window(&mut sim);
    sim.temps = [30.0; 10];
    sim.last_cool_request = 100.0;
    sim.last_water_flow_kg_s = [2.0, 2.0];
    sim.last_water_pump_running = [true, true];
    sim.step_chunk_json(1).unwrap();
    assert!(
        sim.source_running[2],
        "CDL backup start should reach the physical chiller on the first outage step"
    );
}

#[test]
fn physical_source_start_delay_withholds_running_proof_and_delivery() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
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
    let scenario = json!({"step_seconds":60,"equipment_startup_seconds":{
        "heat_pump":300.0,"boiler":0.0,"chiller":0.0
    }});
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    start_present_window(&mut sim);
    sim.temps = [16.0; 10];
    sim.last_heat_request = 100.0;
    sim.last_water_flow_kg_s = [2.0, 2.0];
    sim.last_water_pump_running = [true, true];
    let startup_steps = (300 / sim.effective_step_seconds()) as usize;
    for step in 1..startup_steps {
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        sim.step_chunk_json(1).unwrap();
        assert_eq!(
            sim.source_start_elapsed_s[0],
            step as f64 * sim.effective_step_seconds() as f64
        );
        assert!(!sim.source_running[0]);
    }
    sim.last_heat_request = 100.0;
    sim.last_water_flow_kg_s = [2.0, 2.0];
    sim.last_water_pump_running = [true, true];
    sim.step_chunk_json(1).unwrap();
    assert!(sim.source_running[0]);
    // Keep the raw load and water proof present through the first recorded
    // weather interval. The graph still decides whether to command heat.
    let mut row = Value::Null;
    for _ in startup_steps..(900 / sim.effective_step_seconds()) {
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        let batch: Value = serde_json::from_str(&sim.step_chunk_json(1).unwrap()).unwrap();
        if let Some(recorded) = batch["rows"].as_array().unwrap().last() {
            row = recorded.clone();
        }
    }
    assert!(row.is_object());
    assert_eq!(row["heat_pump_running"], json!(1.0));
    assert!(row["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
}

#[test]
fn failed_start_keeps_availability_true_and_cdl_fault_dispatches_backup() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
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
    let scenario = json!({"step_seconds":60,"equipment_start_failures":[{
        "equipment":"heat_pump","start_hour":7.75,"duration_hours":0.5
    }],"heat_pump_fault_reset_hour":8.25});
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    start_present_window(&mut sim);
    sim.temps = [16.0; 10];
    sim.last_heat_request = 100.0;
    sim.last_water_flow_kg_s = [2.0, 2.0];
    sim.last_water_pump_running = [true, true];
    let rows = run_weather_intervals(&mut sim, 3);
    let fault = &rows["rows"][0];
    assert_eq!(fault["heat_pump_available"], json!(1.0));
    assert_eq!(fault["heat_pump_running"], json!(0.0));
    assert_eq!(fault["plant_heat_pump_start_fault"], json!(1.0));
    assert_eq!(fault["heat_pump_heating_used_kw"], json!(0.0));
    assert!(fault["boiler_used_kw"].as_f64().unwrap() > 0.0);
    assert_eq!(rows["rows"][1]["plant_heat_pump_start_fault"], json!(1.0));
    assert_eq!(rows["rows"][2]["plant_heat_pump_start_fault"], json!(0.0));
}

#[test]
fn physical_backup_start_failures_reach_cdl_alarms_and_reset() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    for (equipment, outdoor, room, request) in [
        ("boiler", -5.0, 16.0, "last_heat_request"),
        ("chiller", 35.0, 30.0, "last_cool_request"),
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
        let mut scenario = json!({"step_seconds":60,
            "equipment_outages":[{"equipment":"heat_pump","start_hour":7.75,"duration_hours":1.0}],
            "equipment_start_failures":[{"equipment":equipment,"start_hour":7.75,"duration_hours":0.5}]
        });
        scenario[format!("{equipment}_fault_reset_hour")] = json!(8.25);
        let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
        start_present_window(&mut sim);
        sim.temps = [room; 10];
        if request == "last_heat_request" {
            sim.last_heat_request = 100.0;
        } else {
            sim.last_cool_request = 100.0;
        }
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        let rows = run_weather_intervals(&mut sim, 3);
        let alarm = format!("plant_{equipment}_start_fault");
        assert_eq!(
            rows["rows"][0][format!("{equipment}_available")],
            json!(1.0),
            "{equipment}"
        );
        assert_eq!(
            rows["rows"][0][format!("{equipment}_running")],
            json!(0.0),
            "{equipment}"
        );
        assert_eq!(rows["rows"][1][&alarm], json!(1.0), "{equipment}");
        assert_eq!(rows["rows"][2][&alarm], json!(0.0), "{equipment}");
    }
}

#[test]
fn outdoor_performance_changes_the_capacity_offered_to_cdl_dispatch() {
    let points = default_heat_pump_performance();
    assert_eq!(heat_pump_performance(&points, -30.0), (0.55, 2.0, 3.0));
    assert_eq!(heat_pump_performance(&points, 45.0), (0.8, 2.7, 3.0));
    let (fraction, heating_cop, cooling_cop) = heat_pump_performance(&points, -2.5);
    assert!((fraction - 0.775).abs() < 1e-10);
    assert!((heating_cop - 2.6).abs() < 1e-10);
    assert!((cooling_cop - 3.4).abs() < 1e-10);

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
        ("hot_water_flow_kg_s", 2.0),
        ("chilled_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
        ("chilled_water_supply_temp_c", 7.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    let run = |available_kw: f64| {
        let mut inputs = inputs.clone();
        inputs.insert(
            "heat_pump_available_capacity_kw".into(),
            json!(available_kw),
        );
        Executor::default()
            .execute(plant, &inputs, 60.0, "")
            .unwrap()
    };
    let cold = run(80.0 * heat_pump_performance(&points, -15.0).0);
    let mild = run(80.0 * heat_pump_performance(&points, 10.0).0);
    assert!(
        cold["heat_pump_heating_kw"].as_f64().unwrap()
            < mild["heat_pump_heating_kw"].as_f64().unwrap()
    );
    assert!(cold["boiler_kw"].as_f64().unwrap() > mild["boiler_kw"].as_f64().unwrap());
    assert!(cold["chiller_kw"].as_f64().unwrap() > mild["chiller_kw"].as_f64().unwrap());
}

#[test]
fn heat_pump_cop_table_changes_physical_electricity_without_changing_cdl_commands() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 10.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let run = |cop: f64| {
        let scenario = json!({
            "heat_pump_performance": [
                {"outdoor_c": 0.0, "capacity_fraction": 1.0, "heating_cop": cop, "cooling_cop": cop},
                {"outdoor_c": 20.0, "capacity_fraction": 1.0, "heating_cop": cop, "cooling_cop": cop}
            ],
            "heat_pump_part_load_performance": [
                {"load_fraction": 0.0, "cop_multiplier": 0.8},
                {"load_fraction": 1.0, "cop_multiplier": 1.0}
            ],
            "heat_pump_hot_water_cop_performance": [
                {"water_c": 35.0, "cop_multiplier": 0.8, "capacity_multiplier": 1.2},
                {"water_c": 55.0, "cop_multiplier": 0.8, "capacity_multiplier": 1.2}
            ],
            "heat_pump_chilled_water_cop_performance": [
                {"water_c": 4.0, "cop_multiplier": 1.1, "capacity_multiplier": 0.8},
                {"water_c": 17.0, "cop_multiplier": 1.1, "capacity_multiplier": 0.8}
            ]
        });
        let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
        start_present_window(&mut sim);
        sim.temps = [16.0; 10];
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        let row = run_weather_intervals(&mut sim, 1)["rows"][0].clone();
        let fraction = row["heat_pump_part_load_fraction"].as_f64().unwrap();
        let multiplier = row["heat_pump_cop_multiplier"].as_f64().unwrap();
        assert!((multiplier - (0.8 + 0.2 * fraction)).abs() < 1e-9);
        assert_eq!(row["heat_pump_hot_water_cop_multiplier"], json!(0.8));
        assert_eq!(row["heat_pump_chilled_water_cop_multiplier"], json!(1.1));
        assert!(
            (row["heat_pump_heating_cop"].as_f64().unwrap() - cop * multiplier * 0.8).abs() < 1e-9
        );
        row
    };
    let low_cop = run(2.0);
    let high_cop = run(4.0);
    assert_eq!(low_cop["heat_pump_available_capacity_kw"], json!(96.0));
    assert_eq!(
        low_cop["heat_pump_heating_available_capacity_kw"],
        json!(96.0)
    );
    assert_eq!(
        low_cop["heat_pump_cooling_available_capacity_kw"],
        json!(64.0)
    );
    assert_eq!(
        low_cop["heat_pump_heating_used_kw"],
        high_cop["heat_pump_heating_used_kw"]
    );
    assert!(low_cop["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
    assert!(
        low_cop["heat_pump_electric_kw"].as_f64().unwrap()
            > high_cop["heat_pump_electric_kw"].as_f64().unwrap()
    );
}

#[test]
fn joint_heat_pump_map_changes_physical_cop_after_cdl_dispatch() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 10.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let run = |cop: f64| {
        let scenario = json!({"heat_pump_joint_performance": {
            "outdoor_c":[10.0], "hot_water_c":[45.0],
            "chilled_water_c":[7.0], "load_fraction":[0.0,1.0],
            "values": [
                {"heating_capacity_fraction":0.75,"cooling_capacity_fraction":0.5,"heating_cop":cop,"cooling_cop":cop},
                {"heating_capacity_fraction":0.75,"cooling_capacity_fraction":0.5,"heating_cop":cop,"cooling_cop":cop}
            ]
        }});
        let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
        start_present_window(&mut sim);
        sim.temps = [16.0; 10];
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        run_weather_intervals(&mut sim, 1)["rows"][0].clone()
    };
    let low = run(2.0);
    let high = run(4.0);
    assert_eq!(low["heat_pump_heating_available_capacity_kw"], json!(60.0));
    assert_eq!(low["heat_pump_cooling_available_capacity_kw"], json!(40.0));
    assert_eq!(low["heat_pump_heating_cop"], json!(2.0));
    assert_eq!(high["heat_pump_heating_cop"], json!(4.0));
    assert_eq!(
        low["heat_pump_heating_used_kw"],
        high["heat_pump_heating_used_kw"]
    );
    assert!(low["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
    assert!(
        low["heat_pump_electric_kw"].as_f64().unwrap()
            > high["heat_pump_electric_kw"].as_f64().unwrap()
    );
}

#[test]
fn joint_heat_pump_map_follows_cdl_modes_and_physical_outage() {
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    // Keep a cool outdoor condition from erasing the cooling demand
    // through the optional teaching free-cooling path.
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
            .find(|block| block["name"] == "economizer_use_teaching_low_limit")
            .unwrap()["parameters"][0]["value"] = json!(true);
    }
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 10.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let map = json!({
        "outdoor_c":[10.0],"hot_water_c":[45.0],"chilled_water_c":[7.0],
        "load_fraction":[0.0,1.0],
        "values":[
            {"heating_capacity_fraction":0.75,"cooling_capacity_fraction":0.5,"heating_cop":2.0,"cooling_cop":4.0,"simultaneous_cop":6.0},
            {"heating_capacity_fraction":0.75,"cooling_capacity_fraction":0.5,"heating_cop":2.0,"cooling_cop":4.0,"simultaneous_cop":6.0}
        ]
    });
    let mode_trace = "FourPipeG36Controls|output.plant_heat_pump_requested_mode";
    let run = |temperatures: [f64; 10],
               heat: f64,
               cool: f64,
               outage: bool,
               simultaneous_cop: Option<f64>| {
        let mut performance = map.clone();
        for value in performance["values"].as_array_mut().unwrap() {
            if let Some(cop) = simultaneous_cop {
                value["simultaneous_cop"] = json!(cop);
            } else {
                value.as_object_mut().unwrap().remove("simultaneous_cop");
            }
        }
        let scenario = json!({
            "heat_pump_joint_performance":performance,
            "equipment_outages": if outage {
                vec![json!({"equipment":"heat_pump","start_hour":7.75,"duration_hours":1.0})]
            } else { vec![] }
        });
        let mut sim =
            Simulation::new_test(&graph.to_string(), &weather, &scenario.to_string()).unwrap();
        sim.set_traces_json(&json!([mode_trace]).to_string())
            .unwrap();
        start_present_window(&mut sim);
        sim.temps = temperatures;
        sim.last_heat_request = heat;
        sim.last_cool_request = cool;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        run_weather_intervals(&mut sim, 1)["rows"][0].clone()
    };
    let heating = run([16.0; 10], 100.0, 0.0, false, Some(6.0));
    let cooling = run([28.0; 10], 0.0, 100.0, false, Some(6.0));
    let mixed = run(
        [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0],
        100.0,
        100.0,
        false,
        Some(6.0),
    );
    let mixed_higher_cop = run(
        [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0],
        100.0,
        100.0,
        false,
        Some(12.0),
    );
    let mixed_legacy = run(
        [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0],
        100.0,
        100.0,
        false,
        None,
    );
    let unavailable = run([16.0; 10], 100.0, 0.0, true, Some(6.0));
    assert_eq!(heating["traces"][mode_trace], json!(1.0));
    assert_eq!(cooling["traces"][mode_trace], json!(2.0));
    assert_eq!(mixed["traces"][mode_trace], json!(3.0));
    for row in [&heating, &cooling, &mixed, &unavailable] {
        assert_eq!(row["heat_pump_heating_available_capacity_kw"], json!(60.0));
        assert_eq!(row["heat_pump_cooling_available_capacity_kw"], json!(40.0));
        let heat = row["heat_pump_heating_used_kw"].as_f64().unwrap();
        let cool = row["heat_pump_cooling_used_kw"].as_f64().unwrap();
        let work = row["heat_pump_electric_kw"].as_f64().unwrap();
        let expected = if row["traces"][mode_trace].as_f64() == Some(3.0) {
            (heat + cool) / 6.0
        } else {
            heat_pump_compressor_work_kw(heat, cool, 2.0, 4.0, false, 1.0)
        };
        assert!((work - expected).abs() < 1e-9);
        let ambient = row["heat_pump_ambient_exchange_kw"].as_f64().unwrap();
        assert!((heat - cool - work - ambient).abs() < 1e-9);
    }
    assert!(heating["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
    assert!(cooling["heat_pump_cooling_used_kw"].as_f64().unwrap() > 0.0);
    assert!(mixed["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
    assert!(mixed["heat_pump_cooling_used_kw"].as_f64().unwrap() > 0.0);
    assert_eq!(
        mixed["traces"][mode_trace],
        mixed_higher_cop["traces"][mode_trace]
    );
    assert_eq!(
        mixed["heat_pump_heating_used_kw"],
        mixed_higher_cop["heat_pump_heating_used_kw"]
    );
    assert_eq!(
        mixed["heat_pump_cooling_used_kw"],
        mixed_higher_cop["heat_pump_cooling_used_kw"]
    );
    assert_eq!(
        mixed["traces"][mode_trace],
        mixed_legacy["traces"][mode_trace]
    );
    assert_eq!(
        mixed["heat_pump_heating_used_kw"],
        mixed_legacy["heat_pump_heating_used_kw"]
    );
    assert_eq!(
        mixed["heat_pump_cooling_used_kw"],
        mixed_legacy["heat_pump_cooling_used_kw"]
    );
    let legacy_heat = mixed_legacy["heat_pump_heating_used_kw"].as_f64().unwrap();
    let legacy_cool = mixed_legacy["heat_pump_cooling_used_kw"].as_f64().unwrap();
    let legacy_work = heat_pump_compressor_work_kw(legacy_heat, legacy_cool, 2.0, 4.0, true, 1.0);
    assert!((mixed_legacy["heat_pump_electric_kw"].as_f64().unwrap() - legacy_work).abs() < 1e-9);
    assert!(
        (mixed_higher_cop["heat_pump_electric_kw"].as_f64().unwrap() * 2.0
            - mixed["heat_pump_electric_kw"].as_f64().unwrap())
        .abs()
            < 1e-9
    );
    let run_ratio_curve = |cops: [f64; 2]| {
        let mut performance = map.clone();
        performance["simultaneous_heating_fraction"] = json!([0.2, 0.8]);
        for value in performance["values"].as_array_mut().unwrap() {
            value.as_object_mut().unwrap().remove("simultaneous_cop");
            value["simultaneous_cop_by_heating_fraction"] = json!(cops);
        }
        let scenario = json!({"heat_pump_joint_performance": performance});
        let mut sim =
            Simulation::new_test(&graph.to_string(), &weather, &scenario.to_string()).unwrap();
        sim.set_traces_json(&json!([mode_trace]).to_string())
            .unwrap();
        start_present_window(&mut sim);
        sim.temps = [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0];
        sim.last_heat_request = 100.0;
        sim.last_cool_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        run_weather_intervals(&mut sim, 1)["rows"][0].clone()
    };
    let ratio = run_ratio_curve([4.0, 12.0]);
    let ratio_double = run_ratio_curve([8.0, 24.0]);
    assert_eq!(ratio["traces"][mode_trace], json!(3.0));
    assert_eq!(
        ratio["heat_pump_heating_used_kw"],
        ratio_double["heat_pump_heating_used_kw"]
    );
    assert_eq!(
        ratio["heat_pump_cooling_used_kw"],
        ratio_double["heat_pump_cooling_used_kw"]
    );
    let ratio_heat = ratio["heat_pump_heating_used_kw"].as_f64().unwrap();
    let ratio_cool = ratio["heat_pump_cooling_used_kw"].as_f64().unwrap();
    let ratio_fraction = (ratio_heat / (ratio_heat + ratio_cool)).clamp(0.2, 0.8);
    let ratio_cop = 4.0 + (ratio_fraction - 0.2) / 0.6 * 8.0;
    let ratio_work = ratio["heat_pump_electric_kw"].as_f64().unwrap();
    assert!((ratio_work - (ratio_heat + ratio_cool) / ratio_cop).abs() < 1e-9);
    assert!(
        (ratio_double["heat_pump_electric_kw"].as_f64().unwrap() * 2.0 - ratio_work).abs() < 1e-9
    );
    assert_eq!(unavailable["heat_pump_heating_used_kw"], json!(0.0));
    assert_eq!(unavailable["heat_pump_electric_kw"], json!(0.0));
}

#[test]
fn g36_capacity_input_moves_physical_heat_between_heat_pump_and_boiler() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
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
    let scenario = |capacity: f64| {
        format!(
            r#"{{"heat_pump_capacity_kw":{capacity},"boiler_capacity_kw":1000,"zone_ua_kw_per_k":1.0,"zone_capacity_kwh_per_k":50.0}}"#
        )
    };
    let mut low = Simulation::new_test(graph, &weather, &scenario(10.0)).unwrap();
    let mut high = Simulation::new_test(graph, &weather, &scenario(80.0)).unwrap();
    for simulation in [&mut low, &mut high] {
        start_present_window(simulation);
        simulation.temps = [16.0; 10];
        simulation.last_heat_request = 100.0;
    }
    let low_rows = run_weather_intervals(&mut low, 2);
    let high_rows = run_weather_intervals(&mut high, 2);
    let low_hp: f64 = (0..2)
        .map(|i| {
            low_rows["rows"][i]["heat_pump_heating_used_kw"]
                .as_f64()
                .unwrap()
        })
        .sum();
    let high_hp: f64 = (0..2)
        .map(|i| {
            high_rows["rows"][i]["heat_pump_heating_used_kw"]
                .as_f64()
                .unwrap()
        })
        .sum();
    let low_boiler: f64 = (0..2)
        .map(|i| low_rows["rows"][i]["boiler_used_kw"].as_f64().unwrap())
        .sum();
    let high_boiler: f64 = (0..2)
        .map(|i| high_rows["rows"][i]["boiler_used_kw"].as_f64().unwrap())
        .sum();
    assert!(
        high_hp > low_hp + 20.0,
        "first load-step HP output: low={low_hp}, high={high_hp}"
    );
    assert!(
        low_boiler > high_boiler + 20.0,
        "first load-step boiler output: low={low_boiler}, high={high_boiler}"
    );
    let derated_scenario = json!({
        "heat_pump_capacity_kw": 80.0,
        "boiler_capacity_kw": 1000.0,
        "zone_ua_kw_per_k": 1.0,
        "zone_capacity_kwh_per_k": 50.0,
        "heat_pump_hot_water_cop_performance": [
            {"water_c": 35.0, "cop_multiplier": 1.0, "capacity_multiplier": 0.125},
            {"water_c": 55.0, "cop_multiplier": 1.0, "capacity_multiplier": 0.125}
        ]
    });
    let mut derated = Simulation::new_test(graph, &weather, &derated_scenario.to_string()).unwrap();
    start_present_window(&mut derated);
    derated.temps = [16.0; 10];
    derated.last_heat_request = 100.0;
    let derated_rows = run_weather_intervals(&mut derated, 2);
    let derated_hp: f64 = (0..2)
        .map(|i| {
            derated_rows["rows"][i]["heat_pump_heating_used_kw"]
                .as_f64()
                .unwrap()
        })
        .sum();
    let derated_boiler: f64 = (0..2)
        .map(|i| derated_rows["rows"][i]["boiler_used_kw"].as_f64().unwrap())
        .sum();
    assert!(
        derated_hp < high_hp - 20.0,
        "water derating did not reduce heat-pump delivery"
    );
    assert!(
        derated_boiler > high_boiler + 20.0,
        "CDL did not command backup for derating"
    );
    for row in derated_rows["rows"].as_array().unwrap() {
        let base = row["heat_pump_available_capacity_kw"].as_f64().unwrap();
        let offered = row["heat_pump_heating_available_capacity_kw"]
            .as_f64()
            .unwrap();
        assert!((offered - base * 0.125).abs() < 1e-9);
        assert!(row["heat_pump_heating_used_kw"].as_f64().unwrap() <= offered + 1e-9);
    }
}

#[test]
fn simultaneous_compressor_savings_affects_only_physical_work() {
    let default: Scenario = serde_json::from_str("{}").unwrap();
    assert_eq!(default.heat_recovery_compressor_savings_fraction, 1.0);
    let selected: Scenario =
        serde_json::from_str(r#"{"heat_recovery_compressor_savings_fraction":0.5}"#).unwrap();
    assert_eq!(selected.heat_recovery_compressor_savings_fraction, 0.5);

    let heat_kw = 60.0;
    let cool_kw = 30.0;
    let heat_cop = 3.0;
    let cool_cop = 3.0;
    for (fraction, expected_work) in [(0.0, 30.0), (0.5, 25.0), (1.0, 20.0)] {
        let work =
            heat_pump_compressor_work_kw(heat_kw, cool_kw, heat_cop, cool_cop, true, fraction);
        assert_eq!(work, expected_work);
        let ambient_exchange = heat_kw - cool_kw - work;
        assert_eq!(heat_kw, cool_kw + work + ambient_exchange);
        assert_eq!(
            heat_pump_compressor_work_kw(heat_kw, cool_kw, heat_cop, cool_cop, false, fraction),
            30.0,
        );
    }
}

#[test]
fn g36_simultaneous_mode_drives_shared_compressor_and_closes_energy_balance() {
    let mut graph_value: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    // Keep this plant-mode test independent of the optional free-cooling
    // experiment: its 10 °C outdoor air would otherwise remove the cooling load.
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
    let mut separate: Value = serde_json::from_str(&graph).unwrap();
    let plant = separate["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "plant")
        .unwrap();
    plant["connections"]
        .as_array_mut()
        .unwrap()
        .retain(|edge| edge["to"] != "heat_pump_requested_mode");
    plant["connections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"from":"heat_pump_heating_mode_code.y","to":"heat_pump_requested_mode"}));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 10.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut recovered = Simulation::new_test(&graph, &weather, "{}").unwrap();
    let mut independent = Simulation::new_test(&separate.to_string(), &weather, "{}").unwrap();
    let mut renamed_graph: Value = serde_json::from_str(&graph).unwrap();
    renamed_graph["name"] = json!("CustomBuildingSequence");
    let mut renamed = Simulation::new_test(&renamed_graph.to_string(), &weather, "{}").unwrap();
    let mode_trace = "FourPipeG36Controls|output.plant_heat_pump_requested_mode";
    for sim in [&mut recovered, &mut independent] {
        sim.set_traces_json(&json!([mode_trace]).to_string())
            .unwrap();
        start_present_window(sim);
        sim.temps = [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0];
        sim.last_heat_request = 100.0;
        sim.last_cool_request = 100.0;
    }
    start_present_window(&mut renamed);
    renamed.temps = [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0];
    renamed.last_heat_request = 100.0;
    renamed.last_cool_request = 100.0;
    let recovered_rows = run_weather_intervals(&mut recovered, 3);
    let independent_rows = run_weather_intervals(&mut independent, 3);
    let renamed_rows = run_weather_intervals(&mut renamed, 3);
    for i in 0..3 {
        let recovery = &recovered_rows["rows"][i];
        let separate = &independent_rows["rows"][i];
        let renamed = &renamed_rows["rows"][i];
        let heat = recovery["heat_pump_heating_used_kw"].as_f64().unwrap();
        let cool = recovery["heat_pump_cooling_used_kw"].as_f64().unwrap();
        let work = recovery["heat_pump_electric_kw"].as_f64().unwrap();
        let ambient = recovery["heat_pump_ambient_exchange_kw"].as_f64().unwrap();
        assert!(
            heat > 0.0 && cool > 0.0,
            "row {i}: heat={heat}, cool={cool}, mode={:?}",
            recovery["traces"][mode_trace]
        );
        assert_eq!(recovery["traces"][mode_trace], json!(3.0));
        assert_eq!(
            renamed["heat_pump_electric_kw"],
            recovery["heat_pump_electric_kw"]
        );
        assert_eq!(renamed["electric_kw"], recovery["electric_kw"]);
        assert_eq!(separate["traces"][mode_trace], json!(1.0));
        assert!((heat - cool - work - ambient).abs() < 1e-9);
        assert!(separate["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
        assert_eq!(separate["heat_pump_cooling_used_kw"], json!(0.0));
        let separate_heat = separate["heat_pump_heating_used_kw"].as_f64().unwrap();
        let separate_work = separate["heat_pump_electric_kw"].as_f64().unwrap();
        let separate_ambient = separate["heat_pump_ambient_exchange_kw"].as_f64().unwrap();
        assert!((separate_heat - separate_work - separate_ambient).abs() < 1e-9);
    }
}

#[test]
fn conventional_cdl_mode_changes_physical_compressor_delivery() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let mut separate: Value = serde_json::from_str(graph).unwrap();
    let plant = separate["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "plant")
        .unwrap();
    plant["connections"]
        .as_array_mut()
        .unwrap()
        .retain(|edge| edge["to"] != "heat_pump_requested_mode");
    plant["connections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"from":"heat_pump_heating_mode_code.y","to":"heat_pump_requested_mode"}));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 10.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut recovered = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut independent = Simulation::new_test(&separate.to_string(), &weather, "{}").unwrap();
    let trace = "FourPipeBuildingControls|output.plant_heat_pump_requested_mode";
    for sim in [&mut recovered, &mut independent] {
        sim.set_traces_json(&json!([trace]).to_string()).unwrap();
        start_present_window(sim);
        sim.temps = [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0];
        sim.last_heat_request = 100.0;
        sim.last_cool_request = 100.0;
    }
    let recovery = run_weather_intervals(&mut recovered, 1);
    let separate = run_weather_intervals(&mut independent, 1);
    let shared = &recovery["rows"][0];
    let independent = &separate["rows"][0];
    assert_eq!(shared["traces"][trace], json!(3.0));
    assert_eq!(independent["traces"][trace], json!(1.0));
    assert!(
        independent["heat_pump_heating_used_kw"].as_f64().unwrap()
            >= shared["heat_pump_heating_used_kw"].as_f64().unwrap()
    );
    assert!(shared["heat_pump_cooling_used_kw"].as_f64().unwrap() > 0.0);
    assert_eq!(independent["heat_pump_cooling_used_kw"], json!(0.0));
    assert!(independent["heat_pump_electric_kw"].as_f64().unwrap() > 0.0);
    for (mode, expected_heat, expected_cool) in [(0, false, false), (2, false, true)] {
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        let plant = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "plant")
            .unwrap();
        plant["connections"]
            .as_array_mut()
            .unwrap()
            .retain(|edge| edge["to"] != "heat_pump_requested_mode");
        plant["connections"].as_array_mut().unwrap().push(
            json!({"from":"heat_pump_mode_cooling_weight.y","to":"heat_pump_requested_mode"}),
        );
        plant["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "heat_pump_mode_cooling_weight")
            .unwrap()["parameters"][0]["value"] = json!(mode);
        let mut sim = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
        start_present_window(&mut sim);
        sim.temps = [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0];
        sim.last_heat_request = 100.0;
        sim.last_cool_request = 100.0;
        let mut dynamic = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
        dynamic.oracle = Some(cdl_runtime_oracle::Executor::default());
        start_present_window(&mut dynamic);
        dynamic.temps = sim.temps;
        dynamic.last_heat_request = sim.last_heat_request;
        dynamic.last_cool_request = sim.last_cool_request;
        let row = run_weather_intervals(&mut sim, 1)["rows"][0].clone();
        let dynamic_row = run_weather_intervals(&mut dynamic, 1)["rows"][0].clone();
        assert_eq!(
            row["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0,
            expected_heat
        );
        assert_eq!(
            row["heat_pump_cooling_used_kw"].as_f64().unwrap() > 0.0,
            expected_cool
        );
        // Source staging is a separate CDL output. Editing only the mode
        // can leave the source running with zero compatible thermal
        // service; the harness does not infer an off command from mode.
        assert!(sim.source_running[0]);
        if mode == 0 {
            assert_eq!(row["heat_pump_electric_kw"], json!(0.0));
        }
        assert_eq!(
            row["heat_pump_heating_used_kw"],
            dynamic_row["heat_pump_heating_used_kw"]
        );
        assert_eq!(
            row["heat_pump_cooling_used_kw"],
            dynamic_row["heat_pump_cooling_used_kw"]
        );
    }
}

#[test]
fn water_actuators_follow_cdl_outputs_even_when_enable_alias_is_false() {
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
    let edges = plant["connections"].as_array_mut().unwrap();
    edges.retain(|edge| {
        !(edge["from"] == "hot_water_pump_select.y" && edge["to"] == "hot_water_pump_speed")
            && !(edge["from"] == "boiler_enable_gate.y" && edge["to"] == "boiler_kw")
            && !(edge["from"] == "boiler_enable_gate.y" && edge["to"] == "boiler_start_positive.u1")
    });
    edges.push(json!({"from":"one.y","to":"hot_water_pump_speed"}));
    edges.push(json!({"from":"boiler.y","to":"boiler_kw"}));
    edges.push(json!({"from":"boiler.y","to":"boiler_start_positive.u1"}));
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
    let mut gated = Simulation::new_test(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/four_pipe_building.json"
        )),
        &weather,
        "{}",
    )
    .unwrap();
    gated.last_water_supply_c[0] = 40.0;
    let gated_result: Value = serde_json::from_str(&gated.step_chunk_json(1).unwrap()).unwrap();
    assert_eq!(gated_result["rows"][0]["hot_water_enable"], json!(0.0));
    assert_eq!(gated_result["rows"][0]["boiler_used_kw"], json!(0.0));
    let mut sim = Simulation::new_test(&graph.to_string(), &weather, "{}").unwrap();
    sim.last_water_supply_c[0] = 40.0;
    let result: Value = serde_json::from_str(&sim.step_chunk_json(1).unwrap()).unwrap();
    let row = &result["rows"][0];
    assert_eq!(row["hot_water_enable"], json!(0.0));
    assert_eq!(row["hot_water_pump_speed"], json!(1.0));
    assert!(row["hot_water_flow_kg_s"].as_f64().unwrap() > 0.0);
    assert!(row["boiler_used_kw"].as_f64().unwrap() > 0.0);
}

#[test]
fn g36_water_enables_are_wired_to_editable_pump_commands_and_physical_flow() {
    let bundled = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let mut edited: Value = serde_json::from_str(bundled).unwrap();
    let plant = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "plant")
        .unwrap();
    for (source, status, pump) in [
        (
            "freeze_hot_water_pump_enable",
            "hot_water_enable",
            "hot_water_pump_select.u3",
        ),
        (
            "freeze_chilled_water_pump_enable",
            "chilled_water_enable",
            "chilled_water_pump_select.u3",
        ),
    ] {
        let connections = plant["connections"].as_array_mut().unwrap();
        assert!(connections
            .iter()
            .any(|edge| edge["from"] == format!("{source}.y") && edge["to"] == status));
        assert!(connections
            .iter()
            .any(|edge| edge["from"] == format!("{source}.y") && edge["to"] == pump));
        connections.retain(|edge| {
            !edge["to"]
                .as_str()
                .unwrap()
                .starts_with(&format!("{source}."))
        });
        let enable = plant["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == source)
            .unwrap();
        enable["block_type"] = json!("Buildings.Controls.OBC.CDL.Logical.Sources.Constant");
        enable["inputs"] = json!([]);
        enable["parameters"] = json!([{"name":"k","type":"Boolean","value":false}]);
    }
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
    let mut baseline = Simulation::new_test(bundled, &weather, "{}").unwrap();
    let mut disabled = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    start_present_window(&mut baseline);
    start_present_window(&mut disabled);
    baseline.temps = [18.0; 10];
    disabled.temps = [18.0; 10];
    baseline.last_heat_request = 200.0;
    disabled.last_heat_request = 200.0;
    let normal = run_weather_intervals(&mut baseline, 1);
    let off = run_weather_intervals(&mut disabled, 1);
    assert!(normal["rows"][0]["hot_water_enable"].as_f64().unwrap() > 0.0);
    assert!(normal["rows"][0]["hot_water_flow_kg_s"].as_f64().unwrap() > 0.0);
    assert_eq!(off["rows"][0]["hot_water_enable"], json!(0.0));
    assert_eq!(off["rows"][0]["hot_water_pump_speed"], json!(0.0));
    assert_eq!(off["rows"][0]["hot_water_flow_kg_s"], json!(0.0));
    let cooling_weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 32.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut cooling = Simulation::new_test(bundled, &cooling_weather, "{}").unwrap();
    let mut cooling_disabled =
        Simulation::new_test(&edited.to_string(), &cooling_weather, "{}").unwrap();
    start_present_window(&mut cooling);
    start_present_window(&mut cooling_disabled);
    cooling.temps = [29.0; 10];
    cooling_disabled.temps = [29.0; 10];
    cooling.last_cool_request = 200.0;
    cooling_disabled.last_cool_request = 200.0;
    let normal_cooling = run_weather_intervals(&mut cooling, 1);
    let off_cooling = run_weather_intervals(&mut cooling_disabled, 1);
    assert!(
        normal_cooling["rows"][0]["chilled_water_enable"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        normal_cooling["rows"][0]["chilled_water_flow_kg_s"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(off_cooling["rows"][0]["chilled_water_enable"], json!(0.0));
    assert_eq!(
        off_cooling["rows"][0]["chilled_water_pump_speed"],
        json!(0.0)
    );
    assert_eq!(
        off_cooling["rows"][0]["chilled_water_flow_kg_s"],
        json!(0.0)
    );
}

#[test]
fn g36_editable_water_pressure_target_changes_pump_flow_and_delivery() {
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
    let scenario = r#"{"zone_ua_kw_per_k":1.0,"zone_capacity_kwh_per_k":50.0}"#;
    let target = "FourPipeG36Controls|output.plant_hot_dp_setpoint_pa";
    let mut baseline = Simulation::new_test(graph, &weather, scenario).unwrap();
    baseline
        .set_traces_json(&json!([target]).to_string())
        .unwrap();
    start_present_window(&mut baseline);
    baseline.temps = [18.0; 10];
    baseline.last_heat_request = 200.0;
    let normal = run_weather_intervals(&mut baseline, 8);
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    let plant = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "plant")
        .unwrap();
    plant["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "hot_dp_minimum")
        .unwrap()["parameters"][0]["value"] = json!(30_000.0);
    let mut raised = Simulation::new_test(&edited.to_string(), &weather, scenario).unwrap();
    raised
        .set_traces_json(&json!([target]).to_string())
        .unwrap();
    start_present_window(&mut raised);
    raised.temps = [18.0; 10];
    raised.last_heat_request = 200.0;
    let higher = run_weather_intervals(&mut raised, 8);
    let base_row = &normal["rows"][5];
    let high_row = &higher["rows"][5];
    assert!(
        high_row["traces"][target].as_f64().unwrap()
            > base_row["traces"][target].as_f64().unwrap() + 8_000.0,
        "base target={}, raised target={}, base enable={}, raised enable={}",
        base_row["traces"][target],
        high_row["traces"][target],
        base_row["hot_water_enable"],
        high_row["hot_water_enable"]
    );
    assert!(
        high_row["hot_water_pump_speed"].as_f64().unwrap()
            > base_row["hot_water_pump_speed"].as_f64().unwrap() + 0.05
    );
    assert!(
        (high_row["hot_water_differential_pressure_pa"]
            .as_f64()
            .unwrap()
            - base_row["hot_water_differential_pressure_pa"]
                .as_f64()
                .unwrap())
        .abs()
            > 500.0,
        "base pressure={}, raised pressure={}",
        base_row["hot_water_differential_pressure_pa"],
        high_row["hot_water_differential_pressure_pa"]
    );
    assert!(
        (high_row["hot_water_flow_kg_s"].as_f64().unwrap()
            - base_row["hot_water_flow_kg_s"].as_f64().unwrap())
        .abs()
            > 0.01,
        "base flow={}, high-target flow={}",
        base_row["hot_water_flow_kg_s"],
        high_row["hot_water_flow_kg_s"]
    );
}

#[test]
fn sampled_building_step_resolves_editable_cdl_periods() {
    let g36 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let conventional = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/weather_2025.json"
    ));
    let g36_graph: Block = serde_json::from_str(g36).unwrap();
    for (requested, expected) in [(900, 15), (300, 15), (60, 15), (30, 15), (15, 15)] {
        assert_eq!(sampled_step(&g36_graph, requested).unwrap(), expected);
    }
    let mut aligned = Simulation::new_test(g36, weather, "{}").unwrap();
    assert!(
        serde_json::from_str::<Value>(&aligned.step_chunk_json(59).unwrap()).unwrap()["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let boundary: Value = serde_json::from_str(&aligned.step_chunk_json(1).unwrap()).unwrap();
    assert_eq!(boundary["rows"].as_array().unwrap().len(), 1);
    assert_eq!(boundary["rows"][0]["index"], json!(0));
    assert_eq!(
        Simulation::new_test(conventional, weather, "{}")
            .unwrap()
            .effective_step_seconds(),
        900
    );
}

#[test]
fn both_summary_transports_share_totals_and_distinguish_comfort_populations() {
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
    let mut sim = Simulation::new_test(graph, &weather, "{}").unwrap();
    sim.temps = [0.0; 10];
    let chunk: Value = serde_json::from_str(&sim.step_chunk_json(1).unwrap()).unwrap();
    let worker_summary: Value = serde_json::from_str(&sim.summary_json()).unwrap();
    assert_eq!(chunk["summary"], worker_summary);
    let row = &chunk["rows"][0];
    assert_eq!(worker_summary["unmet_zone_hours"], json!(2.5));
    assert_eq!(worker_summary["occupied_unmet_zone_hours"], json!(0.0));
    for (total, power) in [("electricity_kwh", "electric_kw"), ("gas_kwh", "gas_kw")] {
        let expected = row[power].as_f64().unwrap() * 0.25;
        let actual = worker_summary[total].as_f64().unwrap();
        assert!(
            (actual - expected).abs() < 1e-9,
            "{total}: {actual} vs {expected}"
        );
    }
}

#[test]
fn present_zone_comfort_is_a_subset_of_all_zone_comfort() {
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
    let scenario = json!({
        "occupant_count_events": [{
            "zone": 1, "count": 8.0, "start_hour": 0.0, "duration_hours": 1.0
        }],
        "occupancy_sensor_events": [{
            "zone": 1, "present": false, "start_hour": 0.0, "duration_hours": 1.0
        }]
    });
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    sim.temps = [0.0; 10];
    let chunk: Value = serde_json::from_str(&sim.step_chunk_json(1).unwrap()).unwrap();
    let totals = &chunk["summary"];
    assert_eq!(totals["unmet_zone_hours"], json!(2.5));
    assert_eq!(totals["occupied_unmet_zone_hours"], json!(0.25));
    assert_eq!(sim.raw_occupancy[0], false);
    assert_eq!(sim.actual_occupant_count[0], 8.0);
    assert_eq!(
        totals,
        &serde_json::from_str::<Value>(&sim.summary_json()).unwrap()
    );
}

#[test]
fn recorded_interval_power_reconciles_with_cumulative_energy_across_chunks() {
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
    let scenario = json!({"step_seconds": 60, "ahu_fan_startup_seconds": [300.0, 300.0]});
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    let first_partial: Value = serde_json::from_str(&sim.step_chunk_json(7).unwrap()).unwrap();
    assert!(first_partial["rows"].as_array().unwrap().is_empty());
    let first_complete: Value = serde_json::from_str(&sim.step_chunk_json(8).unwrap()).unwrap();
    let next_complete: Value = serde_json::from_str(&sim.step_chunk_json(15).unwrap()).unwrap();
    let first = &first_complete["rows"][0];
    let second = &next_complete["rows"][0];
    for (total, power) in [("electricity_kwh", "electric_kw"), ("gas_kwh", "gas_kw")] {
        let first_energy = first[power].as_f64().unwrap() * 0.25;
        let second_energy = second[power].as_f64().unwrap() * 0.25;
        let first_total = first_complete["summary"][total].as_f64().unwrap();
        let second_total = next_complete["summary"][total].as_f64().unwrap();
        assert!((first_total - first_energy).abs() < 1e-9, "first {total}");
        assert!(
            (second_total - first_energy - second_energy).abs() < 1e-9,
            "second {total}"
        );
    }
}

#[test]
fn water_volume_changes_physical_storage_without_changing_first_cdl_request() {
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
    let mut normal = Simulation::new_test(graph, &weather, r#"{"step_seconds":900}"#).unwrap();
    let mut large = Simulation::new_test(
        graph,
        &weather,
        r#"{"step_seconds":900,"water_volume_kg":[10000,10000]}"#,
    )
    .unwrap();
    let normal_rows: Value = serde_json::from_str(&normal.step_chunk_json(96).unwrap()).unwrap();
    let large_rows: Value = serde_json::from_str(&large.step_chunk_json(96).unwrap()).unwrap();
    assert_eq!(
        normal_rows["rows"][0]["heat_request_kw"],
        large_rows["rows"][0]["heat_request_kw"]
    );
    assert_eq!(
        normal_rows["rows"][0]["cool_request_kw"],
        large_rows["rows"][0]["cool_request_kw"]
    );
    let normal_row = &normal_rows["rows"][95];
    let large_row = &large_rows["rows"][95];
    for loop_index in 0..2 {
        let normal_capacity = normal_row["water_capacity_kwh_per_k"][loop_index]
            .as_f64()
            .unwrap();
        let large_capacity = large_row["water_capacity_kwh_per_k"][loop_index]
            .as_f64()
            .unwrap();
        assert!((large_capacity - 2.0 * normal_capacity).abs() < 1e-9);
        for row in [normal_row, large_row] {
            let start = row["water_interval_start_c"][loop_index].as_f64().unwrap();
            let end = row["water_interval_end_c"][loop_index].as_f64().unwrap();
            let source = row["water_interval_source_kwh"][loop_index]
                .as_f64()
                .unwrap();
            let delivered = row["water_interval_delivered_kwh"][loop_index]
                .as_f64()
                .unwrap();
            let storage = if loop_index == 0 {
                end - start
            } else {
                start - end
            };
            let capacity = row["water_capacity_kwh_per_k"][loop_index]
                .as_f64()
                .unwrap();
            assert!((storage * capacity - (source - delivered)).abs() < 1e-8);
        }
    }
    assert_ne!(normal.last_water_supply_c[0], large.last_water_supply_c[0]);
    assert_ne!(normal_row["heat_request_kw"], large_row["heat_request_kw"]);
}
