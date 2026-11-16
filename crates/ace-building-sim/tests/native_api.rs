//! Published package tests with no repository fixtures or oracle dependencies.
use ace_building_sim::{Plant, PlantCommands, Scenario, Weather, WeatherRecord};
fn weather() -> Vec<WeatherRecord> {
    vec![
        WeatherRecord {
            weather: Weather {
                outdoor_c: 12.,
                solar_kw: 0.,
                internal_kw: 0.,
                occupied: false
            },
            relative_humidity: Some(0.5)
        };
        35_040
    ]
}
#[test]
fn native_plant_steps_without_a_controller_or_repository_data() {
    let mut plant = Plant::new(weather(), Scenario::default()).unwrap();
    let observation = plant.observe().unwrap();
    assert_eq!(observation, plant.observe().unwrap());
    let result = plant.step(&PlantCommands::default(), 900.).unwrap();
    assert_eq!(result.time_seconds, 900.);
    assert!(result.row.is_some());
    assert_eq!(plant.progress(), 1);
}
#[test]
fn invalid_public_command_does_not_advance_the_plant() {
    let mut plant = Plant::new(weather(), Scenario::default()).unwrap();
    let observation = plant.observe().unwrap();
    let mut commands = PlantCommands::default();
    commands.ahu[0].fan_speed = f64::NAN;
    assert!(plant.step(&commands, 900.).is_err());
    assert_eq!(plant.progress(), 0);
    assert_eq!(plant.observe().unwrap(), observation);
}
#[test]
#[cfg(feature = "checkpoint")]
fn checkpoint_feature_restores_a_fresh_native_program() {
    let source = r#"{"name":"gain","block_type":"Gain","inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],"parameters":[{"name":"k","value":2}]}"#;
    let d = cdl_compat::from_json(source, Default::default()).unwrap();
    let registry = cdl_blocks::Registry::new(d.library.clone());
    let p = cdl_exec::Program::compile(&d, &registry).unwrap();
    let mut a = p.instantiate();
    a.set_input("u", cdl_exec::Scalar::Real(3.)).unwrap();
    a.step(1.).unwrap();
    let json = serde_json::to_string(&a.snapshot().unwrap()).unwrap();
    let q = cdl_exec::Program::compile(&d, &registry).unwrap();
    let mut b = q.instantiate();
    b.restore(&serde_json::from_str(&json).unwrap()).unwrap();
    b.step(1.).unwrap();
    assert_eq!(b.output("y").unwrap(), cdl_exec::Scalar::Real(6.));
}

#[test]
fn one_second_steps_preserve_weather_and_recording_boundaries() {
    let mut records = weather();
    records[1].weather.outdoor_c = 25.;
    let scenario = Scenario {
        step_seconds: 1,
        ..Scenario::default()
    };
    let mut plant = Plant::new(records, scenario).unwrap();
    assert_eq!(plant.effective_step_seconds(), 1);
    assert_eq!(plant.total(), 31_536_000);
    let commands = PlantCommands::default();
    for second in 0..1800 {
        let before = plant.observe().unwrap();
        assert_eq!(before.time_seconds, second as f64);
        assert_eq!(before.step_seconds, 1);
        assert_eq!(
            before.sensors.global.outdoor_c,
            if second < 900 { 12. } else { 25. }
        );
        assert!(plant.step(&commands, 0.5).is_err());
        assert_eq!(plant.observe().unwrap(), before);
        let result = plant.step(&commands, 1.).unwrap();
        assert_eq!(result.time_seconds, (second + 1) as f64);
        assert_eq!(result.row.is_some(), (second + 1) % 900 == 0);
        assert!(!result.done);
        if let Some(row) = result.row {
            assert!(row.zone_c.iter().all(|v| v.is_finite()));
            assert_eq!(row.index, second / 900);
        }
    }
    assert_eq!(plant.progress(), 1800);
}

#[test]
fn supported_plant_timesteps_and_invalid_intervals() {
    for step_seconds in [1, 15, 30, 60, 300, 900] {
        let mut plant = Plant::new(
            weather(),
            Scenario {
                step_seconds,
                ..Scenario::default()
            },
        )
        .unwrap();
        assert_eq!(
            plant
                .step(&PlantCommands::default(), step_seconds as f64)
                .unwrap()
                .time_seconds,
            step_seconds as f64
        );
    }
    for step_seconds in [0, 2, 14, 16, 901] {
        assert!(Plant::new(
            weather(),
            Scenario {
                step_seconds,
                ..Scenario::default()
            }
        )
        .is_err());
    }
}
