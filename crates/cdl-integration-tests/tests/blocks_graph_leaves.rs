use cdl_blocks::{ConfiguredInstance, Registry, SlotAlloc, Step, StepCtx};
use serde_json::Value;
use std::collections::HashMap;
fn scalar(value: &Value) -> f64 {
    value
        .as_f64()
        .or_else(|| value.as_bool().map(|v| if v { 1.0 } else { 0.0 }))
        .unwrap()
}
fn compare(
    block: &cdl_runtime_oracle::Block,
    document: &cdl_model::Document,
    registry: &Registry,
    path: &str,
    count: &mut usize,
) {
    let flat = document.instances().unwrap();
    let by_path: HashMap<_, _> = flat.iter().map(|f| (f.path.to_string(), f)).collect();
    fn visit(
        block: &cdl_runtime_oracle::Block,
        registry: &Registry,
        path: &str,
        by_path: &HashMap<String, &cdl_model::FlatInstance<'_>>,
        count: &mut usize,
    ) {
        if !block.blocks.is_empty() {
            for child in &block.blocks {
                visit(
                    child,
                    registry,
                    &format!("{path}.{}", child.name),
                    by_path,
                    count,
                );
            }
            return;
        }
        let flat = by_path[path];
        let config = ConfiguredInstance::new(flat.instance, flat.class, path);
        let mut slots = Vec::new();
        let mut io = SlotAlloc::default();
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for port in &block.inputs {
            let slot = slots.len();
            slots.push(0.0);
            io.bind_input(&port.name, slot);
            inputs.push((port, slot));
        }
        for port in &block.outputs {
            let slot = slots.len();
            slots.push(0.0);
            io.bind_output(&port.name, slot);
            outputs.push((port, slot));
        }
        let mut kernel = registry
            .build(&config, &io)
            .unwrap_or_else(|e| panic!("{path}: {e}"));
        kernel.initialize(&mut slots);
        let mut compiled = cdl_runtime_oracle::CompiledExecutor::try_new(block)
            .unwrap()
            .unwrap();
        let mut interpreter = cdl_runtime_oracle::Executor::default();
        for frame in 0..12 {
            let mut values = HashMap::new();
            for (port, slot) in &inputs {
                let value = if port.kind == "Boolean" {
                    Value::Bool(frame % 4 < 2)
                } else {
                    Value::from(match port.name.as_str() {
                        "x1" => 0.0,
                        "x2" => 10.0,
                        "f1" => -1.0,
                        "f2" => 1.0,
                        "u2" => [1.0, 2.0, -1.0, 5.0][frame % 4],
                        "u_s" => 1.0,
                        "u_m" => 0.4,
                        _ => [0.0, 2.0, -1.0, 5.0][frame % 4],
                    })
                };
                slots[*slot] = scalar(&value);
                values.insert(port.name.clone(), value);
            }
            let dt = if frame == 0 {
                0.0
            } else {
                [1.0, 15.0, 20.0][frame % 3]
            };
            let ctx = StepCtx::new(dt).unwrap();
            if let Some(mut guess) = kernel.event_state() {
                let baseline = kernel.clone();
                let saved_slots = slots.clone();
                for iteration in 0..32 {
                    kernel = baseline.clone();
                    slots.clone_from_slice(&saved_slots);
                    kernel.set_event_state(guess);
                    kernel.prepare(&mut slots, &ctx);
                    kernel.step(&mut slots, &ctx).unwrap();
                    kernel.commit(&slots);
                    let next = kernel.event_state().unwrap();
                    if next == guess {
                        break;
                    }
                    guess = next;
                    assert!(iteration < 31, "Pre event iteration did not converge");
                }
            } else {
                kernel.prepare(&mut slots, &ctx);
                kernel.step(&mut slots, &ctx).unwrap();
                kernel.commit(&slots);
            }
            let (fast, _) = compiled.execute(&values, dt, false).unwrap();
            let slow = interpreter.execute(block, &values, dt, "").unwrap();
            for (port, slot) in &outputs {
                let actual = slots[*slot];
                assert!(
                    (actual - scalar(&fast[&port.name])).abs() < 1e-10,
                    "{path} frame {frame} {}: {actual} vs compiled {}",
                    port.name,
                    fast[&port.name]
                );
                assert!(
                    (actual - scalar(&slow[&port.name])).abs() < 1e-10,
                    "{path} frame {frame} {}: {actual} vs interpreter {}",
                    port.name,
                    slow[&port.name]
                );
            }
        }
        *count += 1;
    }
    visit(block, registry, path, &by_path, count);
}
#[test]
fn every_g36_leaf_builds_and_matches_both_oracles_over_state_transitions() {
    let source = include_str!("../../../web/public/examples/four_pipe_g36.json");
    let document = cdl_compat::from_json(source, Default::default()).unwrap();
    let registry = Registry::new(document.library.clone());
    let block: cdl_runtime_oracle::Block = serde_json::from_str(source).unwrap();
    let mut count = 0;
    compare(&block, &document, &registry, &block.name, &mut count);
    assert_eq!(count, 7077);
}

#[test]
fn all_43_legacy_families_match_both_oracles_over_state_transitions() {
    use serde_json::json;
    let mut blocks = Vec::new();
    for name in cdl_blocks::LEGACY_NAMES {
        let boolean = matches!(
            *name,
            "And"
                | "Or"
                | "Not"
                | "Pre"
                | "Edge"
                | "FallingEdge"
                | "TrueDelay"
                | "TrueFalseHold"
                | "Latch"
                | "Timer"
                | "BooleanToInteger"
                | "BooleanToReal"
        );
        let kind = if boolean {
            "Boolean"
        } else if *name == "IntegerToReal" {
            "Integer"
        } else {
            "Real"
        };
        let mut inputs = match *name {
            "Constant" | "SampleTrigger" => vec![],
            "Add" | "Min" | "Max" | "Greater" | "Less" | "Equal" | "Subtract" | "Multiply"
            | "Divide" | "And" | "Or" => vec![("u1", kind), ("u2", kind)],
            "Switch" => vec![("u1", "Real"), ("u2", "Real"), ("u3", "Boolean")],
            "Line" => vec![
                ("x1", "Real"),
                ("f1", "Real"),
                ("u", "Real"),
                ("x2", "Real"),
                ("f2", "Real"),
            ],
            "PIDWithReset" | "LimPID" => vec![("u_s", "Real"), ("u_m", "Real")],
            "Latch" => vec![("u", "Boolean"), ("clr", "Boolean")],
            _ => vec![("u", kind)],
        };
        if matches!(
            *name,
            "PIDWithReset" | "TriggeredSampler" | "TriggeredMovingMean"
        ) {
            inputs.push(("trigger", "Boolean"));
        }
        let output_kind = if matches!(*name, "BooleanToInteger" | "RealToInteger") {
            "Integer"
        } else if matches!(*name, "BooleanToReal" | "IntegerToReal" | "Timer") {
            "Real"
        } else if boolean
            || matches!(
                *name,
                "SampleTrigger"
                    | "Hysteresis"
                    | "Greater"
                    | "Less"
                    | "Equal"
                    | "GreaterThreshold"
                    | "LessThreshold"
            )
        {
            "Boolean"
        } else {
            "Real"
        };
        let mut outputs = vec![json!({"name":"y", "type":output_kind})];
        if *name == "Timer" {
            outputs.push(json!({"name":"passed", "type":"Boolean"}));
        }
        let parameters = match *name {
            "UnitDelay" | "Sampler" => json!([{"name":"samplePeriod", "value":2}]),
            "SampleTrigger" => json!([{"name":"period", "value":2}, {"name":"shift", "value":1}]),
            "TriggeredMovingMean" => json!([{"name":"n", "value":3}]),
            "TrueFalseHold" => json!([{"name":"trueHoldDuration", "value":2}]),
            _ => json!([]),
        };
        blocks.push(json!({"name":name, "block_type":format!("Buildings.Controls.OBC.CDL.Continuous.{name}"),
            "inputs":inputs.iter().map(|(name, kind)|json!({"name":name,"type":kind})).collect::<Vec<_>>(),
            "outputs":outputs, "parameters":parameters}));
    }
    let source = json!({"name":"families", "block_type":"FamilyChecks", "category":"composite", "blocks":blocks});
    let document = cdl_compat::from_value(&source, Default::default()).unwrap();
    let registry = Registry::new(document.library.clone());
    let block: cdl_runtime_oracle::Block = serde_json::from_value(source).unwrap();
    let mut count = 0;
    compare(&block, &document, &registry, &block.name, &mut count);
    assert_eq!(count, 43);
}
