use super::*;

#[test]
fn g36_supply_air_reset_responds_to_vav_cooling_requests_in_cdl() {
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
        ("mixed_air_temp".into(), json!(20.0)),
        ("actual_supply_air_temp".into(), json!(16.0)),
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
    let mut low = HashMap::new();
    for _ in 0..90 {
        low = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(fast["supply_air_temp"], low["supply_air_temp"]);
    }
    assert_eq!(low["sat_request_count"], json!(0));
    let warm_target = low["supply_air_temp"].as_f64().unwrap();
    assert_eq!(warm_target, 18.0);
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_sat_request"), json!(3));
    }
    let mut high = HashMap::new();
    for _ in 0..90 {
        high = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(fast["sat_request_count"], high["sat_request_count"]);
        assert_eq!(fast["supply_air_temp"], high["supply_air_temp"]);
    }
    assert_eq!(high["sat_request_count"], json!(15));
    assert!(high["supply_air_temp"].as_f64().unwrap() < warm_target - 1.0);
    let mut edited = ahu.clone();
    edited
        .blocks
        .iter_mut()
        .find(|block| block.name == "sat_response_gain")
        .unwrap()
        .parameters[0]
        .value = json!(0.0);
    let mut without_response = Executor::default();
    let mut neutral = HashMap::new();
    for _ in 0..90 {
        neutral = without_response
            .execute(&edited, &inputs, 60.0, "")
            .unwrap();
    }
    assert_eq!(neutral["supply_air_temp"], json!(18.0));
}

#[test]
fn g36_supply_air_reset_edit_changes_physical_delivery() {
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 20.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let mut reset = Simulation::new_test(
        &graph.to_string(),
        &weather,
        "{\"zone_capacity_kwh_per_k\":50}",
    )
    .unwrap();
    reset.temps = [27.0; 10];
    let reset_rows = run_weather_intervals(&mut reset, 48);
    for ahu in graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|block| block["block_type"] == "AHUController")
    {
        ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "sat_response_gain")
            .unwrap()["parameters"][0]["value"] = json!(0.0);
    }
    let mut neutral = Simulation::new_test(
        &graph.to_string(),
        &weather,
        "{\"zone_capacity_kwh_per_k\":50}",
    )
    .unwrap();
    neutral.temps = [27.0; 10];
    let neutral_rows = run_weather_intervals(&mut neutral, 48);
    let minimum = |rows: &Value, name: &str| {
        rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row[name].as_f64().unwrap())
            .fold(f64::INFINITY, f64::min)
    };
    assert!(
        minimum(&reset_rows, "ahu_1_supply_setpoint_c") < 15.5,
        "minimum reset target {}",
        minimum(&reset_rows, "ahu_1_supply_setpoint_c")
    );
    assert!(
        minimum(&reset_rows, "ahu_1_supply_setpoint_c")
            < minimum(&neutral_rows, "ahu_1_supply_setpoint_c") - 0.1,
        "request response must lower the outdoor-reset target"
    );
    assert!(
        minimum(&reset_rows, "ahu_1_supply_actual_c")
            < minimum(&neutral_rows, "ahu_1_supply_actual_c") - 0.01
    );
}

#[test]
fn g36_water_reset_holds_for_source_stage_changes_in_both_executors() {
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
            let value = match port.kind.as_str() {
                "Boolean" => json!(false),
                "Integer" => json!(0),
                _ => json!(0.0),
            };
            (port.name.clone(), value)
        })
        .collect();
    set_equipment_ready(&mut inputs);
    for (name, value) in [
        ("cool_request_kw", 100.0),
        ("heat_pump_available_capacity_kw", 80.0),
        ("chilled_water_flow_kg_s", 2.0),
        ("chilled_water_supply_temp_c", 7.0),
        ("ahu_1_supply_setpoint_c", 16.0),
        ("ahu_2_supply_setpoint_c", 16.0),
        ("ahu_1_supply_actual_c", 16.0),
        ("ahu_2_supply_actual_c", 16.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
    let mut last = 1.0;
    for step in 0..30 {
        let row = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            row["chilled_water_reset_fraction"],
            fast["chilled_water_reset_fraction"]
        );
        last = row["chilled_water_reset_fraction"].as_f64().unwrap();
        if step < 14 {
            assert_eq!(last, 1.0);
        }
    }
    assert!(
        last < 1.0,
        "reset should trim after the 900-second stage hold"
    );
    inputs.insert("cool_request_kw".into(), json!(10.0));
    inputs.insert("chilled_water_supply_temp_c".into(), json!(4.0));
    let mut held = None;
    for _step in 0..10 {
        let row = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            row["chilled_water_reset_fraction"],
            fast["chilled_water_reset_fraction"]
        );
        let current = row["chilled_water_reset_fraction"].as_f64().unwrap();
        if let Some(previous) = held {
            assert_eq!(
                current, previous,
                "source-stage change should hold the previous sampled reset"
            );
        }
        held = Some(current);
    }
    let mut tuned = plant.clone();
    tuned
        .blocks
        .iter_mut()
        .find(|block| block.name == "chilled_stage_hold_delay")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "delayTime")
        .unwrap()
        .value = json!(120.0);
    let mut edited = Executor::default();
    let mut tuned_inputs = inputs.clone();
    tuned_inputs.insert("cool_request_kw".into(), json!(100.0));
    tuned_inputs.insert("chilled_water_supply_temp_c".into(), json!(7.0));
    let mut early = 1.0;
    // The first call is t=0; the reset clock reaches t=600 on call 11.
    for _ in 0..11 {
        early = edited.execute(&tuned, &tuned_inputs, 60.0, "").unwrap()
            ["chilled_water_reset_fraction"]
            .as_f64()
            .unwrap();
    }
    assert!(
        early < 1.0,
        "editing the CDL hold time should release reset sooner"
    );
}

#[test]
fn g36_water_reset_waits_for_physical_source_running_proof() {
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
        ("heat_pump_available_capacity_kw", 80.0),
        ("hot_water_flow_kg_s", 2.0),
        ("hot_water_supply_temp_c", 45.0),
    ] {
        inputs.insert(name.into(), json!(value));
    }
    inputs.insert("heat_pump_running".into(), json!(false));
    let trace = "plant|hot_stage_hold_delay.y".to_string();
    let mut dynamic = Executor::default();
    dynamic.trace_targets = vec![trace.clone()];
    let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
    compiled.set_traces(&[trace.clone()]).unwrap();
    let active = |value: &Value| {
        value
            .as_bool()
            .unwrap_or_else(|| value.as_f64().unwrap() != 0.0)
    };
    for _ in 0..20 {
        dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let (_, fast_traces) = compiled.execute(&inputs, 60.0, true).unwrap();
        assert!(!active(&dynamic.trace_values[&trace]));
        assert_eq!(
            active(&dynamic.trace_values[&trace]),
            active(&fast_traces[&trace])
        );
    }
    inputs.insert("heat_pump_running".into(), json!(true));
    let mut released = false;
    for _ in 0..25 {
        dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let (_, fast_traces) = compiled.execute(&inputs, 60.0, true).unwrap();
        released = active(&dynamic.trace_values[&trace]);
        assert_eq!(released, active(&fast_traces[&trace]));
    }
    assert!(
        released,
        "the sampled reset should release after physical source proof and its CDL hold time"
    );
}

#[test]
fn g36_chilled_water_reset_responds_to_ahu_requests_in_cdl() {
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
    let mut inputs = HashMap::from([
        ("heat_request_kw".into(), json!(0.0)),
        ("cool_request_kw".into(), json!(100.0)),
        ("heat_pump_available_capacity_kw".into(), json!(80.0)),
        ("hot_water_supply_temp_c".into(), json!(45.0)),
        ("hot_water_return_temp_c".into(), json!(40.0)),
        ("chilled_water_supply_temp_c".into(), json!(7.0)),
        ("chilled_water_return_temp_c".into(), json!(12.0)),
        ("hot_water_flow_kg_s".into(), json!(0.0)),
        ("chilled_water_flow_kg_s".into(), json!(2.0)),
        ("hot_water_differential_pressure_pa".into(), json!(0.0)),
        (
            "chilled_water_differential_pressure_pa".into(),
            json!(25_000.0),
        ),
        ("ahu_1_cooling_valve".into(), json!(0.0)),
        ("ahu_2_cooling_valve".into(), json!(0.0)),
        ("ahu_1_heating_valve".into(), json!(0.0)),
        ("ahu_2_heating_valve".into(), json!(0.0)),
        ("ahu_1_supply_setpoint_c".into(), json!(16.0)),
        ("ahu_2_supply_setpoint_c".into(), json!(16.0)),
        ("ahu_1_supply_actual_c".into(), json!(16.0)),
        ("ahu_2_supply_actual_c".into(), json!(16.0)),
        ("ahu_1_freeze_hot_water_request".into(), json!(0)),
        ("ahu_2_freeze_hot_water_request".into(), json!(0)),
        ("ahu_1_freeze_chilled_water_request".into(), json!(false)),
        ("ahu_2_freeze_chilled_water_request".into(), json!(false)),
    ]);
    set_equipment_ready(&mut inputs);
    for zone in 1..=10 {
        inputs.insert(format!("zone_{zone:02}_hot_water_reset_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_hot_water_plant_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
    let mut low_request = HashMap::new();
    for _ in 0..90 {
        low_request = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            fast["chilled_water_supply_setpoint_c"],
            low_request["chilled_water_supply_setpoint_c"]
        );
        assert_eq!(
            fast["chilled_water_reset_fraction"],
            low_request["chilled_water_reset_fraction"]
        );
    }
    let warm_target = low_request["chilled_water_supply_setpoint_c"]
        .as_f64()
        .unwrap();
    let low_reset = low_request["chilled_water_reset_fraction"]
        .as_f64()
        .unwrap();
    let expected_low_target = 7.0 + 5.0 * (1.0 - (2.0 * (low_reset - 0.5).max(0.0)));
    assert!((warm_target - expected_low_target).abs() < 1e-9);
    assert!(
        (low_request["chilled_dp_setpoint_pa"].as_f64().unwrap()
            - (15_000.0 + 40_000.0 * low_reset.min(0.5)))
        .abs()
            < 1e-9
    );
    assert!(
        warm_target > 7.0,
        "reset should warm the water with no AHU requests"
    );
    inputs.insert("ahu_1_cooling_valve".into(), json!(1.0));
    inputs.insert("ahu_2_cooling_valve".into(), json!(1.0));
    inputs.insert("ahu_1_supply_actual_c".into(), json!(24.0));
    inputs.insert("ahu_2_supply_actual_c".into(), json!(24.0));
    let mut high_request = HashMap::new();
    for _ in 0..90 {
        high_request = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            fast["chilled_water_supply_setpoint_c"],
            high_request["chilled_water_supply_setpoint_c"]
        );
    }
    assert!(
        high_request["chilled_water_supply_setpoint_c"]
            .as_f64()
            .unwrap()
            < warm_target
    );
    let high_reset = high_request["chilled_water_reset_fraction"]
        .as_f64()
        .unwrap();
    let expected_high_target = 7.0 + 5.0 * (1.0 - (2.0 * (high_reset - 0.5).max(0.0)));
    assert!(
        (high_request["chilled_water_supply_setpoint_c"]
            .as_f64()
            .unwrap()
            - expected_high_target)
            .abs()
            < 1e-9
    );
    assert!(
        high_request["chilled_water_reset_fraction"]
            .as_f64()
            .unwrap()
            > low_request["chilled_water_reset_fraction"]
                .as_f64()
                .unwrap()
    );
    let mut first_half_plant = plant.clone();
    first_half_plant
        .blocks
        .iter_mut()
        .find(|block| block.name == "chilled_reset_previous")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "y_start")
        .unwrap()
        .value = json!(0.25);
    first_half_plant
        .blocks
        .iter_mut()
        .find(|block| block.name == "chilled_reset_held_value")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "y_start")
        .unwrap()
        .value = json!(0.25);
    let first_half = Executor::default()
        .execute(&first_half_plant, &inputs, 60.0, "")
        .unwrap();
    assert_eq!(first_half["chilled_water_supply_setpoint_c"], json!(12.0));
    assert_eq!(first_half["chilled_dp_setpoint_pa"], json!(25_000.0));
}

#[test]
fn g36_hot_water_reset_responds_to_ahu_requests_in_cdl() {
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
    for (target, source) in [
        (
            ("hot_reset_request_real", "u"),
            ("hot_reset_with_freeze_requests", "y"),
        ),
        (
            ("hot_reset_with_freeze_requests", "u1"),
            ("hot_reset_all_request_count", "y"),
        ),
    ] {
        let drivers: Vec<_> = plant
            .connections
            .iter()
            .filter(|edge| edge.target() == (target.0.to_owned(), target.1.to_owned()))
            .map(Connection::source)
            .collect();
        assert_eq!(drivers, vec![(source.0.to_owned(), source.1.to_owned())]);
    }
    let mut inputs = HashMap::from([
        ("heat_request_kw".into(), json!(100.0)),
        ("cool_request_kw".into(), json!(0.0)),
        ("heat_pump_available_capacity_kw".into(), json!(80.0)),
        ("hot_water_supply_temp_c".into(), json!(45.0)),
        ("hot_water_return_temp_c".into(), json!(40.0)),
        ("chilled_water_supply_temp_c".into(), json!(7.0)),
        ("chilled_water_return_temp_c".into(), json!(12.0)),
        ("hot_water_flow_kg_s".into(), json!(2.0)),
        ("chilled_water_flow_kg_s".into(), json!(0.0)),
        ("hot_water_differential_pressure_pa".into(), json!(25_000.0)),
        ("chilled_water_differential_pressure_pa".into(), json!(0.0)),
        ("ahu_1_cooling_valve".into(), json!(0.0)),
        ("ahu_2_cooling_valve".into(), json!(0.0)),
        ("ahu_1_heating_valve".into(), json!(0.0)),
        ("ahu_2_heating_valve".into(), json!(0.0)),
        ("ahu_1_supply_setpoint_c".into(), json!(16.0)),
        ("ahu_2_supply_setpoint_c".into(), json!(16.0)),
        ("ahu_1_supply_actual_c".into(), json!(16.0)),
        ("ahu_2_supply_actual_c".into(), json!(16.0)),
        ("ahu_1_freeze_hot_water_request".into(), json!(0)),
        ("ahu_2_freeze_hot_water_request".into(), json!(0)),
        ("ahu_1_freeze_chilled_water_request".into(), json!(false)),
        ("ahu_2_freeze_chilled_water_request".into(), json!(false)),
    ]);
    set_equipment_ready(&mut inputs);
    for zone in 1..=10 {
        inputs.insert(format!("zone_{zone:02}_hot_water_reset_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_hot_water_plant_request"), json!(0));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
    let mut low = HashMap::new();
    for _ in 0..90 {
        low = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            fast["hot_water_supply_setpoint_c"],
            low["hot_water_supply_setpoint_c"]
        );
    }
    let low_target = low["hot_water_supply_setpoint_c"].as_f64().unwrap();
    assert!(
        low_target < 45.0,
        "no requests should trim the hot-water target"
    );
    inputs.insert("heat_request_kw".into(), json!(0.0));
    inputs.insert("ahu_1_freeze_hot_water_request".into(), json!(2));
    let mut freeze = HashMap::new();
    for _ in 0..6 {
        freeze = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            fast["hot_water_supply_setpoint_c"],
            freeze["hot_water_supply_setpoint_c"]
        );
    }
    assert_eq!(freeze["hot_water_enable"], json!(true));
    assert!(freeze["hot_water_pump_speed"].as_f64().unwrap() >= 0.15);
    assert!(freeze["hot_water_supply_setpoint_c"].as_f64().unwrap() >= 45.0);
    inputs.insert("ahu_1_freeze_chilled_water_request".into(), json!(true));
    let chilled = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
    let chilled_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(chilled["chilled_water_enable"], json!(true));
    assert!(chilled["chilled_water_pump_speed"].as_f64().unwrap() >= 0.15);
    assert_eq!(chilled_fast["chilled_water_enable"], json!(true));
    inputs.insert("ahu_1_freeze_chilled_water_request".into(), json!(false));
    inputs.insert("heat_request_kw".into(), json!(100.0));
    inputs.insert("ahu_1_freeze_hot_water_request".into(), json!(0));
    add_ahu_ventilation_inputs(&mut inputs);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_hot_water_reset_request"), json!(1));
        inputs.insert(format!("zone_{zone:02}_hot_water_plant_request"), json!(1));
    }
    let mut terminal = HashMap::new();
    for step in 0..90 {
        terminal = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            fast["terminal_hot_water_request_count"], terminal["terminal_hot_water_request_count"],
            "step {step}"
        );
        assert_eq!(
            fast["hot_water_supply_setpoint_c"],
            terminal["hot_water_supply_setpoint_c"]
        );
    }
    assert_eq!(terminal["terminal_hot_water_request_count"], json!(5));
    assert_eq!(terminal["terminal_hot_water_plant_request_count"], json!(5));
    assert!(terminal["hot_water_supply_setpoint_c"].as_f64().unwrap() > low_target + 1.0);
    for zone in 1..=5 {
        inputs.insert(format!("zone_{zone:02}_hot_water_reset_request"), json!(0));
        inputs.insert(format!("zone_{zone:02}_hot_water_plant_request"), json!(0));
    }
    let cleared = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
    let cleared_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
    assert_eq!(cleared["terminal_hot_water_request_count"], json!(0));
    assert_eq!(cleared["terminal_hot_water_plant_request_count"], json!(0));
    assert_eq!(cleared_fast["terminal_hot_water_request_count"], json!(0));
    assert_eq!(
        cleared_fast["terminal_hot_water_plant_request_count"],
        json!(0)
    );
    inputs.insert("ahu_1_heating_valve".into(), json!(1.0));
    inputs.insert("ahu_2_heating_valve".into(), json!(1.0));
    inputs.insert("ahu_1_supply_actual_c".into(), json!(-2.0));
    inputs.insert("ahu_2_supply_actual_c".into(), json!(-2.0));
    let mut high = HashMap::new();
    for _ in 0..90 {
        high = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            fast["hot_water_supply_setpoint_c"],
            high["hot_water_supply_setpoint_c"]
        );
    }
    assert!(high["hot_water_supply_setpoint_c"].as_f64().unwrap() > low_target + 1.0);
    assert!(
        high["hot_water_reset_fraction"].as_f64().unwrap()
            > low["hot_water_reset_fraction"].as_f64().unwrap()
    );
}

#[test]
fn g36_hot_water_reset_edit_reaches_physical_supply_sensor() {
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
    let scenario = r#"{"design_occupants_per_zone":0}"#;
    let mut reset = Simulation::new_test(&graph.to_string(), &weather, scenario).unwrap();
    let reset_rows = run_weather_intervals(&mut reset, 48);
    let plant = graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "plant")
        .unwrap();
    plant["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "hot_reset_warm_delta")
        .unwrap()["parameters"][0]["value"] = json!(0.0);
    let mut fixed = Simulation::new_test(&graph.to_string(), &weather, scenario).unwrap();
    let fixed_rows = run_weather_intervals(&mut fixed, 48);
    assert!(fixed_rows["rows"].as_array().unwrap().iter().all(|row| row
        ["hot_water_supply_setpoint_c"]
        .as_f64()
        .unwrap()
        == 45.0));
    let reset_target = reset_rows["rows"].as_array().unwrap().last().unwrap()
        ["hot_water_supply_setpoint_c"]
        .as_f64()
        .unwrap();
    let reset_supply = reset_rows["rows"].as_array().unwrap().last().unwrap()
        ["hot_water_supply_temp_c"]
        .as_f64()
        .unwrap();
    let fixed_supply = fixed_rows["rows"].as_array().unwrap().last().unwrap()
        ["hot_water_supply_temp_c"]
        .as_f64()
        .unwrap();
    assert!(
        reset_target < 45.0,
        "low request should trim the CDL hot-water target"
    );
    assert!(
        reset_supply < fixed_supply - 0.01,
        "graph reset should change physical supply: {reset_supply} < {fixed_supply}"
    );
}

#[test]
fn g36_co2_feedback_changes_airflow_and_zone_concentration_from_cdl() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 22.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let scenario = r#"{"zone_air_volume_m3":100,"co2_generation_lps_per_person":0.02}"#;
    let mut active = Simulation::new_test(graph, &weather, scenario).unwrap();
    let active_rows = run_weather_intervals(&mut active, 55);
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    for zone in edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|block| block["block_type"] == "VAVBoxController")
    {
        zone["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "co2_cooling_max_flow")
            .unwrap()["parameters"][0]["value"] = json!(0.2);
    }
    let mut muted = Simulation::new_test(&edited.to_string(), &weather, scenario).unwrap();
    let muted_rows = run_weather_intervals(&mut muted, 55);
    let active_last = active_rows["rows"].as_array().unwrap().last().unwrap();
    let muted_last = muted_rows["rows"].as_array().unwrap().last().unwrap();
    let peak_airflow = |rows: &Value| {
        rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["zone_airflow"][0].as_f64().unwrap())
            .fold(0.0_f64, f64::max)
    };
    assert!(
        peak_airflow(&active_rows) > peak_airflow(&muted_rows) + 0.02,
        "active peak flow {}, muted peak flow {}, active CO2 {}, muted CO2 {}",
        peak_airflow(&active_rows),
        peak_airflow(&muted_rows),
        active_last["zone_co2_ppm"][0],
        muted_last["zone_co2_ppm"][0]
    );
    assert!(
        active_last["zone_co2_ppm"][0].as_f64().unwrap()
            < muted_last["zone_co2_ppm"][0].as_f64().unwrap() - 50.0
    );
}

#[test]
fn per_zone_design_occupants_feed_cdl_ventilation_and_physical_co2() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 22.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let scenario = json!({
        "step_seconds": 60,
        "zone_design_occupants": [16, 0, 8, 8, 8, 8, 8, 8, 8, 8],
    });
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    let population_one = "FourPipeG36Controls|output.zone_01_ventilation_population";
    let population_two = "FourPipeG36Controls|output.zone_02_ventilation_population";
    sim.set_traces_json(&json!([population_one, population_two]).to_string())
        .unwrap();
    sim.cursor = 32 * (900 / sim.effective_step_seconds);
    sim.co2_ppm = [1_600.0; 10];
    sim.step_chunk_json(1).unwrap();
    assert!(sim.raw_occupancy[0] && sim.raw_occupancy[1]);
    assert_eq!(sim.actual_occupant_count[0], 16.0);
    assert_eq!(sim.actual_occupant_count[1], 0.0);
    assert!(sim.co2_ppm[0] > sim.co2_ppm[1]);
    let rows: Value = serde_json::from_str(
        &sim.step_chunk_json(900 / sim.effective_step_seconds() - 1)
            .unwrap(),
    )
    .unwrap();
    let row = &rows["rows"][0];
    assert!(
        row["traces"][population_one].as_f64().unwrap()
            > row["traces"][population_two].as_f64().unwrap(),
        "zone-specific raw counts should reach editable CDL ventilation"
    );
}

#[test]
fn occupancy_count_sensor_fault_changes_cdl_but_not_physical_people() {
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
    let base = json!({"step_seconds":60,"zone_design_occupants":[16,8,8,8,8,8,8,8,8,8],
        "ahu_fan_startup_seconds":[1200,1200]});
    let fault = json!({"step_seconds":60,"zone_design_occupants":[16,8,8,8,8,8,8,8,8,8],
        "ahu_fan_startup_seconds":[1200,1200],
        "occupancy_count_sensor_events":[{"zone":1,"count":0.0,"start_hour":8.0,"duration_hours":0.25}]});
    let mut normal = Simulation::new_test(graph, &weather, &base.to_string()).unwrap();
    let mut biased = Simulation::new_test(graph, &weather, &fault.to_string()).unwrap();
    let trace = "FourPipeG36Controls|output.zone_01_ventilation_population";
    for sim in [&mut normal, &mut biased] {
        sim.set_traces_json(&json!([trace]).to_string()).unwrap();
        sim.cursor = 32 * (900 / sim.effective_step_seconds);
        sim.co2_ppm = [1_600.0; 10];
    }
    let normal_row = run_weather_intervals(&mut normal, 1);
    let biased_row = run_weather_intervals(&mut biased, 1);
    assert_eq!(normal.actual_occupant_count[0], 16.0);
    assert_eq!(biased.actual_occupant_count[0], 16.0);
    assert_eq!(normal.raw_occupant_count[0], 16.0);
    assert_eq!(biased.raw_occupant_count[0], 0.0);
    assert_eq!(normal.co2_ppm[0], biased.co2_ppm[0]);
    assert_eq!(normal.temps[0], biased.temps[0]);
    assert!(
        normal_row["rows"][0]["traces"][trace].as_f64().unwrap()
            > biased_row["rows"][0]["traces"][trace].as_f64().unwrap(),
        "normal={} biased={}",
        normal_row["rows"][0]["traces"][trace],
        biased_row["rows"][0]["traces"][trace]
    );
    assert_eq!(
        biased_row["rows"][0]["zone_occupant_count_sensor"][0],
        json!(0.0)
    );
    assert_eq!(
        biased_row["rows"][0]["zone_occupant_count_actual"][0],
        json!(16.0)
    );
}

#[test]
fn per_zone_air_properties_change_physical_co2_and_heat() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 22.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let scenario = json!({
        "step_seconds": 60,
        "zone_air_volume_m3_by_zone": [100, 1000, 250, 250, 250, 250, 250, 250, 250, 250],
        "zone_infiltration_m3_s_by_zone": [0, 0, 0, 0.5, 0, 0, 0.01, 0.01, 0.01, 0.01],
        "open_window_additional_infiltration_m3_s_by_zone": [0, 0, 0, 0, 0, 0.5, 0, 0, 0, 0],
        "occupant_count_events": (1..=6).map(|zone| json!({
            "zone": zone, "count": 8.0, "start_hour": 7.75, "duration_hours": 0.25
        })).collect::<Vec<_>>(),
        "window_openings": [
            {"zone": 5, "start_hour": 7.75, "duration_hours": 0.25},
            {"zone": 6, "start_hour": 7.75, "duration_hours": 0.25},
        ],
    });
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    start_present_window(&mut sim);
    sim.temps = [24.0; 10];
    sim.co2_ppm = [
        420.0, 420.0, 1600.0, 1600.0, 1600.0, 1600.0, 420.0, 420.0, 420.0, 420.0,
    ];
    sim.step_chunk_json(1).unwrap();
    assert!(sim.raw_occupancy[..6].iter().all(|present| *present));
    assert!(
        sim.co2_ppm[0] > sim.co2_ppm[1],
        "smaller air volume should respond faster"
    );
    assert!(
        sim.co2_ppm[2] > sim.co2_ppm[3],
        "higher infiltration should dilute CO2"
    );
    assert!(
        sim.co2_ppm[4] > sim.co2_ppm[5],
        "open-window airflow should dilute CO2"
    );
    assert!(
        sim.temps[4] > sim.temps[5],
        "open-window airflow should cool the affected zone"
    );
}

#[test]
fn g36_water_reset_edit_reaches_physical_supply_sensor() {
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 26.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    for ahu in graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|block| block["block_type"] == "AHUController")
    {
        ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "sat_minimum")
            .unwrap()["parameters"][0]["value"] = json!(18.0);
    }
    let mut normal = Simulation::new_test(&graph.to_string(), &weather, "{}").unwrap();
    let normal_rows = run_weather_intervals(&mut normal, 48);
    let normal_supply = normal_rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["chilled_water_supply_temp_c"].as_f64().unwrap())
        .fold(f64::NEG_INFINITY, f64::max);
    let plant = graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "plant")
        .unwrap();
    plant["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "chilled_reset_warm_delta")
        .unwrap()["parameters"][0]["value"] = json!(0.0);
    let mut fixed = Simulation::new_test(&graph.to_string(), &weather, "{}").unwrap();
    let fixed_rows = run_weather_intervals(&mut fixed, 48);
    assert!(
        normal_supply > 7.0,
        "the sampled CDL reset should warm physical supply water"
    );
    assert!(fixed_rows["rows"].as_array().unwrap().iter().all(|row| row
        ["chilled_water_supply_setpoint_c"]
        .as_f64()
        .unwrap()
        == 7.0));
    let fixed_supply = fixed_rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["chilled_water_supply_temp_c"].as_f64().unwrap())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        normal_supply > fixed_supply + 0.01,
        "graph reset should change physical supply: {normal_supply} > {fixed_supply}"
    );
}

#[test]
fn g36_building_exercises_zone_pressure_requests() {
    let mut graph: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    for ahu in graph["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|b| b["block_type"] == "AHUController")
    {
        let pi = ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|b| b["name"] == "duct_pressure_pi")
            .unwrap();
        pi["parameters"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["name"] == "yMax")
            .unwrap()["value"] = json!(0.05);
        let limit = ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|b| b["name"] == "fan_limit")
            .unwrap();
        limit["parameters"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["name"] == "uMax")
            .unwrap()["value"] = json!(0.05);
    }
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
    let mut sim = Simulation::new_test(&graph.to_string(), &weather, "{}").unwrap();
    sim.temps = [30.0; 10];
    let target = "FourPipeG36Controls|output.ahu_1_pressure_request_count";
    sim.set_traces_json(&json!([target]).to_string()).unwrap();
    let mut highest = 0.0_f64;
    while sim.cursor < 96 * (900 / sim.effective_step_seconds) {
        let result: Value = serde_json::from_str(&sim.step_chunk_json(672).unwrap()).unwrap();
        for row in result["rows"].as_array().unwrap() {
            highest = highest.max(row["traces"][target].as_f64().unwrap_or(0.0));
        }
    }
    assert!(
        highest > 0.0,
        "the physical example never exercised pressure reset requests"
    );
}

#[test]
fn g36_sat_suppression_follows_pinned_obc_positive_start_trace() {
    assert_g36_sat_suppression_trace(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../verification/g36_time_suppression_positive_dymola.txt"
        )),
        0.0,
        1000,
    );
}

#[test]
fn g36_sat_suppression_follows_pinned_obc_negative_start_trace() {
    assert_g36_sat_suppression_trace(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../verification/g36_time_suppression_negative_dymola.txt"
        )),
        -360.0,
        600,
    );
}

#[test]
fn g36_sample_uses_debounced_presence_for_setpoint_modes() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let mut weather = vec![
        Weather {
            outdoor_c: 22.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ];
    for sample in &mut weather[70..87] {
        sample.occupied = false;
    }
    let weather = serde_json::to_string(&weather).unwrap();
    let mut sim = Simulation::new_test(graph, &weather, r#"{"step_seconds":15}"#).unwrap();
    let trace = "FourPipeG36Controls.zone_01|cool_setpoint_active.y";
    sim.set_traces_json(&serde_json::to_string(&vec![trace]).unwrap())
        .unwrap();
    while sim.cursor < 31 * 60 {
        sim.step_chunk_json((31 * 60 - sim.cursor).min(672))
            .unwrap();
    }
    sim.step_chunk_json(3).unwrap();
    assert!(
        !sim.zone_occupied[0],
        "presence must persist for one minute"
    );
    sim.step_chunk_json(2).unwrap();
    assert!(sim.zone_occupied[0]);
    let mut values = HashMap::new();
    while sim.cursor < 71 * 60 {
        let chunk: Value = serde_json::from_str(&sim.step_chunk_json(672).unwrap()).unwrap();
        for row in chunk["rows"].as_array().unwrap() {
            let index = row["index"].as_u64().unwrap();
            if [31, 48, 70].contains(&index) {
                values.insert(index, row.clone());
            }
        }
    }
    assert_eq!(values[&31]["zone_occupied"][0], 1.0);
    assert_eq!(values[&31]["traces"][trace], 24.0);
    assert_eq!(values[&48]["zone_occupied"][0], 0.0);
    assert_eq!(values[&48]["traces"][trace], 24.5);
    assert_eq!(values[&70]["schedule_occupied"], 0.0);
    assert_eq!(values[&70]["preconditioning_mode"], 0.0);
    assert_eq!(values[&70]["traces"][trace], 80.0);
}

#[test]
fn g36_vacancy_requires_five_continuous_minutes() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 22.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut sim = Simulation::new_test(graph, &weather, r#"{"step_seconds":15}"#).unwrap();
    while sim.cursor < 48 * 60 {
        sim.step_chunk_json((48 * 60 - sim.cursor).min(672))
            .unwrap();
    }
    assert!(sim.zone_occupied[0]);
    sim.step_chunk_json(1).unwrap();
    assert!(!sim.raw_occupancy[0]);
    sim.step_chunk_json(18).unwrap();
    assert!(sim.zone_occupied[0]);
    sim.step_chunk_json(2).unwrap();
    assert!(!sim.zone_occupied[0]);
}

#[test]
fn g36_timing_decisions_change_when_cdl_parameters_change() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    ));
    let mut weather = vec![
        Weather {
            outdoor_c: 22.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ];
    for sample in &mut weather[70..87] {
        sample.occupied = false;
    }
    let weather = serde_json::to_string(&weather).unwrap();
    let mut reference = Simulation::new_test(graph, &weather, "{}").unwrap();
    reference.cursor = 80 * (900 / reference.effective_step_seconds);
    reference.temps = [19.0; 10];
    let reference_start = run_weather_intervals(&mut reference, 1);
    let mut reference = Simulation::new_test(graph, &weather, "{}").unwrap();
    let reference = run_weather_intervals(&mut reference, 34);
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    let horizon = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "preconditioning_horizon")
        .unwrap();
    horizon["parameters"][0]["value"] = json!(1.0);
    let zone = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "zone_01")
        .unwrap();
    let arrival = zone["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "presence_delay")
        .unwrap();
    arrival["parameters"][0]["value"] = json!(1800.0);
    let mut changed = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    changed.cursor = 80 * (900 / changed.effective_step_seconds);
    changed.temps = [19.0; 10];
    let changed_start = run_weather_intervals(&mut changed, 1);
    let mut changed = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    let changed = run_weather_intervals(&mut changed, 34);
    assert_eq!(reference["rows"][31]["zone_occupied"][0], 1.0);
    assert_eq!(changed["rows"][31]["zone_occupied"][0], 0.0);
    assert_eq!(changed["rows"][33]["zone_occupied"][0], 1.0);
    assert_eq!(reference_start["rows"][0]["preconditioning_mode"], 1.0);
    assert_eq!(changed_start["rows"][0]["preconditioning_mode"], 0.0);
}

#[test]
fn compiled_g36_timing_matches_dynamic_executor() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/weather_2025.json"
    ));
    let mut fast = Simulation::new_test(graph, weather, "{}").unwrap();
    assert!(fast.oracle.is_none());
    let mut dynamic = Simulation::new_test(graph, weather, "{}").unwrap();
    dynamic.oracle = Some(cdl_runtime_oracle::Executor::default());
    let fast = run_weather_intervals(&mut fast, 96);
    let dynamic = run_weather_intervals(&mut dynamic, 96);
    for (index, (a, b)) in fast["rows"]
        .as_array()
        .unwrap()
        .iter()
        .zip(dynamic["rows"].as_array().unwrap())
        .enumerate()
    {
        assert_eq!(a["preconditioning_mode"], b["preconditioning_mode"]);
        assert_eq!(a["zone_occupied"], b["zone_occupied"]);
        for point in [
            "hot_water_pump_speed",
            "chilled_water_pump_speed",
            "hot_water_differential_pressure_pa",
            "chilled_water_differential_pressure_pa",
        ] {
            assert!(
                (a[point].as_f64().unwrap() - b[point].as_f64().unwrap()).abs() < 1e-9,
                "step {index} {point}: compiled={}, dynamic={}",
                a[point],
                b[point]
            );
        }
        assert!(
            (a["electric_kw"].as_f64().unwrap() - b["electric_kw"].as_f64().unwrap()).abs() < 1e-9
        );
    }
}

#[test]
fn conventional_outdoor_air_command_is_graph_owned() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
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
    let mut normal = Simulation::new_test(graph, &weather, "{}").unwrap();
    let base: Value = serde_json::from_str(&normal.step_chunk_json(3).unwrap()).unwrap();
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    for controller in edited["blocks"].as_array_mut().unwrap() {
        if controller["block_type"] != "AHUController" {
            continue;
        }
        let constant = controller["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "minimum_outdoor_air")
            .unwrap();
        constant["parameters"][0]["value"] = json!(0.8);
    }
    let mut changed = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    let high: Value = serde_json::from_str(&changed.step_chunk_json(3).unwrap()).unwrap();
    assert_eq!(base["rows"][2]["ahu_1_outdoor_air_fraction"], 0.2);
    assert_eq!(high["rows"][2]["ahu_1_outdoor_air_fraction"], 0.8);
    assert!(
        high["rows"][2]["ahu_1_mixed_air_c"].as_f64().unwrap()
            > base["rows"][2]["ahu_1_mixed_air_c"].as_f64().unwrap()
    );
}

#[test]
fn conventional_return_damper_command_changes_physical_air_delivery() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
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
    let mut base = Simulation::new_test(graph, &weather, "{}").unwrap();
    let normal: Value = serde_json::from_str(&base.step_chunk_json(3).unwrap()).unwrap();
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    for ahu in edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|block| block["block_type"] == "AHUController")
    {
        ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "conventional_return_one")
            .unwrap()["parameters"][0]["value"] = json!(0.2);
    }
    let mut changed = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    let restricted: Value = serde_json::from_str(&changed.step_chunk_json(3).unwrap()).unwrap();
    assert_eq!(
        normal["rows"][2]["ahu_1_actual_return_damper_position"],
        json!(0.8)
    );
    assert_eq!(
        restricted["rows"][2]["ahu_1_actual_return_damper_position"],
        json!(0.0)
    );
    assert!(
        restricted["rows"][2]["delivered_airflow"].as_f64().unwrap()
            < normal["rows"][2]["delivered_airflow"].as_f64().unwrap()
    );
    assert!(
        restricted["rows"][2]["ahu_1_outdoor_air_fraction"]
            .as_f64()
            .unwrap()
            > normal["rows"][2]["ahu_1_outdoor_air_fraction"]
                .as_f64()
                .unwrap()
    );

    edited["outputs"]
        .as_array_mut()
        .unwrap()
        .retain(|port| port["name"] != "ahu_1_return_air_damper_position");
    edited["connections"]
        .as_array_mut()
        .unwrap()
        .retain(|edge| edge["to"] != "ahu_1_return_air_damper_position");
    let missing: Block = serde_json::from_value(edited).unwrap();
    assert!(conventional_control_contract(&missing)
        .iter()
        .any(|error| error.contains("ahu_1_return_air_damper_position")));
}

#[test]
fn scheduled_raw_window_opening_reaches_physical_zone() {
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
    let mut closed = Simulation::new_test(graph, &weather, "{}").unwrap();
    let mut opened = Simulation::new_test(
        graph,
        &weather,
        r#"{"window_openings":[{"zone":1,"start_hour":0,"duration_hours":2}]}"#,
    )
    .unwrap();
    closed.temps = [29.0; 10];
    opened.temps = [29.0; 10];
    let normal = run_weather_intervals(&mut closed, 2);
    let with_open_window = run_weather_intervals(&mut opened, 2);
    assert!(
        with_open_window["rows"][1]["zone_airflow"][0]
            .as_f64()
            .unwrap()
            < normal["rows"][1]["zone_airflow"][0].as_f64().unwrap()
    );
    assert!(
        with_open_window["rows"][1]["zone_c"][0].as_f64().unwrap()
            > normal["rows"][1]["zone_c"][0].as_f64().unwrap()
    );
}

#[test]
fn open_window_air_exchange_changes_heat_and_co2_without_changing_cdl_contact() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 32.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false,
        };
        35_040
    ])
    .unwrap();
    let contact = r#""window_openings":[{"zone":1,"start_hour":0,"duration_hours":2}]"#;
    let mut no_exchange = Simulation::new_test(
        graph,
        &weather,
        &format!(r#"{{{contact},"open_window_additional_infiltration_m3_s":0}}"#),
    )
    .unwrap();
    let mut with_exchange = Simulation::new_test(
        graph,
        &weather,
        &format!(r#"{{{contact},"open_window_additional_infiltration_m3_s":0.2}}"#),
    )
    .unwrap();
    no_exchange.temps = [29.0; 10];
    with_exchange.temps = [29.0; 10];
    no_exchange.co2_ppm = [1000.0; 10];
    with_exchange.co2_ppm = [1000.0; 10];
    let still = run_weather_intervals(&mut no_exchange, 1);
    let exchanged = run_weather_intervals(&mut with_exchange, 1);
    let still_row = &still["rows"][0];
    let exchanged_row = &exchanged["rows"][0];
    assert!(
        exchanged_row["zone_c"][0].as_f64().unwrap() > still_row["zone_c"][0].as_f64().unwrap()
    );
    assert!(
        exchanged_row["zone_co2_ppm"][0].as_f64().unwrap()
            < still_row["zone_co2_ppm"][0].as_f64().unwrap()
    );
    assert_eq!(exchanged_row["zone_c"][1], still_row["zone_c"][1]);
    assert_eq!(
        exchanged_row["zone_co2_ppm"][1],
        still_row["zone_co2_ppm"][1]
    );
    assert_eq!(
        exchanged_row["zone_airflow"][0],
        still_row["zone_airflow"][0]
    );
}

#[test]
fn scheduled_plant_operator_contact_reaches_cdl_start_fault_acknowledgement() {
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
        "equipment_start_failures": [{"equipment": "heat_pump", "start_hour": 7.75, "duration_hours": 0.5}],
        "heat_pump_fault_reset_hour": 8.25,
        "plant_alarm_acknowledgements": [{"start_hour": 8.0, "duration_hours": 0.25}]
    });
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    start_present_window(&mut sim);
    sim.temps = [16.0; 10];
    sim.last_heat_request = 100.0;
    sim.last_water_flow_kg_s = [2.0, 2.0];
    sim.last_water_pump_running = [true, true];
    let fault = "FourPipeG36Controls|output.plant_heat_pump_start_fault";
    let acknowledged = "FourPipeG36Controls|output.plant_heat_pump_start_fault_acknowledged";
    sim.set_traces_json(&json!([fault, acknowledged]).to_string())
        .unwrap();
    let rows = run_weather_intervals(&mut sim, 3);
    let rows = rows["rows"].as_array().unwrap();
    assert_eq!(rows[0]["traces"][fault], json!(1.0));
    assert_eq!(rows[0]["traces"][acknowledged], json!(0.0));
    assert_eq!(rows[1]["traces"][fault], json!(1.0));
    assert_eq!(rows[1]["traces"][acknowledged], json!(1.0));
    assert_eq!(rows[2]["traces"][fault], json!(0.0));
    assert_eq!(rows[2]["traces"][acknowledged], json!(0.0));
}

#[test]
fn scheduled_ahu_operator_contact_reaches_cdl_alarm_acknowledgement() {
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
    let scenario = json!({
        "step_seconds": 60,
        "fan_faults": [{"equipment": "ahu_1_stuck_off", "start_hour": 0, "duration_hours": 2}],
        "ahu_alarm_acknowledgements": [{"ahu": 1, "start_hour": 0.5, "duration_hours": 0.25}]
    });
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    sim.temps = [29.0; 10];
    let level = "FourPipeG36Controls|output.ahu_1_supply_fan_alarm_level";
    let acknowledged = "FourPipeG36Controls|output.ahu_1_supply_fan_alarm_acknowledged";
    sim.set_traces_json(&json!([level, acknowledged]).to_string())
        .unwrap();
    let rows = run_weather_intervals(&mut sim, 4);
    let rows = rows["rows"].as_array().unwrap();
    assert!(rows[1]["traces"][level].as_f64().unwrap() > 0.0);
    assert_eq!(rows[1]["traces"][acknowledged], json!(0.0));
    assert_eq!(rows[2]["traces"][acknowledged], json!(1.0));
    assert!(
        rows[2]["traces"][level].as_f64().unwrap() > 0.0,
        "acknowledgement must not suppress the active alarm"
    );
}

#[test]
fn scheduled_group_window_alarm_reaches_ahu_cdl_acknowledgement() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
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
    let scenario = json!({
        "step_seconds": 60,
        "window_openings": [{"zone": 1, "start_hour": 0, "duration_hours": 2}],
        "ahu_alarm_acknowledgements": [{"ahu": 1, "start_hour": 0.5, "duration_hours": 0.25}]
    });
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    let level = "FourPipeG36Controls|output.ahu_1_group_window_open_alarm_level";
    let acknowledged = "FourPipeG36Controls|output.ahu_1_group_window_open_alarm_acknowledged";
    sim.set_traces_json(&json!([level, acknowledged]).to_string())
        .unwrap();
    let rows = run_weather_intervals(&mut sim, 4);
    let rows = rows["rows"].as_array().unwrap();
    assert!(rows[1]["traces"][level].as_f64().unwrap() > 0.0);
    assert_eq!(rows[1]["traces"][acknowledged], json!(0.0));
    assert_eq!(rows[2]["traces"][acknowledged], json!(1.0));
    assert!(rows[2]["traces"][level].as_f64().unwrap() > 0.0);
}

#[test]
fn scheduled_operator_contact_reaches_cdl_alarm_acknowledgement() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 30.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let scenario = json!({
        "step_seconds": 60,
        "terminal_design_flow": [0.01, 0.95, 0.95, 0.95, 0.95, 0.95, 0.95, 0.95, 0.95, 0.95],
        "alarm_acknowledgements": [{"zone": 1, "start_hour": 9.0, "duration_hours": 0.25}]
    });
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    start_present_window(&mut sim);
    sim.temps[0] = 32.0;
    let level = "FourPipeG36Controls|output.zone_01_low_airflow_alarm_level";
    let acknowledged = "FourPipeG36Controls|output.zone_01_low_airflow_acknowledged";
    sim.set_traces_json(&json!([level, acknowledged]).to_string())
        .unwrap();
    let rows = run_weather_intervals(&mut sim, 9);
    let rows = rows["rows"].as_array().unwrap();
    assert!(
        rows[4]["traces"][level].as_f64().unwrap() > 0.0,
        "alarm should be active before scheduled contact: {:?}",
        rows.iter()
            .map(|row| &row["traces"][level])
            .collect::<Vec<_>>()
    );
    assert_eq!(rows[4]["traces"][acknowledged], json!(0.0));
    assert_eq!(rows[5]["traces"][acknowledged], json!(1.0));
    assert!(
        rows[5]["traces"][level].as_f64().unwrap() > 0.0,
        "acknowledgement must not suppress the active alarm"
    );
}

#[test]
fn g36_low_airflow_alarm_matches_pinned_obc_validation_trace() {
    let branch = g36_low_airflow_alarm_branch();
    let archive = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/g36_reheat_alarms_dymola.txt"
    ));
    let expected: Vec<i64> = archive
        .lines()
        .find(|line| line.starts_with("ala.yLowFloAla="))
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
            ("setpoint".into(), json!(0.1 + 0.9 * seconds / 7_200.0)),
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
fn g36_low_airflow_alarm_requires_sustained_new_flow_request() {
    let branch = g36_low_airflow_alarm_branch();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    // The fan and flow deficit have been steady for five minutes while
    // the setpoint stayed below the OBC near-zero guard. A new request
    // must still be present for five minutes before either level appears.
    for tick in 0..=900 {
        let setpoint = if (301..=420).contains(&tick) || tick >= 481 {
            0.1
        } else {
            0.005
        };
        let inputs = HashMap::from([
            ("occupied".into(), json!(true)),
            ("supply_fan_proven".into(), json!(true)),
            ("setpoint".into(), json!(setpoint)),
            ("measured_airflow".into(), json!(0.0)),
        ]);
        let ordinary = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
        let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
        for result in [&ordinary, &fast] {
            assert_eq!(
                result["alarm"],
                json!(if tick >= 781 { 2 } else { 0 }),
                "tick {tick}"
            );
        }
    }
}

#[test]
fn hot_water_plant_proof_reports_heating_source_operation_not_cooling_only() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    ));
    for (outdoor, zone_temp, heat_request, cool_request, expected) in
        [(-5.0, 16.0, 100.0, 0.0, 1.0), (35.0, 32.0, 0.0, 100.0, 0.0)]
    {
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
        let mut sim = Simulation::new_test(graph, &weather, r#"{"step_seconds":60}"#).unwrap();
        start_present_window(&mut sim);
        sim.temps = [zone_temp; 10];
        sim.last_heat_request = heat_request;
        sim.last_cool_request = cool_request;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.last_water_pump_running = [true, true];
        let chunk = run_weather_intervals(&mut sim, 1);
        let row = &chunk["rows"][0];
        assert_eq!(row["hot_water_plant_proven"], json!(expected));
        if expected == 0.0 {
            assert_eq!(row["heat_pump_heating_used_kw"], json!(0.0));
            assert!(row["heat_pump_cooling_used_kw"].as_f64().unwrap() > 0.0);
        } else {
            assert!(row["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
        }
    }
}

#[test]
fn hot_water_plant_proof_remains_true_when_running_source_cannot_charge_full_volume() {
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
        "fan_faults": [
            {"equipment": "ahu_1_stuck_off", "start_hour": 7.75, "duration_hours": 1.0},
            {"equipment": "ahu_2_stuck_off", "start_hour": 7.75, "duration_hours": 1.0}
        ]
    });
    let mut sim = Simulation::new_test(graph, &weather, &scenario.to_string()).unwrap();
    start_present_window(&mut sim);
    sim.temps = [16.0; 10];
    sim.last_heat_request = 100.0;
    sim.last_water_supply_c[0] = 60.0;
    sim.last_water_flow_kg_s = [2.0, 2.0];
    sim.last_water_pump_running = [true, true];
    sim.step_chunk_json(1).unwrap();
    assert!(sim.source_running[0]);
    assert!(sim.last_hot_water_plant_proven);
    assert!(sim.last_zone_airflow.iter().all(|flow| *flow == 0.0));
    assert_eq!(sim.last_water_supply_c[0], 60.0);
}
