use ace_cdl_runtime::Simulation;
use std::fs;

fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/public/examples");
    let mut args = std::env::args().skip(1);
    let graph_file = args
        .next()
        .unwrap_or_else(|| "four_pipe_building.json".into());
    let output_file = args.next().unwrap_or_else(|| "baseline_2025.json".into());
    let step_seconds: usize = args.next().as_deref().unwrap_or("900").parse().unwrap();
    let graph = fs::read_to_string(root.join(graph_file)).unwrap();
    let weather = fs::read_to_string(root.join("weather_2025.json")).unwrap();
    let mut sim = Simulation::new(
        &graph,
        &weather,
        &format!(r#"{{"zone_ua_kw_per_k":0.15,"zone_capacity_kwh_per_k":5,"heat_pump_fraction":0.55,"step_seconds":{step_seconds}}}"#),
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
        if output_file.starts_with('/') {
            std::path::PathBuf::from(output_file)
        } else {
            root.join(output_file)
        },
        serde_json::to_string(&serde_json::json!({"rows":rows,"summary":summary})).unwrap(),
    )
    .unwrap();
}
