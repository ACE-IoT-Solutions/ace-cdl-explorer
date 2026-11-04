use ace_cdl_runtime::Simulation;
use std::fs;

fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/public/examples");
    let graph = fs::read_to_string(root.join("four_pipe_building.json")).unwrap();
    let weather = fs::read_to_string(root.join("weather_2025.json")).unwrap();
    let mut sim = Simulation::new(
        &graph,
        &weather,
        r#"{"zone_ua_kw_per_k":0.15,"zone_capacity_kwh_per_k":5,"heat_pump_fraction":0.55}"#,
    )
    .unwrap();
    let mut rows = Vec::new();
    let summary = loop {
        let value: serde_json::Value = serde_json::from_str(&sim.step_chunk(672).unwrap()).unwrap();
        rows.extend(
            value["rows"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["index"].as_u64().unwrap() % 4 == 0)
                .cloned(),
        );
        if value["done"] == true {
            break value["summary"].clone();
        }
    };
    fs::write(
        root.join("baseline_2025.json"),
        serde_json::to_string(&serde_json::json!({"rows":rows,"summary":summary})).unwrap(),
    )
    .unwrap();
}
