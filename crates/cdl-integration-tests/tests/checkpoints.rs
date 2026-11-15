#![cfg(all(feature = "checkpoints", feature = "verification"))]
use cdl_blocks::Registry;
use cdl_exec::{Program, Scalar, StateSnapshot};
use cdl_verify::{input_scalar, read_csv, Suite};
use std::path::Path;
fn program(source: &str) -> Program {
    let document = cdl_compat::from_json(source, Default::default()).unwrap();
    Program::compile(&document, &Registry::new(document.library.clone())).unwrap()
}
#[test]
fn every_suite_case_continues_exactly_after_json_checkpoint_and_recompile() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../verification");
    let suite = Suite::load(root.join("suite.json")).unwrap();
    for case in &suite.cases {
        let document = suite.document(case).unwrap();
        let registry = Registry::new(document.library.clone());
        let p = Program::compile(&document, &registry).unwrap();
        let mut reordered = document.clone();
        reordered.library.classes = reordered.library.classes.into_iter().rev().collect();
        let q = Program::compile(&reordered, &Registry::new(reordered.library.clone())).unwrap();
        let mut uninterrupted = p.instantiate();
        let mut resumed = q.instantiate();
        let trend = read_csv(root.join(&case.trend)).unwrap();
        let input_names: Vec<_> = document.library.classes[&document.root.class]
            .inputs
            .iter()
            .map(|i| {
                (
                    i.name.clone(),
                    p.input(&i.name).unwrap(),
                    q.input(&i.name).unwrap(),
                )
            })
            .collect();
        let output = p.output(&case.output).unwrap();
        let resumed_output = q.output(&case.output).unwrap();
        let cut = (trend.rows.len() / 2).max(1);
        let mut previous = None;
        for (index, row) in trend.rows.iter().enumerate() {
            if index == cut {
                let snapshot = uninterrupted.snapshot().unwrap();
                let json = serde_json::to_string(&snapshot).unwrap();
                let decoded: StateSnapshot = serde_json::from_str(&json).unwrap();
                resumed
                    .restore(&decoded)
                    .unwrap_or_else(|e| panic!("{}: {e}", case.id));
                assert_eq!(
                    snapshot,
                    resumed.snapshot().unwrap(),
                    "{} checkpoint",
                    case.id
                );
            }
            for (name, a, b) in &input_names {
                let value = row.inputs[name] + case.input_offsets.get(name).copied().unwrap_or(0.);
                uninterrupted
                    .set(a, input_scalar(value, a.data_type()).unwrap())
                    .unwrap();
                if index >= cut {
                    resumed
                        .set(b, input_scalar(value, b.data_type()).unwrap())
                        .unwrap();
                }
            }
            let dt = previous.map_or(0., |t| row.time - t);
            previous = Some(row.time);
            uninterrupted.step(dt).unwrap();
            if index >= cut {
                resumed.step(dt).unwrap();
                assert_eq!(
                    uninterrupted.get(&output).unwrap(),
                    resumed.get(&resumed_output).unwrap(),
                    "{} sample {}",
                    case.id,
                    index
                );
            }
        }
        assert_eq!(
            uninterrupted.snapshot().unwrap(),
            resumed.snapshot().unwrap(),
            "{} final memory",
            case.id
        );
    }
}
#[test]
fn bad_checkpoints_are_rejected_atomically_and_inputs_flags_survive() {
    let source = r#"{"name":"gain","block_type":"Gain","inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],"parameters":[{"name":"k","value":2}]}"#;
    let p = program(source);
    let mut s = p.instantiate();
    let initial = s.snapshot().unwrap();
    let mut target = program(source).instantiate();
    target.restore(&initial).unwrap();
    assert!(target.step(1.).is_err());
    s.set_input("u", Scalar::Real(-0.)).unwrap();
    s.step(1.).unwrap();
    let good = s.snapshot().unwrap();
    for bad in [
        {
            let mut v = good.clone();
            v.version = 999;
            v
        },
        {
            let mut v = good.clone();
            v.signals.remove("gain.u");
            v
        },
        {
            let mut v = good.clone();
            v.signals.insert("gain.u".into(), f64::NAN);
            v
        },
        {
            let mut v = good.clone();
            v.blocks.values_mut().next().unwrap().version = 2;
            v
        },
    ] {
        assert!(s.restore(&bad).is_err());
        assert_eq!(s.snapshot().unwrap(), good);
    }
    let mut different = program(&source.replace("\"value\":2", "\"value\":3")).instantiate();
    assert!(different.restore(&good).is_err());
    let decoded: StateSnapshot =
        serde_json::from_str(&serde_json::to_string(&good).unwrap()).unwrap();
    assert_eq!(decoded.signals["gain.u"].to_bits(), (-0.0f64).to_bits());
}
#[test]
fn aliased_signals_must_agree_before_any_memory_is_restored() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../verification");
    let suite = Suite::load(root.join("suite.json")).unwrap();
    for case in &suite.cases {
        let d = suite.document(case).unwrap();
        let Some(body) = d.library.classes[&d.root.class].body.as_ref() else {
            continue;
        };
        let Some(wire) = body.connections.first() else {
            continue;
        };
        let name = match &wire.a.instance {
            Some(child) => format!("{}.{}.{}", d.root.name, child, wire.a.connector),
            None => format!("{}.{}", d.root.name, wire.a.connector),
        };
        let p = Program::compile(&d, &Registry::new(d.library.clone())).unwrap();
        let mut state = p.instantiate();
        let good = state.snapshot().unwrap();
        let mut bad = good.clone();
        *bad.signals.get_mut(&name).unwrap() += 0.25;
        assert!(state.restore(&bad).is_err());
        assert_eq!(state.snapshot().unwrap(), good);
        return;
    }
    panic!("suite should contain a wired composite");
}
#[test]
fn pi_and_pid_tracking_hold_output_without_synthesizing_a_reset_edge() {
    for mode in ["PI", "PID"] {
        let source = format!(
            r#"{{"name":"pid","block_type":"Buildings.Controls.OBC.CDL.Reals.PIDWithReset","inputs":[{{"name":"u_s","type":"Real"}},{{"name":"u_m","type":"Real"}},{{"name":"trigger","type":"Boolean"}}],"outputs":[{{"name":"y","type":"Real"}}],"parameters":[{{"name":"k","value":0.25}},{{"name":"Ti","value":60}},{{"name":"Td","value":10}},{{"name":"controllerType","value":"{mode}"}}]}}"#
        );
        let mut state = program(&source).instantiate();
        assert!(state.track_output("pid", 0.375).is_err());
        state.set_input("u_s", Scalar::Real(22.)).unwrap();
        state.set_input("u_m", Scalar::Real(20.)).unwrap();
        state.set_input("trigger", Scalar::Boolean(true)).unwrap();
        state.track_output("pid", 0.375).unwrap();
        state.step(0.).unwrap();
        assert_eq!(state.output("y").unwrap(), Scalar::Real(0.375));
        let good = state.snapshot().unwrap();
        assert!(state.track_output("pid", 2.).is_err());
        assert_eq!(state.snapshot().unwrap(), good);
    }
}

#[test]
fn comparator_hysteresis_memory_is_persistent_even_outside_event_rollback_list() {
    for (kind, before, after) in [("Greater", 1., -0.5), ("Less", -1., 0.5)] {
        let source = format!(
            r#"{{"name":"compare","block_type":"Buildings.Controls.OBC.CDL.Reals.{kind}","inputs":[{{"name":"u1","type":"Real"}},{{"name":"u2","type":"Real"}}],"outputs":[{{"name":"y","type":"Boolean"}}],"parameters":[{{"name":"h","value":1}}]}}"#
        );
        let p = program(&source);
        let mut a = p.instantiate();
        a.set_input("u1", Scalar::Real(before)).unwrap();
        a.set_input("u2", Scalar::Real(0.)).unwrap();
        a.step(0.).unwrap();
        assert_eq!(a.output("y").unwrap(), Scalar::Boolean(true));
        let json = serde_json::to_string(&a.snapshot().unwrap()).unwrap();
        let mut b = program(&source).instantiate();
        b.restore(&serde_json::from_str(&json).unwrap()).unwrap();
        for state in [&mut a, &mut b] {
            state.set_input("u1", Scalar::Real(after)).unwrap();
            state.step(0.).unwrap();
            assert_eq!(state.output("y").unwrap(), Scalar::Boolean(true));
        }
    }
}

#[test]
fn checkpoints_cannot_override_compiled_constants() {
    let source = r#"{"name":"constant","block_type":"Constant","outputs":[{"name":"y","type":"Real"}],"parameters":[{"name":"k","value":2}]}"#;
    let mut state = program(source).instantiate();
    let good = state.snapshot().unwrap();
    let mut bad = good.clone();
    bad.signals.insert("constant.y".into(), 3.);
    assert!(state.restore(&bad).is_err());
    assert_eq!(state.snapshot().unwrap(), good);
}
