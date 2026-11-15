use cdl_blocks::{ConfiguredInstance, Registry, SlotAlloc, Step, StepCtx};
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};
fn number(v: &Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_bool().map(|b| if b { 1.0 } else { 0.0 }))
        .unwrap()
}
#[test]
fn every_elementary_suite_trace_matches_both_existing_executors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let suite: Value =
        serde_json::from_slice(&fs::read(root.join("verification/suite.json")).unwrap()).unwrap();
    let mut cases = 0;
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
        if selected["blocks"].as_array().is_some_and(|b| !b.is_empty()) {
            continue;
        }
        let document = cdl_compat::from_value(selected, Default::default()).unwrap();
        let class = &document.library.classes[&document.root.class];
        let config = ConfiguredInstance::new(&document.root, class, case["id"].as_str().unwrap());
        let registry = Registry::new(document.library.clone());
        let mut io = SlotAlloc::default();
        let mut slots = Vec::new();
        let mut input_slots = Vec::new();
        for input in &class.inputs {
            let index = slots.len();
            slots.push(0.0);
            io.bind_input(&input.name, index);
            input_slots.push((input.name.clone(), index));
        }
        let mut output_slots = HashMap::new();
        for output in &class.outputs {
            let index = slots.len();
            slots.push(0.0);
            io.bind_output(&output.name, index);
            output_slots.insert(output.name.clone(), index);
        }
        let mut kernel = registry.build(&config, &io).unwrap();
        kernel.initialize(&mut slots);
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
            for (name, slot) in &input_slots {
                let raw = fields[columns.iter().position(|c| c == name).unwrap()];
                let is_bool =
                    config.connector(name).unwrap().data_type == cdl_model::DataType::Boolean;
                let value = if is_bool {
                    Value::Bool(matches!(raw, "true" | "1" | "1.0"))
                } else {
                    Value::from(
                        raw.parse::<f64>().unwrap()
                            + case["input_offsets"][name].as_f64().unwrap_or(0.0),
                    )
                };
                slots[*slot] = number(&value);
                inputs.insert(name.clone(), value);
            }
            let ctx = StepCtx::new(dt).unwrap();
            kernel.prepare(&mut slots, &ctx);
            kernel.step(&mut slots, &ctx).unwrap();
            kernel.commit(&slots);
            let (fast, _) = compiled.execute(&inputs, dt, false).unwrap();
            let slow = interpreter.execute(&legacy, &inputs, dt, "").unwrap();
            for (name, index) in &output_slots {
                let actual = slots[*index];
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
    assert_eq!(cases, 20);
    println!("{cases} elementary suite traces match both executors exactly");
}
