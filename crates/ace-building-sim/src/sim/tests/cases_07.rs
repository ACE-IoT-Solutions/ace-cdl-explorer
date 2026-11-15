use super::*;

#[test]
fn fan_gain_changes_delivered_air_and_zone_temperature() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 30.0,
            solar_kw: 12.0,
            internal_kw: 16.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut normal =
        Simulation::new_test(&serde_json::to_string(&graph).unwrap(), &weather, "{}").unwrap();
    let normal_rows: Value = serde_json::from_str(&normal.step_chunk_json(192).unwrap()).unwrap();
    let ahu = graph.blocks.iter_mut().find(|b| b.name == "ahu_1").unwrap();
    ahu.blocks
        .iter_mut()
        .find(|b| b.name == "fan_gain")
        .unwrap()
        .parameters[0]
        .value = json!(0.1);
    let mut slow =
        Simulation::new_test(&serde_json::to_string(&graph).unwrap(), &weather, "{}").unwrap();
    let slow_rows: Value = serde_json::from_str(&slow.step_chunk_json(192).unwrap()).unwrap();
    let normal_last = &normal_rows["rows"][191];
    let slow_last = &slow_rows["rows"][191];
    let ahu_1_flow = |row: &Value| {
        row["zone_airflow"].as_array().unwrap()[..5]
            .iter()
            .map(|value| value.as_f64().unwrap())
            .sum::<f64>()
    };
    let flow_loss = normal_rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .zip(slow_rows["rows"].as_array().unwrap())
        .map(|(normal, slow)| ahu_1_flow(normal) - ahu_1_flow(slow))
        .fold(0.0_f64, f64::max);
    let zone_warming = normal_rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .zip(slow_rows["rows"].as_array().unwrap())
        .map(|(normal, slow)| {
            slow["zone_c"][0].as_f64().unwrap() - normal["zone_c"][0].as_f64().unwrap()
        })
        .fold(0.0_f64, f64::max);
    assert!(
        flow_loss > 0.1,
        "maximum AHU 1 airflow loss {flow_loss}; final flows {} and {}",
        ahu_1_flow(normal_last),
        ahu_1_flow(slow_last)
    );
    assert!(zone_warming > 0.1, "maximum zone warming {zone_warming}");
}

#[test]
fn closing_cdl_damper_stops_flow_despite_airflow_request() {
    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 30.0,
            solar_kw: 12.0,
            internal_kw: 16.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
    let mut open = Simulation::new_test(graph, &weather, "{}").unwrap();
    let reference: Value = serde_json::from_str(&open.step_chunk_json(3).unwrap()).unwrap();
    let mut edited: Value = serde_json::from_str(graph).unwrap();
    let zone = edited["blocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|block| block["name"] == "zone_01")
        .unwrap();
    let edges = zone["connections"].as_array_mut().unwrap();
    edges.retain(|edge| edge["to"] != "damper_position");
    edges.push(json!({"from":"zero.y","to":"damper_position"}));
    let mut closed = Simulation::new_test(&edited.to_string(), &weather, "{}").unwrap();
    let result: Value = serde_json::from_str(&closed.step_chunk_json(3).unwrap()).unwrap();
    assert!(result["rows"][2]["requested_airflow"].as_f64().unwrap() > 0.0);
    assert!(reference["rows"][2]["zone_airflow"][0].as_f64().unwrap() > 0.0);
    assert_eq!(result["rows"][2]["zone_airflow"][0], json!(0.0));
}

#[test]
fn plant_output_limits_supply_air_and_energy_use() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 32.0,
            solar_kw: 12.0,
            internal_kw: 16.0,
            occupied: true,
        };
        35_040
    ])
    .unwrap();
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
        .value = json!(0.0);
    let scenario = "{}";
    let mut normal =
        Simulation::new_test(&serde_json::to_string(&graph).unwrap(), &weather, scenario).unwrap();
    let normal_rows: Value = serde_json::from_str(&normal.step_chunk_json(192).unwrap()).unwrap();
    let plant = graph.blocks.iter_mut().find(|b| b.name == "plant").unwrap();
    plant
        .blocks
        .iter_mut()
        .find(|b| b.name == "one")
        .unwrap()
        .parameters[0]
        .value = json!(0.0);
    let mut unavailable =
        Simulation::new_test(&serde_json::to_string(&graph).unwrap(), &weather, scenario).unwrap();
    let unavailable_rows: Value =
        serde_json::from_str(&unavailable.step_chunk_json(192).unwrap()).unwrap();
    let normal_last = &normal_rows["rows"][191];
    let unavailable_last = &unavailable_rows["rows"][191];
    assert!(normal_last["cool_kw"].as_f64().unwrap() > 0.0);
    assert_eq!(unavailable_last["cool_kw"], json!(0.0));
    assert_eq!(unavailable_last["chiller_used_kw"], json!(0.0));
    assert!(
        unavailable_last["ahu_1_supply_actual_c"].as_f64().unwrap()
            > unavailable_last["ahu_1_supply_setpoint_c"]
                .as_f64()
                .unwrap()
    );
    assert!(
        unavailable_last["zone_c"][0].as_f64().unwrap()
            > normal_last["zone_c"][0].as_f64().unwrap()
    );
    let used_cooling = normal_last["heat_pump_cooling_used_kw"].as_f64().unwrap()
        + normal_last["chiller_used_kw"].as_f64().unwrap();
    let stored_delta = (normal_last["chilled_water_supply_temp_c"].as_f64().unwrap()
        - normal.last_water_supply_c[1])
        * (5_000.0 * 4.18 / 3_600.0)
        / 0.25;
    assert!((normal_last["cool_kw"].as_f64().unwrap() + stored_delta - used_cooling).abs() < 1e-8);
    assert!(
        normal_last["cool_kw"].as_f64().unwrap()
            <= normal_last["cool_request_kw"].as_f64().unwrap() + 1e-8
    );
}

#[test]
fn supply_reset_changes_coil_load_and_zone_temperature() {
    let mut graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 32.0,
            solar_kw: 12.0,
            internal_kw: 16.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let mut normal =
        Simulation::new_test(&serde_json::to_string(&graph).unwrap(), &weather, "{}").unwrap();
    let normal_rows: Value = serde_json::from_str(&normal.step_chunk_json(192).unwrap()).unwrap();
    graph
        .blocks
        .iter_mut()
        .find(|b| b.name == "ahu_1")
        .unwrap()
        .blocks
        .iter_mut()
        .find(|b| b.name == "cooling_supply")
        .unwrap()
        .parameters[0]
        .value = json!(18.0);
    let mut warm =
        Simulation::new_test(&serde_json::to_string(&graph).unwrap(), &weather, "{}").unwrap();
    let warm_rows: Value = serde_json::from_str(&warm.step_chunk_json(192).unwrap()).unwrap();
    let a = &normal_rows["rows"][191];
    let b = &warm_rows["rows"][191];
    assert!(
        b["ahu_1_supply_setpoint_c"].as_f64().unwrap()
            > a["ahu_1_supply_setpoint_c"].as_f64().unwrap()
    );
    assert!(
        (b["ahu_1_supply_actual_c"].as_f64().unwrap()
            - a["ahu_1_supply_actual_c"].as_f64().unwrap()).abs() > 0.1,
        "normal supply {}, warm supply {}, normal target {}, warm target {}, normal cool {}, warm cool {}",
        a["ahu_1_supply_actual_c"], b["ahu_1_supply_actual_c"],
        a["ahu_1_supply_setpoint_c"], b["ahu_1_supply_setpoint_c"],
        a["ahu_1_cooling_coil_delivered_kw"], b["ahu_1_cooling_coil_delivered_kw"]
    );
    assert!((b["zone_c"][0].as_f64().unwrap() - a["zone_c"][0].as_f64().unwrap()).abs() > 0.05,
        "normal zone {}, warm zone {}, normal supply {}, warm supply {}, normal airflow {}, warm airflow {}",
        a["zone_c"][0], b["zone_c"][0], a["ahu_1_supply_actual_c"], b["ahu_1_supply_actual_c"],
        a["zone_airflow"][0], b["zone_airflow"][0]);
    assert!(
        (b["cool_request_kw"].as_f64().unwrap() - a["cool_request_kw"].as_f64().unwrap()).abs()
            > 0.1
    );
}

#[test]
fn plant_dispatch_moves_energy_between_electricity_and_fuel() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    )))
    .unwrap();
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: -5.0,
            solar_kw: 0.0,
            internal_kw: 1.2,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    let dispatch_graph = |fraction: f64| {
        let mut edited = graph.clone();
        edited
            .blocks
            .iter_mut()
            .find(|block| block.name == "plant")
            .unwrap()
            .blocks
            .iter_mut()
            .find(|block| block.name == "heat_pump_dispatch_fraction")
            .unwrap()
            .parameters[0]
            .value = json!(fraction);
        serde_json::to_string(&edited).unwrap()
    };
    let mut boiler = Simulation::new_test(
        &dispatch_graph(0.0),
        &weather,
        r#"{"boiler_capacity_kw":1000}"#,
    )
    .unwrap();
    let mut heat_pump = Simulation::new_test(
        &dispatch_graph(1.0),
        &weather,
        r#"{"heat_pump_capacity_kw":1000}"#,
    )
    .unwrap();
    let mut ignored_legacy_scenario = Simulation::new_test(
        &dispatch_graph(0.0),
        &weather,
        r#"{"heat_pump_fraction":1.0,"boiler_capacity_kw":1000}"#,
    )
    .unwrap();
    let boiler_result: Value = serde_json::from_str(&boiler.step_chunk_json(192).unwrap()).unwrap();
    let hp_result: Value = serde_json::from_str(&heat_pump.step_chunk_json(192).unwrap()).unwrap();
    let legacy_result: Value =
        serde_json::from_str(&ignored_legacy_scenario.step_chunk_json(192).unwrap()).unwrap();
    assert_eq!(
        legacy_result["rows"][191]["gas_kw"],
        boiler_result["rows"][191]["gas_kw"]
    );
    let boiler_last = &boiler_result["rows"][191];
    let hp_last = &hp_result["rows"][191];
    assert!(boiler_last["gas_kw"].as_f64().unwrap() > 0.0);
    assert_eq!(hp_last["gas_kw"], json!(0.0));
    assert!(
        hp_last["electric_kw"].as_f64().unwrap() > boiler_last["electric_kw"].as_f64().unwrap()
    );
    assert!(
        (hp_last["heat_kw"].as_f64().unwrap() - boiler_last["heat_kw"].as_f64().unwrap()).abs()
            < 1e-6
    );
}
