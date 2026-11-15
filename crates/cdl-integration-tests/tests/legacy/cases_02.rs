use super::*;

#[test]
fn g36_zone_conditioning_modes_are_cdl_decisions_and_drive_setpoints() {
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
    let heat_trace = "zone_01|heat_setpoint_active.y".to_string();
    let cool_trace = "zone_01|cool_setpoint_active.y".to_string();
    for (room, schedule, warmup, cooldown, setback, setup, freeze, presence, mode, heat, cool) in [
        (
            22.0, false, false, false, false, false, false, false, 0.0, -50.0, 80.0,
        ),
        (
            15.0, false, false, false, true, false, false, false, 6.0, 18.0, 80.0,
        ),
        (
            33.0, false, false, false, false, true, false, false, 5.0, -50.0, 32.0,
        ),
        (
            3.0, false, false, false, false, false, true, false, 7.0, 18.0, 80.0,
        ),
        (
            19.0, false, true, false, false, false, false, false, 3.0, 21.0, 24.0,
        ),
        (
            27.0, false, false, true, false, false, false, false, 4.0, 21.0, 24.0,
        ),
        (
            22.0, true, false, false, false, false, false, false, 2.0, 20.5, 24.5,
        ),
        (
            22.0, true, false, false, false, false, false, true, 1.0, 21.0, 24.0,
        ),
    ] {
        let inputs = HashMap::from([
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(room)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_air_setpoint".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(schedule)),
            ("group_warmup".into(), json!(warmup)),
            ("group_cooldown".into(), json!(cooldown)),
            ("group_setback".into(), json!(setback)),
            ("group_setup".into(), json!(setup)),
            ("group_freeze_setback".into(), json!(freeze)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("supply_fan_proven".into(), json!(false)),
            ("actual_fan_speed".into(), json!(0.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("zone_presence_raw".into(), json!(presence)),
            (
                "occupancy_count".into(),
                json!(if presence { 8.0 } else { 0.0 }),
            ),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
            ("measured_airflow".into(), json!(0.0)),
            ("discharge_temp".into(), json!(16.0)),
        ]);
        let mut dynamic = Executor::default();
        dynamic.trace_targets = vec![heat_trace.clone(), cool_trace.clone()];
        let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
        let mut actual = HashMap::new();
        for _ in 0..7 {
            actual = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                actual["conditioning_state"].as_f64(),
                fast["conditioning_state"].as_f64()
            );
        }
        assert_eq!(
            actual["conditioning_state"].as_f64(),
            Some(mode),
            "room={room}, schedule={schedule}, warmup={warmup}, cooldown={cooldown}"
        );
        assert_eq!(dynamic.trace_values[&heat_trace].as_f64(), Some(heat));
        assert_eq!(dynamic.trace_values[&cool_trace].as_f64(), Some(cool));
    }
    let mut tuned = zone.clone();
    tuned
        .blocks
        .iter_mut()
        .find(|block| block.name == "heat_setpoint")
        .unwrap()
        .parameters[0]
        .value = json!(23.0);
    let inputs = HashMap::from([
        ("flow_override_code".into(), json!(0)),
        ("room_temp".into(), json!(22.0)),
        ("co2_ppm".into(), json!(420.0)),
        ("supply_air_temp".into(), json!(16.0)),
        ("supply_air_setpoint".into(), json!(16.0)),
        ("supply_pressure".into(), json!(1.0)),
        ("scheduled_occupied".into(), json!(false)),
        ("group_warmup".into(), json!(true)),
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
        ("zone_presence_raw".into(), json!(false)),
        ("occupancy_count".into(), json!(0.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("measured_airflow".into(), json!(0.0)),
        ("discharge_temp".into(), json!(16.0)),
    ]);
    let standard = Executor::default()
        .execute(zone, &inputs, 60.0, "")
        .unwrap();
    let changed = Executor::default()
        .execute(&tuned, &inputs, 60.0, "")
        .unwrap();
    assert_eq!(standard["conditioning_state"].as_f64(), Some(0.0));
    assert_eq!(changed["conditioning_state"].as_f64(), Some(3.0));
}

#[test]
fn g36_zone_group_optimal_start_is_an_editable_cdl_decision() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let mut inputs = HashMap::from([
        ("seconds_until_occupied".into(), json!(3600.0)),
        ("lookahead_allowed".into(), json!(true)),
        ("scheduled_occupied".into(), json!(false)),
    ]);
    for zone in 1..=5 {
        inputs.insert(format!("room_{zone}"), json!(19.0));
        inputs.insert(format!("heat_target_{zone}"), json!(21.0));
        inputs.insert(format!("cool_target_{zone}"), json!(24.0));
        inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
        inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
        inputs.insert(format!("window_closed_{zone}"), json!(true));
        inputs.insert(format!("override_{zone}"), json!(false));
    }
    let check = |block: &Block, values: &HashMap<String, Value>| {
        let dynamic = Executor::default()
            .execute(block, values, 60.0, "")
            .unwrap();
        let compiled = CompiledExecutor::try_new(block)
            .unwrap()
            .unwrap()
            .execute(values, 60.0, false)
            .unwrap()
            .0;
        for name in ["warmup", "cooldown", "preconditioning"] {
            assert_eq!(dynamic[name].as_bool(), compiled[name].as_bool());
        }
        dynamic
    };
    let warm = check(group, &inputs);
    assert_eq!(warm["warmup"], true);
    assert_eq!(warm["cooldown"], false);
    let mut status_tuned = group.clone();
    for zone in 1..=5 {
        let status = status_tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == format!("zone_status_{zone}"))
            .unwrap();
        let hysteresis = status
            .blocks
            .iter_mut()
            .find(|block| block.name == "hys")
            .unwrap();
        hysteresis
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "uHigh")
            .unwrap()
            .value = json!(50.0);
    }
    assert_eq!(check(&status_tuned, &inputs)["warmup"], false);
    inputs.insert("seconds_until_occupied".into(), json!(7200.0));
    assert_eq!(check(group, &inputs)["preconditioning"], false);
    inputs.insert("seconds_until_occupied".into(), json!(3600.0));
    for zone in 1..=5 {
        inputs.insert(format!("room_{zone}"), json!(29.0));
    }
    let cool = check(group, &inputs);
    assert_eq!(cool["cooldown"], true);
    assert_eq!(cool["warmup"], false);
    inputs.insert("seconds_until_occupied".into(), json!(10801.0));
    assert_eq!(check(group, &inputs)["preconditioning"], false);
    inputs.insert("lookahead_allowed".into(), json!(false));
    inputs.insert("seconds_until_occupied".into(), json!(3600.0));
    assert_eq!(check(group, &inputs)["preconditioning"], false);
    let mut tuned = group.clone();
    let mode = tuned
        .blocks
        .iter_mut()
        .find(|block| block.name == "obc_operation_mode")
        .unwrap();
    mode.blocks
        .iter_mut()
        .find(|block| block.name == "maxWarCooTime")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "k")
        .unwrap()
        .value = json!(600.0);
    inputs.insert("lookahead_allowed".into(), json!(true));
    assert_eq!(check(&tuned, &inputs)["preconditioning"], false);
}

#[test]
fn g36_optimal_start_learns_from_measured_zone_recovery_in_both_executors() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let mut group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap()
        .clone();
    // Advance a configurable daily clock after the 30-minute recovery in this short trace.
    group
        .blocks
        .iter_mut()
        .find(|block| block.name == "optimal_daily_sample")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "period")
        .unwrap()
        .value = json!(2100.0);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&group).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("seconds_until_occupied".into(), json!(3500.0)),
        ("lookahead_allowed".into(), json!(true)),
        ("scheduled_occupied".into(), json!(false)),
    ]);
    for zone in 1..=5 {
        inputs.insert(format!("room_{zone}"), json!(19.0));
        inputs.insert(format!("heat_target_{zone}"), json!(21.0));
        inputs.insert(format!("cool_target_{zone}"), json!(24.0));
        inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
        inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
        inputs.insert(format!("window_closed_{zone}"), json!(true));
        inputs.insert(format!("override_{zone}"), json!(false));
    }
    let mut step = |inputs: &HashMap<String, Value>| {
        let reference = dynamic.execute(&group, inputs, 60.0, "").unwrap();
        let optimized = compiled.execute(inputs, 60.0, false).unwrap().0;
        assert_eq!(reference["warmup"].as_bool(), optimized["warmup"].as_bool());
        reference["warmup"].as_bool().unwrap()
    };
    assert!(step(&inputs));
    for minute in 1..=36 {
        if minute == 10 {
            inputs.insert("scheduled_occupied".into(), json!(true));
        }
        inputs.insert(
            "seconds_until_occupied".into(),
            json!(if minute < 10 {
                3500.0 - minute as f64 * 60.0
            } else {
                86400.0
            }),
        );
        for zone in 1..=5 {
            inputs.insert(
                format!("room_{zone}"),
                json!(19.0 + 2.0 * (minute.min(30)) as f64 / 30.0),
            );
        }
        step(&inputs);
    }
    inputs.insert("seconds_until_occupied".into(), json!(86400.0));
    inputs.insert("lookahead_allowed".into(), json!(false));
    inputs.insert("scheduled_occupied".into(), json!(false));
    assert!(!step(&inputs));
    inputs.insert("seconds_until_occupied".into(), json!(3500.0));
    inputs.insert("lookahead_allowed".into(), json!(true));
    for zone in 1..=5 {
        inputs.insert(format!("room_{zone}"), json!(19.0));
    }
    assert!(
        !step(&inputs),
        "the measured faster recovery must shorten the next day's start"
    );
}

#[test]
fn g36_recovery_observation_requires_an_optimal_start_episode() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let mut group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap()
        .clone();
    group
        .blocks
        .iter_mut()
        .find(|block| block.name == "optimal_warmup_horizon_hold")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "trueHoldDuration")
        .unwrap()
        .value = json!(180.0);
    let trace = "zone_group_1|optimal_warmup_active.y".to_string();
    let mut dynamic = Executor::default();
    dynamic.trace_targets = vec![trace.clone()];
    let mut compiled = CompiledExecutor::try_new(&group).unwrap().unwrap();
    compiled.set_traces(std::slice::from_ref(&trace)).unwrap();
    let mut inputs = HashMap::from([
        ("seconds_until_occupied".into(), json!(3500.0)),
        ("lookahead_allowed".into(), json!(true)),
        ("scheduled_occupied".into(), json!(false)),
    ]);
    for zone in 1..=5 {
        inputs.insert(format!("room_{zone}"), json!(19.0));
        inputs.insert(format!("heat_target_{zone}"), json!(21.0));
        inputs.insert(format!("cool_target_{zone}"), json!(24.0));
        inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
        inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
        inputs.insert(format!("window_closed_{zone}"), json!(true));
        inputs.insert(format!("override_{zone}"), json!(false));
    }
    let mut ordinary_inputs = inputs.clone();
    ordinary_inputs.insert("scheduled_occupied".into(), json!(true));
    ordinary_inputs.insert("seconds_until_occupied".into(), json!(86400.0));
    ordinary_inputs.insert("lookahead_allowed".into(), json!(false));
    for zone in 1..=5 {
        ordinary_inputs.insert(format!("room_{zone}"), json!(22.0));
    }
    let mut ordinary = Executor::default();
    ordinary.trace_targets = vec![trace.clone()];
    ordinary
        .execute(&group, &ordinary_inputs, 60.0, "")
        .unwrap();
    assert_eq!(
        ordinary.trace_values[&trace],
        json!(false),
        "ordinary occupied operation must not teach recovery"
    );
    let mut observed = |inputs: &HashMap<String, Value>| {
        dynamic.execute(&group, inputs, 60.0, "").unwrap();
        let (_, traces) = compiled.execute(inputs, 60.0, true).unwrap();
        let reference = dynamic.trace_values[&trace].as_bool().unwrap();
        assert_eq!(reference, traces[&trace].as_f64().unwrap() != 0.0);
        reference
    };
    assert!(observed(&inputs));
    inputs.insert("scheduled_occupied".into(), json!(true));
    inputs.insert("seconds_until_occupied".into(), json!(86400.0));
    for zone in 1..=5 {
        inputs.insert(format!("room_{zone}"), json!(21.0));
    }
    assert!(
        observed(&inputs),
        "the episode must continue into occupancy"
    );
    inputs.insert("scheduled_occupied".into(), json!(false));
    inputs.insert("lookahead_allowed".into(), json!(false));
    assert!(
        observed(&inputs),
        "the episode must survive the input transition until the hold expires"
    );
    assert!(
        !observed(&inputs),
        "the episode must clear when the hold expires"
    );
}

#[test]
fn g36_optimal_start_uses_longest_closed_window_zone_prediction() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let mut group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap()
        .clone();
    group
        .blocks
        .iter_mut()
        .find(|block| block.name == "optimal_daily_sample")
        .unwrap()
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == "period")
        .unwrap()
        .value = json!(3900.0);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&group).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("seconds_until_occupied".into(), json!(3500.0)),
        ("lookahead_allowed".into(), json!(true)),
        ("scheduled_occupied".into(), json!(false)),
    ]);
    for zone in 1..=5 {
        inputs.insert(
            format!("room_{zone}"),
            json!(if zone <= 2 { 19.0 } else { 21.0 }),
        );
        inputs.insert(format!("heat_target_{zone}"), json!(21.0));
        inputs.insert(format!("cool_target_{zone}"), json!(24.0));
        inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
        inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
        inputs.insert(format!("window_closed_{zone}"), json!(true));
        inputs.insert(format!("override_{zone}"), json!(false));
    }
    let mut step = |inputs: &HashMap<String, Value>| {
        let reference = dynamic.execute(&group, inputs, 60.0, "").unwrap();
        let optimized = compiled.execute(inputs, 60.0, false).unwrap().0;
        assert_eq!(reference["warmup"].as_bool(), optimized["warmup"].as_bool());
        reference["warmup"].as_bool().unwrap()
    };
    assert!(step(&inputs));
    for minute in 1..=60 {
        inputs.insert(
            "seconds_until_occupied".into(),
            json!(100.0_f64.max(3500.0 - minute as f64 * 60.0)),
        );
        inputs.insert(
            "room_1".into(),
            json!(19.0 + 2.0 * minute.min(30) as f64 / 30.0),
        );
        inputs.insert("room_2".into(), json!(19.0 + 2.0 * minute as f64 / 60.0));
        step(&inputs);
    }
    for _ in 0..5 {
        step(&inputs);
    }
    inputs.insert("seconds_until_occupied".into(), json!(86400.0));
    inputs.insert("lookahead_allowed".into(), json!(false));
    assert!(!step(&inputs));
    inputs.insert("seconds_until_occupied".into(), json!(3500.0));
    inputs.insert("lookahead_allowed".into(), json!(true));
    inputs.insert("room_1".into(), json!(21.0));
    inputs.insert("room_2".into(), json!(19.0));
    assert!(
        step(&inputs),
        "the slower zone should set the group start time"
    );
    inputs.insert("window_closed_2".into(), json!(false));
    assert!(
        !step(&inputs),
        "an open window should remove the slow zone's request"
    );
    inputs.insert("window_closed_2".into(), json!(true));
    assert!(
        step(&inputs),
        "closing the window should resample that zone's need"
    );
}

#[test]
fn g36_recovery_learner_records_one_rate_per_active_episode() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let learner = group
        .blocks
        .iter()
        .find(|block| block.name == "optimal_warmup_1_learner")
        .unwrap();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(learner).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("error".into(), json!(2.0)),
        ("active".into(), json!(true)),
        ("window_closed".into(), json!(true)),
        ("fallback_rate".into(), json!(1.0 / 3600.0)),
        ("midnight".into(), json!(false)),
        ("sampled_need".into(), json!(2.0)),
    ]);
    let mut step = |values: &HashMap<String, Value>| {
        let reference = dynamic.execute(learner, values, 60.0, "").unwrap()["rate"]
            .as_f64()
            .unwrap();
        let optimized = compiled.execute(values, 60.0, false).unwrap().0["rate"]
            .as_f64()
            .unwrap();
        assert!((reference - optimized).abs() < 1e-12);
        reference
    };
    step(&inputs);
    for minute in 1..=30 {
        inputs.insert("error".into(), json!(2.0 * (1.0 - minute as f64 / 30.0)));
        step(&inputs);
    }
    assert_eq!(step(&inputs), 1.0 / 3600.0);
    inputs.insert("midnight".into(), json!(true));
    let learned = step(&inputs);
    inputs.insert("midnight".into(), json!(false));
    assert!(learned > 1.0 / 3600.0);
    inputs.insert("error".into(), json!(0.2));
    assert_eq!(step(&inputs), learned);
    inputs.insert("error".into(), json!(0.0));
    assert_eq!(
        step(&inputs),
        learned,
        "re-crossing the setpoint must not add a second episode sample"
    );
    inputs.insert("midnight".into(), json!(true));
    inputs.insert("sampled_need".into(), json!(0.0));
    assert_eq!(
        step(&inputs),
        learned,
        "no sampled need must suppress the next daily sample"
    );
    inputs.insert("midnight".into(), json!(false));
    step(&inputs);
    inputs.insert("sampled_need".into(), json!(2.0));
    inputs.insert("midnight".into(), json!(true));
    assert!(
        step(&inputs) > learned,
        "OBC's positive sampled need can reuse the latest held recovery rate"
    );
}

#[test]
fn g36_daily_recovery_sample_uses_start_need_and_window_state() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let learner = group
        .blocks
        .iter()
        .find(|block| block.name == "optimal_warmup_1_learner")
        .unwrap();
    let trace = format!("{}|daily_sample.y", learner.name);
    let mut dynamic = Executor::default();
    dynamic.trace_targets = vec![trace.clone()];
    let mut compiled = CompiledExecutor::try_new(learner).unwrap().unwrap();
    compiled.set_traces(std::slice::from_ref(&trace)).unwrap();
    let mut inputs = HashMap::from([
        ("error".into(), json!(2.0)),
        ("active".into(), json!(false)),
        ("window_closed".into(), json!(true)),
        ("fallback_rate".into(), json!(1.0 / 3600.0)),
        ("midnight".into(), json!(false)),
        ("sampled_need".into(), json!(0.0)),
    ]);
    for (midnight, need, window_closed, expected) in [
        (false, 0.0, true, false),
        (true, 0.0, true, false),
        (false, 2.0, false, false),
        (true, 2.0, false, false),
        (false, 2.0, true, false),
        (true, 2.0, true, true),
    ] {
        inputs.insert("midnight".into(), json!(midnight));
        inputs.insert("sampled_need".into(), json!(need));
        inputs.insert("window_closed".into(), json!(window_closed));
        dynamic.execute(learner, &inputs, 60.0, "").unwrap();
        let (_, optimized) = compiled.execute(&inputs, 60.0, true).unwrap();
        assert_eq!(dynamic.trace_values[&trace].as_bool(), Some(expected));
        assert_eq!(
            optimized[&trace].as_f64(),
            Some(if expected { 1.0 } else { 0.0 })
        );
    }
}

#[test]
fn g36_recovery_slope_keeps_the_obc_minimum_in_both_executors() {
    // OBC OptimalStartCalculation uses Max(sampled slope, 1/3600 K/s).
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let learner = group
        .blocks
        .iter()
        .find(|block| block.name == "optimal_warmup_1_learner")
        .unwrap();
    let fallback = 1.0 / 3600.0;
    let mut dynamic = Executor::default();
    let raw_rate_trace = format!("{}|rate_mean.y", learner.name);
    dynamic.trace_targets = vec![raw_rate_trace.clone()];
    let mut compiled = CompiledExecutor::try_new(learner).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("error".into(), json!(2.0)),
        ("active".into(), json!(true)),
        ("window_closed".into(), json!(true)),
        ("fallback_rate".into(), json!(fallback)),
        ("midnight".into(), json!(false)),
        ("sampled_need".into(), json!(2.0)),
    ]);
    for minute in 0..=180 {
        inputs.insert("error".into(), json!(2.0 * (1.0 - minute as f64 / 180.0)));
        let actual = dynamic.execute(learner, &inputs, 60.0, "").unwrap()["rate"]
            .as_f64()
            .unwrap();
        let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0["rate"]
            .as_f64()
            .unwrap();
        assert!((actual - optimized).abs() < 1e-12);
        assert!(
            (actual - fallback).abs() < 1e-12,
            "a slower recovery cannot lower the OBC minimum slope"
        );
    }
    inputs.insert("midnight".into(), json!(true));
    let actual = dynamic.execute(learner, &inputs, 60.0, "").unwrap()["rate"]
        .as_f64()
        .unwrap();
    let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0["rate"]
        .as_f64()
        .unwrap();
    assert!((actual - optimized).abs() < 1e-12);
    assert!((actual - fallback).abs() < 1e-12);
    assert!(
        dynamic.trace_values[&raw_rate_trace].as_f64().unwrap() < fallback,
        "the measured mean must actually be slower than the fallback"
    );
}

#[test]
fn g36_recovery_learner_does_not_teach_from_timer_expiry() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let learner = group
        .blocks
        .iter()
        .find(|block| block.name == "optimal_warmup_1_learner")
        .unwrap();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(learner).unwrap().unwrap();
    let mut inputs = HashMap::from([
        ("error".into(), json!(2.0)),
        ("active".into(), json!(true)),
        ("window_closed".into(), json!(true)),
        ("fallback_rate".into(), json!(1.0 / 36000.0)),
        ("midnight".into(), json!(false)),
        ("sampled_need".into(), json!(2.0)),
    ]);
    for minute in 0..=185 {
        let reference = dynamic.execute(learner, &inputs, 60.0, "").unwrap()["rate"]
            .as_f64()
            .unwrap();
        let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0["rate"]
            .as_f64()
            .unwrap();
        assert!((reference - optimized).abs() < 1e-12);
        assert!(
            (reference - 1.0 / 36000.0).abs() < 1e-12,
            "minute {minute}: timer expiry without setpoint attainment must not teach"
        );
    }
    inputs.insert("error".into(), json!(0.0));
    let reference = dynamic.execute(learner, &inputs, 60.0, "").unwrap()["rate"]
        .as_f64()
        .unwrap();
    let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0["rate"]
        .as_f64()
        .unwrap();
    assert!((reference - optimized).abs() < 1e-12);
    assert!(
        (reference - 1.0 / 36000.0).abs() < 1e-12,
        "attainment waits for the daily sample"
    );
    inputs.insert("midnight".into(), json!(true));
    let reference = dynamic.execute(learner, &inputs, 60.0, "").unwrap()["rate"]
        .as_f64()
        .unwrap();
    let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0["rate"]
        .as_f64()
        .unwrap();
    assert!((reference - optimized).abs() < 1e-12);
    assert!(
        reference > 1.0 / 36000.0,
        "actual attainment within fourteen hours should teach on the daily event"
    );
}

#[test]
fn g36_group_setback_setup_and_freeze_latch_in_cdl() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let mut inputs = HashMap::from([
        ("seconds_until_occupied".into(), json!(86400.0)),
        ("lookahead_allowed".into(), json!(false)),
        ("scheduled_occupied".into(), json!(false)),
    ]);
    for zone in 1..=5 {
        inputs.insert(format!("room_{zone}"), json!(19.0));
        inputs.insert(format!("heat_target_{zone}"), json!(21.0));
        inputs.insert(format!("cool_target_{zone}"), json!(24.0));
        inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
        inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
        inputs.insert(format!("window_closed_{zone}"), json!(true));
        inputs.insert(format!("override_{zone}"), json!(false));
    }
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
    for (room, setback, setup, freeze, alarm, mode) in [
        (19.0, false, false, false, 0.0, 7),
        (17.0, true, false, false, 0.0, 5),
        (18.5, true, false, false, 0.0, 5),
        (19.2, false, false, false, 0.0, 7),
        (33.0, false, true, false, 0.0, 3),
        (31.5, false, true, false, 0.0, 3),
        (30.8, false, false, false, 0.0, 7),
        (3.0, false, false, true, 3.0, 6),
        (6.0, false, false, true, 3.0, 6),
        (7.2, true, false, false, 0.0, 5),
    ] {
        for zone in 1..=5 {
            inputs.insert(format!("room_{zone}"), json!(room));
        }
        let result = dynamic.execute(group, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for (name, expected) in [
            ("setback", setback),
            ("setup", setup),
            ("freeze_setback", freeze),
        ] {
            assert_eq!(
                result[name],
                json!(expected),
                "{name} at {room} °C; OBC mode {}",
                result["operation_mode"]
            );
            assert_eq!(fast[name].as_bool(), Some(expected));
        }
        assert_eq!(result["freeze_alarm_level"].as_f64(), Some(alarm));
        assert_eq!(fast["freeze_alarm_level"].as_f64(), Some(alarm));
        assert_eq!(result["operation_mode"].as_f64(), Some(mode as f64));
        assert_eq!(fast["operation_mode"].as_f64(), Some(mode as f64));
    }
}

#[test]
fn g36_mode_priority_resolves_simultaneous_recovery_requests() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let mut inputs = HashMap::from([
        ("seconds_until_occupied".into(), json!(3500.0)),
        ("lookahead_allowed".into(), json!(true)),
        ("scheduled_occupied".into(), json!(false)),
    ]);
    for zone in 1..=5 {
        inputs.insert(
            format!("room_{zone}"),
            json!(if zone == 1 {
                15.0
            } else if zone == 2 {
                29.0
            } else {
                22.0
            }),
        );
        inputs.insert(format!("heat_target_{zone}"), json!(21.0));
        inputs.insert(format!("cool_target_{zone}"), json!(24.0));
        inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
        inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
        inputs.insert(format!("window_closed_{zone}"), json!(true));
        inputs.insert(format!("override_{zone}"), json!(false));
    }
    let raw_trace = "zone_group_1|obc_operation_mode.yOpeMod".to_string();
    let mut dynamic = Executor::default();
    dynamic.trace_targets = vec![raw_trace.clone()];
    let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
    compiled
        .set_traces(std::slice::from_ref(&raw_trace))
        .unwrap();
    let result = dynamic.execute(group, &inputs, 60.0, "").unwrap();
    let (fast, traces) = compiled.execute(&inputs, 60.0, true).unwrap();
    assert_eq!(
        dynamic.trace_values[&raw_trace].as_f64(),
        Some(6.0),
        "the pinned OBC block adds simultaneous cool-down and warm-up codes"
    );
    assert_eq!(traces[&raw_trace].as_f64(), Some(6.0));
    for output in [&result, &fast] {
        assert_eq!(
            output["operation_mode"].as_f64(),
            Some(2.0),
            "the editable priority tree chooses one recovery mode"
        );
        assert_eq!(output["cooldown"], json!(true));
        assert_eq!(output["warmup"], json!(false));
    }
}

#[test]
fn g36_window_contact_changes_group_and_terminal_cdl_decisions() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let mut group_inputs = HashMap::from([
        ("seconds_until_occupied".into(), json!(3600.0)),
        ("lookahead_allowed".into(), json!(true)),
        ("scheduled_occupied".into(), json!(false)),
    ]);
    for zone in 1..=5 {
        group_inputs.insert(
            format!("room_{zone}"),
            json!(if zone == 1 { 15.0 } else { 21.0 }),
        );
        group_inputs.insert(format!("heat_target_{zone}"), json!(21.0));
        group_inputs.insert(format!("cool_target_{zone}"), json!(24.0));
        group_inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
        group_inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
        group_inputs.insert(format!("window_closed_{zone}"), json!(true));
        group_inputs.insert(format!("override_{zone}"), json!(false));
    }
    assert_eq!(
        Executor::default()
            .execute(group, &group_inputs, 60.0, "")
            .unwrap()["warmup"],
        true
    );
    group_inputs.insert("window_closed_1".into(), json!(false));
    let opened = Executor::default()
        .execute(group, &group_inputs, 60.0, "")
        .unwrap();
    assert_eq!(opened["warmup"], false);
    let compiled = CompiledExecutor::try_new(group)
        .unwrap()
        .unwrap()
        .execute(&group_inputs, 60.0, false)
        .unwrap()
        .0;
    assert_eq!(compiled["warmup"], false);
    group_inputs.insert("lookahead_allowed".into(), json!(false));
    group_inputs.insert("room_1".into(), json!(5.0));
    assert_eq!(
        Executor::default()
            .execute(group, &group_inputs, 60.0, "")
            .unwrap()["setback"],
        false
    );
    group_inputs.insert("window_closed_1".into(), json!(true));
    assert_eq!(
        Executor::default()
            .execute(group, &group_inputs, 60.0, "")
            .unwrap()["setback"],
        true
    );
    group_inputs.insert("window_closed_1".into(), json!(false));
    group_inputs.insert("room_1".into(), json!(3.0));
    assert_eq!(
        Executor::default()
            .execute(group, &group_inputs, 60.0, "")
            .unwrap()["freeze_setback"],
        true
    );

    let zone = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let mut inputs = HashMap::from([
        ("room_temp".into(), json!(29.0)),
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
        ("zone_presence_raw".into(), json!(true)),
        ("occupancy_count".into(), json!(8.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("measured_airflow".into(), json!(0.0)),
        ("discharge_temp".into(), json!(16.0)),
        ("window_closed".into(), json!(true)),
        ("local_override".into(), json!(false)),
        ("supply_fan_proven".into(), json!(false)),
        ("actual_fan_speed".into(), json!(0.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
    ]);
    let trace = "zone_01|window_primary_minimum.y".to_string();
    let mut dynamic = Executor::default();
    dynamic.trace_targets = vec![trace.clone()];
    let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
    compiled.set_traces(std::slice::from_ref(&trace)).unwrap();
    for _ in 0..5 {
        dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        compiled.execute(&inputs, 60.0, true).unwrap();
    }
    let closed = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
    let closed_minimum = dynamic.trace_values[&trace].as_f64().unwrap();
    let (_, closed_traces) = compiled.execute(&inputs, 60.0, true).unwrap();
    assert!(closed_minimum > 0.0);
    assert_eq!(closed_traces[&trace].as_f64(), Some(closed_minimum));
    inputs.insert("window_closed".into(), json!(false));
    let open = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
    let (_, open_traces) = compiled.execute(&inputs, 60.0, true).unwrap();
    assert!(closed["cooling_demand"].as_f64().unwrap() > 0.0);
    assert_eq!(open["cooling_demand"], 0.0);
    assert_eq!(dynamic.trace_values[&trace], json!(0.0));
    assert_eq!(open_traces[&trace].as_f64(), Some(0.0));
    inputs.insert("room_temp".into(), json!(3.0));
    inputs.insert("group_freeze_setback".into(), json!(true));
    inputs.insert("scheduled_occupied".into(), json!(false));
    let freeze = Executor::default()
        .execute(zone, &inputs, 60.0, "")
        .unwrap();
    assert!(freeze["heating_demand"].as_f64().unwrap() > 0.0);
    assert!(freeze["airflow_request"].as_f64().unwrap() > 0.0);

    inputs.insert("room_temp".into(), json!(22.0));
    inputs.insert("group_freeze_setback".into(), json!(false));
    inputs.insert("scheduled_occupied".into(), json!(true));
    inputs.insert("zone_presence_raw".into(), json!(false));
    inputs.insert("occupancy_count".into(), json!(0.0));
    inputs.insert("window_closed".into(), json!(true));
    let mut no_standby = zone.clone();
    no_standby
        .blocks
        .iter_mut()
        .find(|block| block.name == "ventilation_standby_permitted")
        .unwrap()
        .parameters[0]
        .value = json!(false);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(&no_standby).unwrap().unwrap();
    let standby_trace = "zone_01|window_primary_minimum.y".to_string();
    let heat_trace = "zone_01|heat_setpoint_active.y".to_string();
    let cool_trace = "zone_01|cool_setpoint_active.y".to_string();
    let traces = vec![
        standby_trace.clone(),
        heat_trace.clone(),
        cool_trace.clone(),
    ];
    dynamic.trace_targets = traces.clone();
    compiled.set_traces(&traces).unwrap();
    for _ in 0..3 {
        dynamic.execute(&no_standby, &inputs, 60.0, "").unwrap();
        compiled.execute(&inputs, 60.0, true).unwrap();
    }
    let reference = dynamic.execute(&no_standby, &inputs, 60.0, "").unwrap();
    let (fast, fast_traces) = compiled.execute(&inputs, 60.0, true).unwrap();
    let mut default_dynamic = Executor::default();
    default_dynamic.trace_targets = vec![heat_trace.clone(), cool_trace.clone()];
    let default_standby = default_dynamic.execute(zone, &inputs, 60.0, "").unwrap();
    let mut default_compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
    default_compiled
        .set_traces(&[heat_trace.clone(), cool_trace.clone()])
        .unwrap();
    let (default_fast, default_traces) = default_compiled.execute(&inputs, 60.0, true).unwrap();
    for name in [
        "conditioning_state",
        "airflow_request",
        "damper_position",
        "ventilation_area",
        "ventilation_population",
    ] {
        assert_eq!(default_fast[name], default_standby[name]);
    }
    for trace in [&heat_trace, &cool_trace] {
        assert_eq!(default_traces[trace], default_dynamic.trace_values[trace]);
    }
    assert_eq!(default_standby["conditioning_state"], json!(2));
    assert_eq!(default_standby["airflow_request"], json!(0.0));
    assert_eq!(default_standby["damper_position"], json!(0.0));
    assert_eq!(default_dynamic.trace_values[&heat_trace], json!(20.5));
    assert_eq!(default_dynamic.trace_values[&cool_trace], json!(24.5));
    assert_eq!(reference["conditioning_state"], json!(1));
    assert!(reference["airflow_request"].as_f64().unwrap() > 0.0);
    assert!(reference["damper_position"].as_f64().unwrap() > 0.0);
    assert_eq!(dynamic.trace_values[&heat_trace], json!(21.0));
    assert_eq!(dynamic.trace_values[&cool_trace], json!(24.0));
    assert_eq!(reference["ventilation_area"], json!(0.03));
    assert_eq!(reference["ventilation_population"], json!(0.0));
    assert!(dynamic.trace_values[&standby_trace].as_f64().unwrap() > 0.0);
    assert_eq!(fast["ventilation_area"], 0.03);
    assert_eq!(fast["ventilation_population"], 0.0);
    assert_eq!(fast["conditioning_state"], 1.0);
    assert_eq!(fast["airflow_request"], reference["airflow_request"]);
    for trace in &traces {
        assert_eq!(fast_traces[trace], dynamic.trace_values[trace]);
    }
}

#[test]
fn g36_local_override_and_window_alarm_are_cdl_decisions() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_group_1")
        .unwrap();
    let mut inputs = HashMap::from([
        ("seconds_until_occupied".into(), json!(3600.0)),
        ("lookahead_allowed".into(), json!(true)),
        ("scheduled_occupied".into(), json!(false)),
    ]);
    for zone in 1..=5 {
        inputs.insert(
            format!("room_{zone}"),
            json!(if zone == 1 { 15.0 } else { 21.0 }),
        );
        inputs.insert(format!("heat_target_{zone}"), json!(21.0));
        inputs.insert(format!("cool_target_{zone}"), json!(24.0));
        inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
        inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
        inputs.insert(format!("window_closed_{zone}"), json!(true));
        inputs.insert(format!("override_{zone}"), json!(false));
    }
    let closed = Executor::default()
        .execute(group, &inputs, 60.0, "")
        .unwrap();
    assert_eq!(closed["warmup"], true);
    assert_eq!(closed["occupied"], false);
    assert_eq!(closed["window_open_alarm_level"], 0.0);
    inputs.insert("override_1".into(), json!(true));
    inputs.insert("window_closed_1".into(), json!(false));
    let overridden = Executor::default()
        .execute(group, &inputs, 60.0, "")
        .unwrap();
    assert_eq!(overridden["occupied"], true);
    assert_eq!(overridden["warmup"], false);
    assert_eq!(overridden["window_open_alarm_level"], 0.0);
    let compiled = CompiledExecutor::try_new(group)
        .unwrap()
        .unwrap()
        .execute(&inputs, 60.0, false)
        .unwrap()
        .0;
    assert_eq!(compiled["occupied"], true);
    assert_eq!(compiled["window_open_alarm_level"], 0.0);
    inputs.insert("override_1".into(), json!(false));
    let alarm = Executor::default()
        .execute(group, &inputs, 60.0, "")
        .unwrap();
    assert_eq!(alarm["occupied"], false);
    assert_eq!(alarm["window_open_alarm_level"], 4.0);

    let zone = graph
        .blocks
        .iter()
        .find(|block| block.name == "zone_01")
        .unwrap();
    let mut zone_inputs = HashMap::from([
        ("room_temp".into(), json!(29.0)),
        ("co2_ppm".into(), json!(420.0)),
        ("supply_air_temp".into(), json!(16.0)),
        ("supply_air_setpoint".into(), json!(16.0)),
        ("supply_pressure".into(), json!(1.0)),
        ("scheduled_occupied".into(), json!(false)),
        ("group_warmup".into(), json!(false)),
        ("group_cooldown".into(), json!(false)),
        ("group_setback".into(), json!(false)),
        ("group_setup".into(), json!(false)),
        ("group_freeze_setback".into(), json!(false)),
        ("zone_presence_raw".into(), json!(false)),
        ("occupancy_count".into(), json!(0.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("measured_airflow".into(), json!(0.0)),
        ("discharge_temp".into(), json!(16.0)),
        ("window_closed".into(), json!(true)),
        ("local_override".into(), json!(false)),
        ("supply_fan_proven".into(), json!(false)),
        ("actual_fan_speed".into(), json!(0.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
    ]);
    let idle = Executor::default()
        .execute(zone, &zone_inputs, 60.0, "")
        .unwrap();
    zone_inputs.insert("local_override".into(), json!(true));
    let active = Executor::default()
        .execute(zone, &zone_inputs, 60.0, "")
        .unwrap();
    assert_eq!(idle["cooling_demand"], 0.0);
    assert!(active["cooling_demand"].as_f64().unwrap() > 0.0);
}

#[test]
fn g36_vav_low_flow_alarm_uses_measured_flow_and_occupied_gate() {
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
        ("room_temp".into(), json!(29.0)),
        ("co2_ppm".into(), json!(420.0)),
        ("supply_air_temp".into(), json!(14.0)),
        ("supply_air_setpoint".into(), json!(14.0)),
        ("supply_pressure".into(), json!(1.0)),
        ("discharge_temp".into(), json!(14.0)),
        ("scheduled_occupied".into(), json!(true)),
        ("group_warmup".into(), json!(false)),
        ("group_cooldown".into(), json!(false)),
        ("group_setback".into(), json!(false)),
        ("group_setup".into(), json!(false)),
        ("group_freeze_setback".into(), json!(false)),
        ("zone_presence_raw".into(), json!(true)),
        ("occupancy_count".into(), json!(8.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("measured_airflow".into(), json!(0.0)),
        ("window_closed".into(), json!(true)),
        ("local_override".into(), json!(false)),
        ("supply_fan_proven".into(), json!(false)),
        ("actual_fan_speed".into(), json!(0.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
    ]);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
    for _ in 0..40 {
        let result = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert!(result["airflow_request"].as_f64().unwrap() > 0.1);
        assert_eq!(result["low_airflow_alarm_level"], json!(0));
        assert_eq!(fast["low_airflow_alarm_level"].as_f64(), Some(0.0));
    }
    inputs.insert("supply_fan_proven".into(), json!(true));
    for step in 0..36 {
        let result = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert!(result["airflow_request"].as_f64().unwrap() > 0.1);
        assert_eq!(
            result["low_airflow_alarm_level"],
            json!(if step == 35 { 2 } else { 0 })
        );
        assert_eq!(
            result["low_airflow_alarm_level"].as_f64().unwrap(),
            fast["low_airflow_alarm_level"].as_f64().unwrap()
        );
    }
    let request = dynamic.execute(zone, &inputs, 60.0, "").unwrap()["airflow_request"]
        .as_f64()
        .unwrap();
    inputs.insert("measured_airflow".into(), json!(request * 0.6));
    for _ in 0..6 {
        let result = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(result["low_airflow_alarm_level"], json!(3));
        assert_eq!(
            result["low_airflow_alarm_level"].as_f64().unwrap(),
            fast["low_airflow_alarm_level"].as_f64().unwrap()
        );
    }
    inputs.insert("supply_fan_proven".into(), json!(false));
    let lost_proof = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
    assert_eq!(lost_proof["low_airflow_alarm_level"], json!(0));
    assert_eq!(
        compiled.execute(&inputs, 60.0, false).unwrap().0["low_airflow_alarm_level"],
        json!(0)
    );
    inputs.insert("supply_fan_proven".into(), json!(true));
    for _ in 0..35 {
        assert_eq!(
            dynamic.execute(zone, &inputs, 60.0, "").unwrap()["low_airflow_alarm_level"],
            json!(0)
        );
    }
    assert_eq!(
        dynamic.execute(zone, &inputs, 60.0, "").unwrap()["low_airflow_alarm_level"],
        json!(3)
    );
    inputs.insert("scheduled_occupied".into(), json!(false));
    inputs.insert("group_setup".into(), json!(true));
    inputs.insert("room_temp".into(), json!(35.0));
    inputs.insert("measured_airflow".into(), json!(0.0));
    let clear = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
    assert_eq!(clear["low_airflow_alarm_level"], json!(0));
    let fresh_setup = Executor::default()
        .execute(zone, &inputs, 60.0, "")
        .unwrap();
    assert!(fresh_setup["airflow_request"].as_f64().unwrap() > 0.1);
    assert_eq!(fresh_setup["low_airflow_alarm_level"], json!(0));
}

#[test]
fn g36_low_flow_alarm_acknowledgement_is_cdl_latched_and_rearms() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1_alarm_acknowledgement")
        .unwrap();
    let mut inputs: HashMap<String, Value> = group
        .inputs
        .iter()
        .map(|port| {
            (
                port.name.clone(),
                if port.kind == "Boolean" {
                    json!(false)
                } else {
                    json!(0)
                },
            )
        })
        .collect();
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
    let mut step = |inputs: &HashMap<String, Value>| {
        let slow = dynamic.execute(group, inputs, 60.0, "").unwrap();
        let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
        assert_eq!(slow["acknowledged_1"], fast["acknowledged_1"]);
        slow["acknowledged_1"].as_bool().unwrap()
    };
    assert!(!step(&inputs));
    inputs.insert("acknowledge_1".into(), json!(true));
    assert!(!step(&inputs));
    inputs.insert("alarm_level_1".into(), json!(2));
    assert!(!step(&inputs)); // A button already held before the fault is not a new press.
    inputs.insert("acknowledge_1".into(), json!(false));
    assert!(!step(&inputs));
    inputs.insert("acknowledge_1".into(), json!(true));
    assert!(step(&inputs));
    inputs.insert("acknowledge_1".into(), json!(false));
    inputs.insert("alarm_level_1".into(), json!(3));
    assert!(step(&inputs)); // A severity change does not discard acknowledgement.
    inputs.insert("alarm_level_1".into(), json!(0));
    assert!(!step(&inputs));
    inputs.insert("alarm_level_1".into(), json!(2));
    assert!(!step(&inputs)); // A new episode must be acknowledged separately.
}

#[test]
fn g36_other_vav_alarm_acknowledgements_are_independent_cdl_state() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1_alarm_acknowledgement")
        .unwrap();
    for kind in [
        "airflow_sensor_alarm",
        "low_discharge_temperature_alarm",
        "leaking_damper_alarm",
        "leaking_reheat_valve_alarm",
        "high_discharge_alarm",
    ] {
        let mut inputs: HashMap<String, Value> = group
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else {
                        json!(0)
                    },
                )
            })
            .collect();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
        let output = format!("{kind}_acknowledged_1");
        let alarm = format!("{kind}_level_1");
        let mut step = |inputs: &HashMap<String, Value>| {
            let slow = dynamic.execute(group, inputs, 60.0, "").unwrap();
            let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
            assert_eq!(slow[&output], fast[&output], "{kind} executor mismatch");
            assert_eq!(
                slow["acknowledged_1"],
                json!(false),
                "inactive low-flow alarm must not be acknowledged by another alarm"
            );
            slow[&output].as_bool().unwrap()
        };
        assert!(!step(&inputs));
        inputs.insert(alarm.clone(), json!(3));
        assert!(!step(&inputs));
        inputs.insert("acknowledge_1".into(), json!(true));
        assert!(step(&inputs), "{kind} should accept an active-alarm press");
        inputs.insert("acknowledge_1".into(), json!(false));
        assert!(step(&inputs));
        inputs.insert(alarm, json!(0));
        assert!(!step(&inputs), "{kind} should clear with its alarm");
    }
}

#[test]
fn g36_ahu_alarm_acknowledgements_are_independent_cdl_state() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "ahu_1_equipment_alarm_acknowledgement")
        .unwrap();
    for alarm in [
        "freeze_ventilation_alarm",
        "freeze_alarm",
        "supply_fan_alarm",
        "fan_speed_tracking_alarm",
        "outdoor_damper_tracking_alarm",
        "return_damper_tracking_alarm",
        "heating_coil_valve_tracking_alarm",
        "cooling_coil_valve_tracking_alarm",
        "group_freeze_alarm",
        "group_window_open_alarm",
    ] {
        let mut inputs: HashMap<String, Value> = group
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else {
                        json!(0)
                    },
                )
            })
            .collect();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
        let level = format!("ahu_1_{alarm}_level");
        let status = format!("ahu_1_{alarm}_acknowledged");
        let mut step = |inputs: &HashMap<String, Value>| {
            let slow = dynamic.execute(group, inputs, 60.0, "").unwrap();
            let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
            assert_eq!(slow[&status], fast[&status], "{alarm} executor mismatch");
            assert_eq!(
                slow["ahu_1_supply_fan_alarm_acknowledged"],
                if alarm == "supply_fan_alarm" {
                    slow[&status].clone()
                } else {
                    json!(false)
                }
            );
            slow[&status].as_bool().unwrap()
        };
        assert!(!step(&inputs));
        inputs.insert(level.clone(), json!(3));
        assert!(!step(&inputs));
        inputs.insert("acknowledge".into(), json!(true));
        assert!(step(&inputs), "{alarm} should accept an active-alarm press");
        inputs.insert("acknowledge".into(), json!(false));
        assert!(step(&inputs));
        inputs.insert(level, json!(0));
        assert!(!step(&inputs), "{alarm} should clear with its alarm");
    }
}

#[test]
fn g36_plant_fault_acknowledgements_are_independent_cdl_state() {
    let graph: Block = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_g36.json"
    )))
    .unwrap();
    let group = graph
        .blocks
        .iter()
        .find(|block| block.name == "plant_alarm_acknowledgement")
        .unwrap();
    let faults = [
        "heat_pump_start_fault",
        "boiler_start_fault",
        "chiller_start_fault",
        "hot_water_pump_start_fault",
        "chilled_water_pump_start_fault",
        "hot_water_flow_fault",
        "chilled_water_flow_fault",
    ];
    for fault in faults {
        let mut inputs: HashMap<String, Value> = group
            .inputs
            .iter()
            .map(|port| (port.name.clone(), json!(false)))
            .collect();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
        let status = format!("{fault}_acknowledged");
        let mut step = |inputs: &HashMap<String, Value>| {
            let slow = dynamic.execute(group, inputs, 60.0, "").unwrap();
            let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
            assert_eq!(slow[&status], fast[&status], "{fault} executor mismatch");
            for other in faults {
                if other != fault {
                    assert_eq!(slow[&format!("{other}_acknowledged")], json!(false));
                }
            }
            slow[&status].as_bool().unwrap()
        };
        assert!(!step(&inputs));
        inputs.insert("acknowledge".into(), json!(true));
        assert!(
            !step(&inputs),
            "press before fault must not acknowledge {fault}"
        );
        inputs.insert(fault.into(), json!(true));
        assert!(
            !step(&inputs),
            "held press must not acknowledge new {fault} fault"
        );
        inputs.insert("acknowledge".into(), json!(false));
        assert!(!step(&inputs));
        inputs.insert("acknowledge".into(), json!(true));
        assert!(step(&inputs));
        inputs.insert("acknowledge".into(), json!(false));
        assert!(step(&inputs));
        inputs.insert(fault.into(), json!(false));
        assert!(!step(&inputs), "{fault} acknowledgment clears on reset");
    }
}

#[test]
fn g36_high_discharge_alarm_requires_continuous_fan_and_occupied_proof() {
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
        ("discharge_temp".into(), json!(60.0)),
        ("scheduled_occupied".into(), json!(true)),
        ("group_warmup".into(), json!(false)),
        ("group_cooldown".into(), json!(false)),
        ("group_setback".into(), json!(false)),
        ("group_setup".into(), json!(false)),
        ("group_freeze_setback".into(), json!(false)),
        ("zone_presence_raw".into(), json!(true)),
        ("occupancy_count".into(), json!(8.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("measured_airflow".into(), json!(0.2)),
        ("window_closed".into(), json!(true)),
        ("local_override".into(), json!(false)),
        ("supply_fan_proven".into(), json!(false)),
        ("actual_fan_speed".into(), json!(0.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
    ]);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
    let mut step = |inputs: &HashMap<String, Value>| {
        let slow = dynamic.execute(zone, inputs, 60.0, "").unwrap();
        let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
        assert_eq!(slow["high_discharge_alarm"], fast["high_discharge_alarm"]);
        assert_eq!(
            slow["high_discharge_alarm_level"],
            fast["high_discharge_alarm_level"]
        );
        assert_eq!(
            slow["high_discharge_alarm_level"],
            json!(if slow["high_discharge_alarm"] == json!(true) {
                3
            } else {
                0
            })
        );
        slow["high_discharge_alarm"].as_bool().unwrap()
    };
    for _ in 0..8 {
        assert!(!step(&inputs));
    }
    inputs.insert("supply_fan_proven".into(), json!(true));
    for _ in 0..5 {
        assert!(!step(&inputs));
    }
    assert!(step(&inputs));
    inputs.insert("supply_fan_proven".into(), json!(false));
    assert!(!step(&inputs));
    inputs.insert("supply_fan_proven".into(), json!(true));
    assert!(!step(&inputs));
    inputs.insert("scheduled_occupied".into(), json!(false));
    assert!(!step(&inputs));
}

#[test]
fn g36_airflow_sensor_alarm_uses_fan_off_proof_and_measured_flow() {
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
        ("discharge_temp".into(), json!(14.0)),
        ("scheduled_occupied".into(), json!(false)),
        ("group_warmup".into(), json!(false)),
        ("group_cooldown".into(), json!(false)),
        ("group_setback".into(), json!(false)),
        ("group_setup".into(), json!(false)),
        ("group_freeze_setback".into(), json!(false)),
        ("zone_presence_raw".into(), json!(false)),
        ("occupancy_count".into(), json!(0.0)),
        ("actual_reheat_valve_position".into(), json!(0.0)),
        ("hot_water_plant_proven".into(), json!(false)),
        ("window_closed".into(), json!(true)),
        ("local_override".into(), json!(false)),
        ("measured_airflow".into(), json!(0.2)),
        ("supply_fan_proven".into(), json!(true)),
        ("actual_fan_speed".into(), json!(1.0)),
        ("actual_outdoor_damper_position".into(), json!(0.2)),
        ("actual_return_damper_position".into(), json!(0.8)),
    ]);
    let mut dynamic = Executor::default();
    let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
    for _ in 0..12 {
        let result = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(result["airflow_sensor_alarm_level"], json!(0));
        assert_eq!(fast["airflow_sensor_alarm_level"], json!(0));
    }
    inputs.insert("supply_fan_proven".into(), json!(false));
    for step in 0..11 {
        let result = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(
            result["airflow_sensor_alarm_level"],
            json!(if step >= 10 { 3 } else { 0 })
        );
        assert_eq!(
            result["airflow_sensor_alarm_level"],
            fast["airflow_sensor_alarm_level"]
        );
    }
    inputs.insert("measured_airflow".into(), json!(0.09));
    assert_eq!(
        dynamic.execute(zone, &inputs, 60.0, "").unwrap()["airflow_sensor_alarm_level"],
        json!(0)
    );
    assert_eq!(
        compiled.execute(&inputs, 60.0, false).unwrap().0["airflow_sensor_alarm_level"],
        json!(0)
    );
    inputs.insert("measured_airflow".into(), json!(0.2));
    inputs.insert("supply_fan_proven".into(), json!(true));
    assert_eq!(
        dynamic.execute(zone, &inputs, 60.0, "").unwrap()["airflow_sensor_alarm_level"],
        json!(0)
    );
    assert_eq!(
        compiled.execute(&inputs, 60.0, false).unwrap().0["airflow_sensor_alarm_level"],
        json!(0)
    );
}
