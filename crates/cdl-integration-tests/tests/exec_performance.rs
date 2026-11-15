use cdl_blocks::Registry;
use cdl_exec::{Program, Scalar};
use cdl_model::DataType;
use serde_json::{json, Value};
use std::{collections::HashMap, hint::black_box, time::Instant};

#[test]
#[ignore = "explicit native performance gate"]
fn g36_matches_the_original_compiled_engine_with_no_more_than_five_percent_overhead() {
    let source = include_str!("../../../web/public/examples/four_pipe_g36.json");
    let document = cdl_compat::from_json(source, Default::default()).unwrap();
    let program = Program::compile(&document, &Registry::new(document.library.clone())).unwrap();
    let block: cdl_runtime_oracle::Block = serde_json::from_str(source).unwrap();
    let inputs: HashMap<_, _> = block
        .inputs
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                if p.kind == "Boolean" {
                    json!(false)
                } else {
                    json!(2.0)
                },
            )
        })
        .collect();
    let handles: Vec<_> = block
        .inputs
        .iter()
        .map(|p| {
            let handle = program.input(&p.name).unwrap();
            let value = match handle.data_type() {
                DataType::Boolean => Scalar::Boolean(false),
                DataType::Integer => Scalar::Integer(2),
                _ => Scalar::Real(2.0),
            };
            (handle, value)
        })
        .collect();
    let outputs: Vec<_> = block
        .outputs
        .iter()
        .map(|p| (p.name.clone(), program.output(&p.name).unwrap()))
        .collect();
    let run_new = |state: &mut cdl_exec::State| {
        for (handle, value) in &handles {
            state.set(handle, *value).unwrap();
        }
        state.step(15.0).unwrap();
        outputs
            .iter()
            .map(|(name, handle)| {
                let value = match state.get(handle).unwrap() {
                    Scalar::Real(v) => json!(v),
                    Scalar::Integer(v) => json!(v),
                    Scalar::Boolean(v) => json!(v),
                };
                (name.clone(), value)
            })
            .collect::<HashMap<String, Value>>()
    };
    let mut state = program.instantiate();
    let mut original = cdl_runtime_oracle::CompiledExecutor::try_new(&block)
        .unwrap()
        .unwrap();
    for frame in 0..100 {
        assert_eq!(
            run_new(&mut state),
            original.execute(&inputs, 15.0, false).unwrap().0,
            "frame {frame}"
        );
    }
    let frames = 10_000;
    let start = Instant::now();
    for _ in 0..frames {
        black_box(original.execute(&inputs, 15.0, false).unwrap());
    }
    let old = start.elapsed();
    let start = Instant::now();
    for _ in 0..frames {
        black_box(run_new(&mut state));
    }
    let new = start.elapsed();
    println!(
        "{frames} G36 frames: original {old:?}, new {new:?}, ratio {:.3}",
        new.as_secs_f64() / old.as_secs_f64()
    );
    assert!(new.as_secs_f64() <= old.as_secs_f64() * 1.05);
}
