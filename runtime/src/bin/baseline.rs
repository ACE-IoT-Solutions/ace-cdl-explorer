use ace_cdl_runtime::Simulation;
use std::fs;

fn main() {
    // Optional fourth and fifth arguments select graph traces and output row stride.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/public/examples");
    let mut args = std::env::args().skip(1);
    let graph_file = args
        .next()
        .unwrap_or_else(|| "four_pipe_building.json".into());
    // Keep manual replay output outside the published examples unless the
    // caller deliberately selects a destination.
    let output_file = args.next().unwrap_or_else(|| {
        std::env::temp_dir()
            .join("ace-cdl-baseline-2025.json")
            .to_string_lossy()
            .into_owned()
    });
    let step_seconds: usize = args.next().as_deref().unwrap_or("900").parse().unwrap();
    let trace_file = args.next();
    let row_stride: u64 = args.next().as_deref().unwrap_or("4").parse().unwrap();
    assert!(row_stride > 0);
    let graph = fs::read_to_string(root.join(graph_file)).unwrap();
    let weather = fs::read_to_string(root.join("weather_2025.json")).unwrap();
    let mut sim = Simulation::new(
        &graph,
        &weather,
        &format!(r#"{{"zone_ua_kw_per_k":0.15,"zone_capacity_kwh_per_k":5,"step_seconds":{step_seconds}}}"#),
    )
    .unwrap();
    if let Some(path) = trace_file {
        sim.set_traces(&fs::read_to_string(path).unwrap()).unwrap();
    }
    let mut rows = Vec::new();
    let summary = loop {
        let value: serde_json::Value = serde_json::from_str(&sim.step_chunk(672).unwrap()).unwrap();
        rows.extend(
            value["rows"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["index"].as_u64().unwrap() % row_stride == 0)
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
