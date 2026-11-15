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
