use super::*;
use cdl_runtime_oracle::Executor;

pub(super) fn add_ahu_ventilation_inputs(inputs: &mut HashMap<String, Value>) {
    inputs
        .entry("outdoor_air_enthalpy_kj_kg".into())
        .or_insert(json!(50.0));
    inputs
        .entry("return_air_enthalpy_kj_kg".into())
        .or_insert(json!(55.0));
    inputs.entry("operation_mode".into()).or_insert(json!(1));
    inputs
        .entry("pollution_active".into())
        .or_insert(json!(false));
    for service in ["heating", "cooling"] {
        inputs
            .entry(format!("actual_{service}_coil_valve_position"))
            .or_insert(json!(0.0));
    }
    for zone in 1..=5 {
        let name = format!("zone_{zone:02}");
        for (suffix, value) in [
            ("ventilation_area", 0.03),
            ("ventilation_population", 0.06),
            ("ventilation_minimum_oa_flow", 0.09),
            ("measured_airflow", 0.4),
        ] {
            inputs
                .entry(format!("{name}_{suffix}"))
                .or_insert(json!(value));
        }
    }
}

pub(super) fn set_equipment_ready(inputs: &mut HashMap<String, Value>) {
    for equipment in ["heat_pump", "boiler", "chiller"] {
        inputs.insert(format!("{equipment}_available"), json!(true));
        inputs.insert(format!("{equipment}_running"), json!(true));
        inputs.insert(format!("{equipment}_fault_reset"), json!(false));
    }
    for water in ["hot_water", "chilled_water"] {
        inputs.insert(format!("{water}_pump_running"), json!(true));
    }
    // Graph-only tests isolate the requested dispatch behavior. The
    // physical simulation supplies measured service capacities each step.
    for service in ["heating", "cooling"] {
        inputs.insert(
            format!("heat_pump_{service}_available_capacity_kw"),
            json!(1_000_000.0),
        );
    }
}

pub(super) fn set_backup_stage_delay(plant: &mut Block, seconds: f64) {
    for source in ["boiler", "chiller"] {
        let delay = plant
            .blocks
            .iter_mut()
            .find(|block| block.name == format!("{source}_stage_up_delay"))
            .unwrap();
        delay
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "delayTime")
            .unwrap()
            .value = json!(seconds);
    }
}

pub(super) fn start_present_window(sim: &mut Simulation) {
    sim.cursor = 31 * (900 / sim.effective_step_seconds);
}

pub(super) fn run_weather_intervals(sim: &mut Simulation, intervals: usize) -> Value {
    let target = sim.cursor + intervals * (900 / sim.effective_step_seconds);
    let mut rows = Vec::new();
    while sim.cursor < target {
        let chunk: Value = serde_json::from_str(
            &sim.step_chunk_json(target.saturating_sub(sim.cursor).min(672))
                .unwrap(),
        )
        .unwrap();
        rows.extend(chunk["rows"].as_array().unwrap().iter().cloned());
    }
    json!({"rows": rows})
}

pub(super) fn assert_coil_air_balance(row: &Value, ahu: usize) {
    let start = (ahu - 1) * 5;
    let flow: f64 = row["zone_airflow"].as_array().unwrap()[start..start + 5]
        .iter()
        .map(|value| value.as_f64().unwrap())
        .sum();
    if flow <= 0.0 {
        return;
    }
    let heat = row[format!("ahu_{ahu}_heating_coil_delivered_kw")]
        .as_f64()
        .unwrap();
    let cool = row[format!("ahu_{ahu}_cooling_coil_delivered_kw")]
        .as_f64()
        .unwrap();
    let mixed_h = row[format!("ahu_{ahu}_mixed_air_enthalpy_kj_kg")]
        .as_f64()
        .unwrap();
    let supply_h = row[format!("ahu_{ahu}_supply_air_enthalpy_kj_kg")]
        .as_f64()
        .unwrap();
    assert!((1.2 * flow * (supply_h - mixed_h) - (heat - cool)).abs() < 1e-4);
}

pub(super) fn assert_water_heat_balance(row: &Value) {
    let hot_flow = row["hot_water_actual_flow_kg_s"].as_f64().unwrap();
    let chilled_flow = row["chilled_water_actual_flow_kg_s"].as_f64().unwrap();
    let hot_delta = row["hot_water_supply_temp_c"].as_f64().unwrap()
        - row["hot_water_return_temp_c"].as_f64().unwrap();
    let chilled_delta = row["chilled_water_return_temp_c"].as_f64().unwrap()
        - row["chilled_water_supply_temp_c"].as_f64().unwrap();
    assert!((row["heat_kw"].as_f64().unwrap() - hot_flow * 4.18 * hot_delta).abs() < 1e-8);
    assert!((row["cool_kw"].as_f64().unwrap() - chilled_flow * 4.18 * chilled_delta).abs() < 1e-8);
}

pub(super) fn assert_water_branch_balance(row: &Value) {
    let value = |name: &str| row[name].as_f64().unwrap();
    let hot_sum = value("hot_water_bypass_flow_kg_s")
        + value("ahu_1_hot_water_flow_kg_s")
        + value("ahu_2_hot_water_flow_kg_s")
        + value("hot_water_reheat_flow_kg_s");
    let chilled_sum = value("chilled_water_bypass_flow_kg_s")
        + value("ahu_1_chilled_water_flow_kg_s")
        + value("ahu_2_chilled_water_flow_kg_s");
    assert!((hot_sum - value("hot_water_actual_flow_kg_s")).abs() < 1e-9);
    assert!((chilled_sum - value("chilled_water_actual_flow_kg_s")).abs() < 1e-9);
    for ahu in 1..=2 {
        let hot_capacity = value(&format!("ahu_{ahu}_hot_water_flow_kg_s"))
            * 4.18
            * (value("hot_water_supply_temp_c") - 35.0).max(0.0);
        let chilled_capacity = value(&format!("ahu_{ahu}_chilled_water_flow_kg_s"))
            * 4.18
            * (17.0 - value("chilled_water_supply_temp_c")).max(0.0);
        assert!(value(&format!("ahu_{ahu}_heating_coil_delivered_kw")) <= hot_capacity + 1e-9);
        assert!(value(&format!("ahu_{ahu}_cooling_coil_delivered_kw")) <= chilled_capacity + 1e-9);
    }
}

pub(super) fn g36_pressure_reset_branch() -> Block {
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
    let names: std::collections::HashSet<&str> = [
        "ignored_request_count",
        "request_count_real",
        "pressure_request_sample",
        "ignored_request_real",
        "excess_request_count",
        "zero_requests",
        "positive_request_count",
        "response_per_request",
        "maximum_response",
        "bounded_response",
        "trim_amount",
        "net_pressure_reset",
        "request_delay",
        "initial_pressure_target",
        "low_pressure_target",
        "high_pressure_target",
        "pressure_previous",
        "pressure_target_sum",
        "pressure_above_minimum",
        "pressure_below_maximum",
        "pressure_candidate",
        "pressure_target_output",
    ]
    .into_iter()
    .collect();
    let blocks: Vec<Value> = ahu["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|block| names.contains(block["name"].as_str().unwrap()))
        .cloned()
        .collect();
    assert_eq!(blocks.len(), names.len());
    let mut connections: Vec<Value> = ahu["connections"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|edge| {
            let source = edge["from"].as_str().unwrap().split('.').next().unwrap();
            let target = edge["to"].as_str().unwrap().split('.').next().unwrap();
            names.contains(source) && names.contains(target)
        })
        .cloned()
        .collect();
    connections.extend([
        json!({"from":"request_count","to":"request_count_real.u"}),
        json!({"from":"fan_requested","to":"request_delay.u"}),
        json!({"from":"pressure_target_output.y","to":"setpoint"}),
    ]);
    let graph: Block = serde_json::from_value(json!({
        "name":"PressureResetReference","block_type":"Composite","category":"composite",
        "inputs":[{"name":"request_count","type":"Integer"},
                  {"name":"fan_requested","type":"Boolean"}],
        "outputs":[{"name":"setpoint","type":"Real"}],
        "blocks":blocks,"connections":connections
    }))
    .unwrap();
    assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
    graph
}

pub(super) fn assert_g36_sat_suppression_trace(source: &str, start: f64, step_millis: usize) {
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
    let expected_after = series("timSupCooReq.yAftSup=");
    let expected_duration = series("timSupCooReq.supTim.y=");
    assert_eq!(expected_after.len(), 101);
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
        "name":"CoolingSuppressionReference","block_type":"Composite","category":"composite",
        "inputs":[{"name":"room_temp","type":"Real"},{"name":"cool_setpoint","type":"Real"},
                  {"name":"cooling_loop","type":"Real"}],
        "outputs":[{"name":"after","type":"Boolean"},{"name":"duration","type":"Real"}],
        "blocks":zone["blocks"].as_array().unwrap().iter()
            .filter(|block| block["name"].as_str().unwrap().starts_with("sat_"))
            .cloned().collect::<Vec<_>>(),
        "connections":zone["connections"].as_array().unwrap().iter()
            .filter(|edge| edge["to"].as_str().unwrap().starts_with("sat_"))
            .map(|edge| {
                let mut copy=edge.clone();
                copy["from"]=json!(match edge["from"].as_str().unwrap() {
                    "window_cool_target.y"=>"cool_setpoint",
                    "cool_pi.y"=>"cooling_loop",
                    other=>other,
                });
                copy
            }).collect::<Vec<_>>()
    });
    branch["connections"].as_array_mut().unwrap().extend([
        json!({"from":"sat_suppression_complete.y","to":"after"}),
        json!({"from":"sat_suppression_duration.y","to":"duration"}),
    ]);
    let branch: Block = serde_json::from_value(branch).unwrap();
    assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
    let mut actual_after = Vec::new();
    let mut actual_duration = Vec::new();
    let steps = ((7200.0 - start) * 1000.0 / step_millis as f64).round() as usize;
    let sample_every = steps / 100;
    assert_eq!(sample_every * 100, steps);
    let dt = step_millis as f64 / 1000.0;
    for second in 0..=steps {
        let time = start + second as f64 * dt;
        // Source validation model: 22 °C to 27 °C ramp after 900 s,
        // and a 25 °C zone with a 2 K, 7200-second sinusoid.
        let setpoint = 22.0 + (5.0 * (time - 900.0) / 600.0).clamp(0.0, 5.0);
        let zone_temp = 25.0 + 2.0 * (2.0 * std::f64::consts::PI * time / 7200.0).sin();
        let inputs = HashMap::from([
            ("cool_setpoint".into(), json!(setpoint)),
            ("room_temp".into(), json!(zone_temp)),
            ("cooling_loop".into(), json!(0.0)),
        ]);
        let elapsed = if second == 0 { 0.0 } else { dt };
        let ordinary = dynamic.execute(&branch, &inputs, elapsed, "").unwrap();
        let fast = compiled.execute(&inputs, elapsed, false).unwrap().0;
        assert_eq!(ordinary["after"], fast["after"], "second {second}");
        assert_eq!(ordinary["duration"], fast["duration"], "second {second}");
        if second % sample_every == 0 {
            actual_after.push(if ordinary["after"].as_bool().unwrap() {
                1.0
            } else {
                0.0
            });
            actual_duration.push(ordinary["duration"].as_f64().unwrap());
        }
    }
    let mismatches: Vec<_> = expected_after
        .iter()
        .zip(&actual_after)
        .enumerate()
        .filter(|(_, (expected, actual))| (*expected - **actual).abs() > 0.0)
        .map(|(index, _)| index)
        .collect();
    assert!(
        mismatches.is_empty(),
        "OBC Boolean trace differs at samples {mismatches:?}"
    );
    let duration_mismatches: Vec<_> = expected_duration
        .iter()
        .zip(&actual_duration)
        .enumerate()
        .filter(|(_, (expected, actual))| (*expected - **actual).abs() > 1e-6)
        .map(|(index, _)| index)
        .collect();
    assert!(
        duration_mismatches.is_empty(),
        "OBC suppression duration differs at samples {duration_mismatches:?}"
    );
    // The archived OBC trace ends after one ramp. Extend the same branch
    // with two later changes to verify that a change during suppression
    // cannot replace the captured duration, while a later change can.
    let mut repeated = HashMap::new();
    for step in 1..=((1800.0 / dt).round() as usize) {
        let time = 7200.0 + step as f64 * dt;
        let setpoint = if time < 7300.0 {
            27.0
        } else if time < 7480.0 {
            24.0
        } else if time < 8200.0 {
            23.0
        } else {
            26.0
        };
        let inputs = HashMap::from([
            ("cool_setpoint".into(), json!(setpoint)),
            ("room_temp".into(), json!(25.0)),
            ("cooling_loop".into(), json!(0.0)),
        ]);
        let ordinary = dynamic.execute(&branch, &inputs, dt, "").unwrap();
        let fast = compiled.execute(&inputs, dt, false).unwrap().0;
        assert_eq!(ordinary["after"], fast["after"], "time {time}");
        assert_eq!(ordinary["duration"], fast["duration"], "time {time}");
        for target in [7440, 7680, 8040, 8400, 8940] {
            if (time - target as f64).abs() < dt / 2.0 {
                repeated.insert(
                    target,
                    (
                        ordinary["after"].as_bool().unwrap(),
                        ordinary["duration"].as_f64().unwrap(),
                    ),
                );
            }
        }
    }
    for target in [7440, 7680, 8400] {
        assert_eq!(repeated[&target].0, false, "time {target}");
        assert!((repeated[&target].1 - 540.0).abs() < 1e-6, "time {target}");
    }
    for target in [8040, 8940] {
        assert_eq!(repeated[&target].0, true, "time {target}");
    }
}

pub(super) fn g36_low_airflow_alarm_branch() -> Block {
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
    let is_alarm_block = |name: &str| {
        name.starts_with("low_flow_")
            || name == "flow_requested"
            || name == "flow_alarm_enable_threshold"
            || matches!(
                name,
                "pressure_importance" | "pressure_importance_zero" | "pressure_importance_positive"
            )
    };
    let mut blocks: Vec<Value> = zone["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|block| is_alarm_block(block["name"].as_str().unwrap()))
        .cloned()
        .collect();
    blocks
        .iter_mut()
        .find(|block| block["name"] == "flow_alarm_enable_threshold")
        .unwrap()["parameters"][0]["value"] = json!(0.01);
    for (name, hysteresis) in [
        ("flow_requested", 0.005),
        ("low_flow_detected", 0.01),
        ("low_flow_below_seventy", 0.01),
    ] {
        blocks
            .iter_mut()
            .find(|block| block["name"] == name)
            .unwrap()["parameters"][0]["value"] = json!(hysteresis);
    }
    let connections: Vec<Value> = zone["connections"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|edge| {
            let target = edge["to"].as_str().unwrap();
            if !is_alarm_block(target.split('.').next().unwrap())
                && target != "low_airflow_alarm_level"
            {
                return None;
            }
            let mut edge = edge.clone();
            edge["from"] = match edge["from"].as_str().unwrap() {
                "local_occupied.y" => json!("occupied"),
                "flow_override_select.y" => json!("setpoint"),
                source => json!(source),
            };
            if target == "low_airflow_alarm_level" {
                edge["to"] = json!("alarm");
            }
            Some(edge)
        })
        .collect();
    let branch: Block = serde_json::from_value(json!({
        "name":"OBCReheatLowAirflowAlarm",
        "block_type":"Composite",
        "category":"composite",
        "inputs":[
            {"name":"occupied","type":"Boolean"},
            {"name":"supply_fan_proven","type":"Boolean"},
            {"name":"measured_airflow","type":"Real","unit":"normalized airflow"},
            {"name":"setpoint","type":"Real","unit":"normalized airflow"}
        ],
        "outputs":[{"name":"alarm","type":"Integer"}],
        "blocks":blocks,
        "connections":connections
    }))
    .unwrap();
    assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
    branch
}

mod cases_00;
mod cases_01;
mod cases_02;
mod cases_03;
mod cases_04;
mod cases_05;
mod cases_06;
mod cases_07;
