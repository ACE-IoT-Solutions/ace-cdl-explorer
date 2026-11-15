use super::*;
use ace_building_sim::Weather;
use arrow_array::Float64Array;
use serde_json::Value;
#[test]
fn arrow_batch_preserves_measurements_and_selected_trace() {
    use arrow_ipc::reader::StreamReader;
    use std::io::Cursor;

    let graph = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/examples/four_pipe_building.json"
    ));
    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 12.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: false
        };
        35_040
    ])
    .unwrap();
    let mut sim = Simulation::new(graph, &weather, r#"{"step_seconds":15}"#).unwrap();
    let trace = "FourPipeBuildingControls.plant|backup_fraction.y";
    sim.set_traces(&serde_json::to_string(&vec![trace]).unwrap())
        .unwrap();
    assert!(sim.step_chunk_arrow(59).unwrap().is_empty());
    let bytes = sim.step_chunk_arrow(1).unwrap();
    let batch = StreamReader::try_new(Cursor::new(bytes), None)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(batch.num_rows(), 1);
    assert_eq!(sim.progress(), 60);
    let outdoor = batch
        .column_by_name("outdoor_c")
        .unwrap()
        .as_any()
        .downcast_ref::<Float64Array>()
        .unwrap();
    assert_eq!(outdoor.value(0), 12.0);
    for name in [
        "zone_01_co2_ppm",
        "zone_01_airflow",
        "zone_01_discharge_c",
        "ahu_1_return_air_c",
        "ahu_2_return_air_c",
        "ahu_1_heating_coil_valve_position",
        "ahu_1_cooling_coil_valve_position",
        "ahu_1_heating_coil_valve_actual_position",
        "ahu_2_heating_coil_valve_actual_position",
        "ahu_1_cooling_coil_valve_actual_position",
        "ahu_2_cooling_coil_valve_actual_position",
        "ahu_1_heating_coil_delivered_kw",
        "ahu_1_cooling_coil_delivered_kw",
        "hot_water_supply_temp_c",
        "hot_water_return_temp_c",
        "hot_water_flow_kg_s",
        "chilled_water_supply_temp_c",
        "chilled_water_return_temp_c",
        "chilled_water_flow_kg_s",
        "water_pump_kw",
        "heat_pump_electric_kw",
        "heat_pump_ambient_exchange_kw",
        "heat_pump_part_load_fraction",
        "heat_pump_cop_multiplier",
        "heat_pump_hot_water_cop_multiplier",
        "heat_pump_chilled_water_cop_multiplier",
    ] {
        let sensor = batch
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap();
        assert!(sensor.value(0).is_finite(), "{name}");
    }
    for name in [
        "schedule_occupied",
        "preconditioning_mode",
        "zone_01_occupied",
        "zone_01_occupant_count_actual",
        "zone_01_occupant_count_sensor",
        "zone_01_occupant_sensible_heat_kw",
    ] {
        let occupancy = batch
            .column_by_name(name)
            .unwrap()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap();
        assert_eq!(occupancy.value(0), 0.0, "{name}");
    }
    let traced = batch
        .column_by_name(&format!("trace:{trace}"))
        .unwrap()
        .as_any()
        .downcast_ref::<Float64Array>()
        .unwrap();
    assert!((traced.value(0) - 0.45).abs() < 1e-12);
}

#[test]
fn arrow_batch_carries_every_scalar_json_measurement() {
    use arrow_ipc::reader::StreamReader;
    use std::io::Cursor;

    let weather = serde_json::to_string(&vec![
        Weather {
            outdoor_c: 12.0,
            solar_kw: 0.0,
            internal_kw: 0.0,
            occupied: true
        };
        35_040
    ])
    .unwrap();
    for graph in [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/four_pipe_building.json"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/examples/four_pipe_g36.json"
        )),
    ] {
        let mut json_sim = Simulation::new(graph, &weather, "{}").unwrap();
        let mut arrow_sim = Simulation::new(graph, &weather, "{}").unwrap();
        let steps = 900 / json_sim.effective_step_seconds();
        let json: Value = serde_json::from_str(&json_sim.step_chunk(steps).unwrap()).unwrap();
        let row = json["rows"][0].as_object().unwrap();
        let bytes = arrow_sim.step_chunk_arrow(steps).unwrap();
        let batch = StreamReader::try_new(Cursor::new(bytes), None)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(batch.num_rows(), 1);
        for (name, value) in row {
            let Some(expected) = value.as_f64() else {
                continue;
            };
            if name == "index" {
                continue;
            }
            let column = batch
                .column_by_name(name)
                .unwrap_or_else(|| panic!("JSON scalar {name} missing from Arrow schema"));
            let actual = column
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap()
                .value(0);
            assert!(
                (actual - expected).abs() < 1e-9,
                "{name}: {actual} vs {expected}"
            );
        }
    }
}
