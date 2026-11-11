use ace_cdl_runtime::Simulation;
use serde_json::Value;
use std::{fs, path::Path};

fn number(row: &Value, key: &str) -> f64 {
    row[key]
        .as_f64()
        .unwrap_or_else(|| panic!("missing numeric {key}"))
}

fn close(label: &str, index: usize, actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{label} at weather interval {index}: {actual} vs {expected}"
    );
}

fn check_row(row: &Value, index: usize) {
    assert_eq!(row["index"].as_u64(), Some(index as u64));
    for ahu in 1..=2 {
        let flows = row["zone_airflow"].as_array().expect("zone airflow");
        let flow: f64 = flows[(ahu - 1) * 5..ahu * 5]
            .iter()
            .map(|value| value.as_f64().expect("zone flow"))
            .sum();
        let inlet = number(row, &format!("ahu_{ahu}_mixed_air_enthalpy_kj_kg"));
        let outlet = number(row, &format!("ahu_{ahu}_supply_air_enthalpy_kj_kg"));
        let heating = number(row, &format!("ahu_{ahu}_heating_coil_delivered_kw"));
        let cooling = number(row, &format!("ahu_{ahu}_cooling_coil_delivered_kw"));
        close(
            &format!("AHU {ahu} coil air balance"),
            index,
            1.2 * flow * (outlet - inlet),
            heating - cooling,
            1e-4,
        );
    }
    for (loop_name, delivered, temperature_difference) in [
        (
            "hot",
            number(row, "heat_kw"),
            number(row, "hot_water_supply_temp_c") - number(row, "hot_water_return_temp_c"),
        ),
        (
            "chilled",
            number(row, "cool_kw"),
            number(row, "chilled_water_return_temp_c") - number(row, "chilled_water_supply_temp_c"),
        ),
    ] {
        let actual_flow = number(row, &format!("{loop_name}_water_actual_flow_kg_s"));
        close(
            &format!("{loop_name} water heat balance"),
            index,
            actual_flow * 4.18 * temperature_difference,
            delivered,
            1e-7,
        );
        let mut branch_flow = number(row, &format!("{loop_name}_water_bypass_flow_kg_s"))
            + number(row, &format!("ahu_1_{loop_name}_water_flow_kg_s"))
            + number(row, &format!("ahu_2_{loop_name}_water_flow_kg_s"));
        if loop_name == "hot" {
            branch_flow += number(row, "hot_water_reheat_flow_kg_s");
        }
        close(
            &format!("{loop_name} branch flow balance"),
            index,
            branch_flow,
            actual_flow,
            1e-9,
        );
    }
}

fn check_water_storage(row: &Value, index: usize, previous_end: Option<[f64; 2]>) -> [f64; 2] {
    let pair = |key: &str| -> [f64; 2] {
        let values = row[key]
            .as_array()
            .unwrap_or_else(|| panic!("missing {key}"));
        assert_eq!(values.len(), 2, "{key} requires hot and chilled loops");
        [values[0].as_f64().unwrap(), values[1].as_f64().unwrap()]
    };
    let start = pair("water_interval_start_c");
    let end = pair("water_interval_end_c");
    let source = pair("water_interval_source_kwh");
    let delivered = pair("water_interval_delivered_kwh");
    let capacity = pair("water_capacity_kwh_per_k");
    if let Some(previous) = previous_end {
        for loop_index in 0..2 {
            close(
                "water interval continuity",
                index,
                start[loop_index],
                previous[loop_index],
                1e-9,
            );
        }
    }
    for (loop_index, name) in ["hot", "chilled"].iter().enumerate() {
        let temperature_change = if loop_index == 0 {
            end[loop_index] - start[loop_index]
        } else {
            start[loop_index] - end[loop_index]
        };
        close(
            &format!("{name} stored-energy balance"),
            index,
            temperature_change * capacity[loop_index],
            source[loop_index] - delivered[loop_index],
            1e-6,
        );
    }
    end
}

fn verify(graph_file: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/public/examples");
    let graph = fs::read_to_string(root.join(graph_file)).expect("building graph");
    let weather = fs::read_to_string(root.join("weather_2025.json")).expect("weather");
    let scenario = r#"{"zone_ua_kw_per_k":0.15,"zone_capacity_kwh_per_k":5,"step_seconds":900}"#;
    let mut simulation = Simulation::new(&graph, &weather, scenario).expect("valid simulation");
    let mut interval_count = 0;
    let mut electricity_kwh = 0.0;
    let mut gas_kwh = 0.0;
    let mut previous_water_end = None;
    loop {
        let chunk: Value = serde_json::from_str(&simulation.step_chunk(672).expect("step"))
            .expect("simulation JSON");
        for row in chunk["rows"].as_array().expect("rows") {
            check_row(row, interval_count);
            previous_water_end = Some(check_water_storage(row, interval_count, previous_water_end));
            electricity_kwh += number(row, "electric_kw") * 0.25;
            gas_kwh += number(row, "gas_kw") * 0.25;
            interval_count += 1;
        }
        if chunk["done"] == true {
            assert_eq!(interval_count, 35_040, "complete 2025 weather year");
            close(
                "electricity totalizer",
                interval_count,
                electricity_kwh,
                number(&chunk["summary"], "electricity_kwh"),
                1e-6,
            );
            close(
                "boiler fuel totalizer",
                interval_count,
                gas_kwh,
                number(&chunk["summary"], "gas_kwh"),
                1e-6,
            );
            println!("{graph_file}: {interval_count} intervals, air/water/energy balances passed");
            break;
        }
    }
}

fn main() {
    verify("four_pipe_building.json");
    verify("four_pipe_g36.json");
}
