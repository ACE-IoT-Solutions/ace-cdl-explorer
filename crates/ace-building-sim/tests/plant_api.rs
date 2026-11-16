#![cfg(ace_building_sim_repository_tests)]
use ace_building_sim::{
    BuildingController, Plant, PlantCommands, ReportingContext, Scenario, Simulation, WeatherRecord,
};
fn weather() -> Vec<WeatherRecord> {
    serde_json::from_str(include_str!(
        "../../../web/public/examples/weather_2025.json"
    ))
    .unwrap()
}
#[test]
fn external_controller_composition_matches_closed_loop_rows_exactly() {
    for graph in [
        include_str!("../../../web/public/examples/four_pipe_building.json"),
        include_str!("../../../web/public/examples/four_pipe_g36.json"),
    ] {
        let document = cdl_compat::from_json(graph, Default::default()).unwrap();
        let scenario = Scenario::default();
        let mut closed = Simulation::new(document.clone(), weather(), scenario.clone()).unwrap();
        let mut controller = BuildingController::new(&document).unwrap();
        let mut physical_scenario = scenario;
        physical_scenario.step_seconds = controller
            .aligned_step(physical_scenario.step_seconds)
            .unwrap();
        let mut plant = Plant::new(weather(), physical_scenario).unwrap();
        for _ in 0..(96 * 900 / plant.effective_step_seconds()) {
            let observation = plant.observe().unwrap();
            assert_eq!(observation, plant.observe().unwrap());
            let frame = controller
                .step(&observation.sensors, observation.step_seconds as f64)
                .unwrap();
            let actual = plant
                .step_reported(
                    &frame.commands,
                    &frame.reporting,
                    observation.step_seconds as f64,
                )
                .unwrap();
            let expected = closed.step_chunk(1).unwrap();
            assert_eq!(
                serde_json::to_value(actual.row.as_ref()).unwrap(),
                serde_json::to_value(expected.rows.first()).unwrap()
            );
        }
        assert_eq!(
            serde_json::to_value(plant.summary()).unwrap(),
            serde_json::to_value(closed.summary()).unwrap()
        );
    }
}
#[test]
fn observe_is_idempotent_and_invalid_commands_do_not_advance_physics() {
    let mut plant = Plant::new(weather(), Scenario::default()).unwrap();
    let before = plant.observe().unwrap();
    let mut commands = PlantCommands::default();
    commands.ahu[0].fan_speed = f64::NAN;
    assert!(plant
        .step_reported(
            &commands,
            &ReportingContext::default(),
            before.step_seconds as f64
        )
        .is_err());
    assert_eq!(plant.observe().unwrap(), before);
    assert_eq!(plant.progress(), 0);
    commands.ahu[0].fan_speed = 0.;
    commands.global.plant_heat_pump_requested_mode = 4.;
    assert!(plant
        .step_reported(
            &commands,
            &ReportingContext::default(),
            before.step_seconds as f64
        )
        .is_err());
    commands.global.plant_heat_pump_requested_mode = 0.;
    assert!(plant
        .step_reported(&commands, &ReportingContext::default(), 1.)
        .is_err());
    assert_eq!(plant.observe().unwrap(), before);
    plant
        .step_reported(
            &commands,
            &ReportingContext::default(),
            before.step_seconds as f64,
        )
        .unwrap();
    assert_eq!(plant.progress(), 1);
}
#[test]
fn reporting_does_not_change_physical_response_or_presence() {
    let mut a = Plant::new(weather(), Scenario::default()).unwrap();
    let mut b = Plant::new(weather(), Scenario::default()).unwrap();
    let commands = PlantCommands::default();
    let reporting = ReportingContext {
        preconditioning_mode: true,
        occupancy_confirmed: [true; 10],
    };
    for _ in 0..40 {
        a.step_reported(&commands, &ReportingContext::default(), 900.)
            .unwrap();
        b.step_reported(&commands, &reporting, 900.).unwrap();
        assert_eq!(a.observe().unwrap(), b.observe().unwrap());
    }
    assert_eq!(
        serde_json::to_value(a.summary()).unwrap(),
        serde_json::to_value(b.summary()).unwrap()
    );
}

#[test]
fn native_defaults_match_legacy_scenario_defaults_and_plain_step_needs_no_controller() {
    assert_eq!(
        format!("{:?}", Scenario::default()),
        format!("{:?}", serde_json::from_str::<Scenario>("{}").unwrap())
    );
    let mut plant = Plant::new(weather(), Scenario::default()).unwrap();
    plant.step(&PlantCommands::default(), 900.).unwrap();
    assert_eq!(plant.progress(), 1);
}

#[test]
#[cfg(feature = "checkpoint")]
fn full_building_controller_handover_preserves_every_command_after_json_restore() {
    let document = cdl_compat::from_json(
        include_str!("../../../web/public/examples/four_pipe_g36.json"),
        Default::default(),
    )
    .unwrap();
    let mut controller = BuildingController::new(&document).unwrap();
    let mut scenario = Scenario::default();
    scenario.step_seconds = controller.aligned_step(scenario.step_seconds).unwrap();
    let mut plant = Plant::new(weather(), scenario).unwrap();
    for _ in 0..37 {
        let obs = plant.observe().unwrap();
        let frame = controller.step(&obs.sensors, 15.).unwrap();
        plant
            .step_reported(&frame.commands, &frame.reporting, 15.)
            .unwrap();
    }
    let checkpoint = serde_json::to_string(&controller.state().snapshot().unwrap()).unwrap();
    let mut replacement = BuildingController::new(&document).unwrap();
    replacement
        .state_mut()
        .restore(&serde_json::from_str(&checkpoint).unwrap())
        .unwrap();
    for _ in 0..100 {
        let obs = plant.observe().unwrap();
        let a = controller.step(&obs.sensors, 15.).unwrap();
        let b = replacement.step(&obs.sensors, 15.).unwrap();
        assert_eq!(a.commands, b.commands);
        assert_eq!(a.reporting, b.reporting);
        plant.step_reported(&a.commands, &a.reporting, 15.).unwrap();
    }
}

#[test]
fn one_second_controllers_match_closed_loop_at_the_first_recording_boundary() {
    for graph in [
        include_str!("../../../web/public/examples/four_pipe_building.json"),
        include_str!("../../../web/public/examples/four_pipe_g36.json"),
    ] {
        let document = cdl_compat::from_json(graph, Default::default()).unwrap();
        let scenario = Scenario {
            step_seconds: 1,
            ..Scenario::default()
        };
        let mut closed = Simulation::new(document.clone(), weather(), scenario.clone()).unwrap();
        let mut controller = BuildingController::new(&document).unwrap();
        assert_eq!(controller.aligned_step(1).unwrap(), 1);
        assert_eq!(closed.effective_step_seconds(), 1);
        let mut plant = Plant::new(weather(), scenario).unwrap();
        for second in 1..=900 {
            let observation = plant.observe().unwrap();
            let frame = controller.step(&observation.sensors, 1.).unwrap();
            let actual = plant
                .step_reported(&frame.commands, &frame.reporting, 1.)
                .unwrap();
            let expected = closed.step_chunk(1).unwrap();
            assert_eq!(actual.time_seconds, second as f64);
            assert_eq!(actual.row.is_some(), second == 900);
            assert_eq!(
                serde_json::to_value(actual.row.as_ref()).unwrap(),
                serde_json::to_value(expected.rows.first()).unwrap()
            );
        }
        assert_eq!(
            serde_json::to_value(plant.summary()).unwrap(),
            serde_json::to_value(closed.summary()).unwrap()
        );
    }
}
