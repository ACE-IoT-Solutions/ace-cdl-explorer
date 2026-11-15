use cdl_blocks::Registry;
use cdl_exec::{Program, Scalar};
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};
fn number(v: &Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_bool().map(|b| if b { 1.0 } else { 0.0 }))
        .unwrap()
}
#[test]
fn every_suite_trace_matches_both_existing_executors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let suite: Value =
        serde_json::from_slice(&fs::read(root.join("verification/suite.json")).unwrap()).unwrap();
    let mut cases = 0;
    let mut gated = 0;
    for case in suite["cases"].as_array().unwrap() {
        let graph = if let Some(path) = case["graph_file"].as_str() {
            serde_json::from_slice(&fs::read(root.join("verification").join(path)).unwrap())
                .unwrap()
        } else {
            case["graph"].clone()
        };
        let mut selected = &graph;
        for name in case["graph_path"].as_array().into_iter().flatten() {
            selected = selected["blocks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b["name"] == *name)
                .unwrap();
        }
        let document = cdl_compat::from_value(selected, Default::default()).unwrap();
        let class = &document.library.classes[&document.root.class];
        let registry = Registry::new(document.library.clone());
        let program = match Program::compile(&document, &registry) {
            Ok(program) => program,
            Err(error)
                if !cfg!(feature = "equations")
                    && error.to_string().contains("equations Cargo feature") =>
            {
                gated += 1;
                continue;
            }
            Err(error) => panic!("{}: {error}", case["id"]),
        };
        let mut state = program.instantiate();
        let legacy: cdl_runtime_oracle::Block = serde_json::from_value(selected.clone()).unwrap();
        let mut compiled = cdl_runtime_oracle::CompiledExecutor::try_new(&legacy)
            .unwrap()
            .unwrap();
        let mut interpreter = cdl_runtime_oracle::Executor::default();
        let csv = fs::read_to_string(
            root.join("verification")
                .join(case["trend"].as_str().unwrap()),
        )
        .unwrap();
        let mut lines = csv.lines();
        let columns: Vec<_> = lines.next().unwrap().split(',').collect();
        let mut previous = None;
        for line in lines.filter(|line| !line.trim().is_empty()) {
            let fields: Vec<_> = line.split(',').collect();
            let time: f64 = fields[0].parse().unwrap();
            let dt = previous.map_or(0.0, |t| time - t);
            previous = Some(time);
            let mut inputs = HashMap::new();
            for port in &class.inputs {
                let name = &port.name;
                let raw = fields[columns.iter().position(|c| c == name).unwrap()];
                let is_bool = port.data_type == cdl_model::DataType::Boolean;
                let value = if is_bool {
                    Value::Bool(matches!(raw, "true" | "1" | "1.0"))
                } else {
                    Value::from(
                        raw.parse::<f64>().unwrap()
                            + case["input_offsets"][name].as_f64().unwrap_or(0.0),
                    )
                };
                let typed = match port.data_type {
                    cdl_model::DataType::Boolean => Scalar::Boolean(value.as_bool().unwrap()),
                    cdl_model::DataType::Integer => Scalar::Integer(value.as_f64().unwrap() as i64),
                    _ => Scalar::Real(value.as_f64().unwrap()),
                };
                state.set_input(name, typed).unwrap();
                inputs.insert(name.clone(), value);
            }
            state
                .step(dt)
                .unwrap_or_else(|e| panic!("{} at {time}: {e}", case["id"]));
            let (fast, _) = compiled.execute(&inputs, dt, false).unwrap();
            let slow = interpreter.execute(&legacy, &inputs, dt, "").unwrap();
            for port in &class.outputs {
                let name = &port.name;
                let actual = match state.output(name).unwrap() {
                    Scalar::Real(v) => v,
                    Scalar::Integer(v) => v as f64,
                    Scalar::Boolean(v) => {
                        if v {
                            1.0
                        } else {
                            0.0
                        }
                    }
                };
                let expected = number(&fast[name]);
                assert!(
                    (actual - expected).abs() < 1e-12,
                    "{} at {time} {name}: {actual} vs compiled {expected}",
                    case["id"]
                );
                assert!(
                    (actual - number(&slow[name])).abs() < 1e-12,
                    "{} at {time} {name}: {actual} vs interpreted {}",
                    case["id"],
                    slow[name]
                );
            }
        }
        cases += 1;
    }
    assert_eq!(cases + gated, 143);
    if cfg!(feature = "equations") {
        assert_eq!(gated, 0);
    }
    println!("{cases} suite traces match both executors exactly");
}
