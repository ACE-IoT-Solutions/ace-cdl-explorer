use cdl_blocks::Registry;
use cdl_exec::{Error, Program, Scalar};
use serde_json::{json, Value};

fn compile(source: Value) -> Program {
    let document = cdl_compat::from_value(&source, Default::default()).unwrap();
    Program::compile(&document, &Registry::new(document.library.clone())).unwrap()
}
fn delay(period: f64) -> Value {
    json!({"name":"delay","block_type":"Buildings.Controls.OBC.CDL.Discrete.UnitDelay",
        "inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],
        "parameters":[{"name":"samplePeriod","value":period}]})
}
#[test]
fn independently_instantiated_states_and_handles_are_typed_and_scoped() {
    let program = compile(delay(15.0));
    let input = program.input("u").unwrap();
    let output = program.output("y").unwrap();
    let mut a = program.instantiate();
    let mut b = program.instantiate();
    assert!(matches!(a.step(1.0), Err(Error::MissingInput(_))));
    assert_eq!(a.set(&output, Scalar::Real(1.0)), Err(Error::InvalidHandle));
    assert_eq!(
        a.set(&input, Scalar::Boolean(true)),
        Err(Error::TypeMismatch)
    );
    assert_eq!(
        a.set(&input, Scalar::Real(f64::NAN)),
        Err(Error::InvalidValue)
    );
    a.set(&input, Scalar::Real(4.0)).unwrap();
    b.set_input("u", Scalar::Real(99.0)).unwrap();
    a.step(0.0).unwrap();
    b.step(0.0).unwrap();
    a.set(&input, Scalar::Real(9.0)).unwrap();
    a.step(15.0).unwrap();
    b.step(15.0).unwrap();
    assert_eq!(a.get(&output).unwrap(), Scalar::Real(4.0));
    assert_eq!(b.output("y").unwrap(), Scalar::Real(99.0));
    assert_eq!(a.step(-1.0), Err(Error::InvalidStep));
    let foreign = compile(delay(15.0)).input("u").unwrap();
    assert_eq!(
        a.set(&foreign, Scalar::Real(1.0)),
        Err(Error::InvalidHandle)
    );
    assert_eq!(a.get(&foreign), Err(Error::InvalidHandle));
    assert!(matches!(
        program.trace("absent"),
        Err(Error::UnknownPort(_))
    ));
}
#[test]
fn nonconvergent_pre_iteration_restores_held_outputs() {
    let program = compile(
        json!({"name":"oscillator","block_type":"Composite","category":"composite",
        "outputs":[{"name":"y","type":"Boolean"}],
        "blocks":[
            {"name":"pre","block_type":"Pre","inputs":[{"name":"u","type":"Boolean"}],"outputs":[{"name":"y","type":"Boolean"}]},
            {"name":"not","block_type":"Not","inputs":[{"name":"u","type":"Boolean"}],"outputs":[{"name":"y","type":"Boolean"}]}
        ], "connections":[{"from":"pre.y","to":"not.u"},{"from":"not.y","to":"pre.u"},{"from":"pre.y","to":"y"}]}),
    );
    let mut state = program.instantiate();
    assert_eq!(state.step(1.0), Err(Error::NonConvergent));
    assert_eq!(state.output("y").unwrap(), Scalar::Boolean(false));
    assert_eq!(state.step(1.0), Err(Error::NonConvergent));
    for path in [
        "oscillator.pre.y",
        "oscillator|pre.y",
        "oscillator|edge.0",
        "oscillator|output.y",
    ] {
        assert_eq!(
            state.get(&program.trace(path).unwrap()).unwrap(),
            Scalar::Boolean(false)
        );
    }
}
#[test]
fn building_alignment_preserves_sampled_and_timed_resolution() {
    assert_eq!(compile(delay(60.0)).aligned_step(900).unwrap(), 60);
    assert!(compile(delay(17.0)).aligned_step(900).is_err());
    assert!(compile(delay(0.5)).aligned_step(900).is_err());
    assert!(compile(delay(60.0)).aligned_step(0).is_err());
    for (family, parameters, expected) in [
        ("TrueDelay", json!([{"name":"delayTime","value":30}]), 30),
        (
            "TrueFalseHold",
            json!([{"name":"trueHoldDuration","value":75}]),
            75,
        ),
        (
            "SampleTrigger",
            json!([{"name":"period","value":60},{"name":"shift","value":-15}]),
            15,
        ),
    ] {
        let inputs = if family == "SampleTrigger" {
            json!([])
        } else {
            json!([{"name":"u","type":"Boolean"}])
        };
        let source = json!({"name":"timed","block_type":family,"inputs":inputs,
            "outputs":[{"name":"y","type":"Boolean"}],"parameters":parameters});
        let program = compile(source.clone());
        let document = cdl_compat::from_value(&source, Default::default()).unwrap();
        let registry = Registry::new(document.library.clone());
        assert_eq!(program.aligned_step(900).unwrap(), expected);
        assert_eq!(
            cdl_exec::aligned_step(&document, &registry, 900),
            program.aligned_step(900)
        );
    }
}

#[test]
fn free_form_equations_are_explicitly_feature_gated() {
    let source = json!({"name":"equations","block_type":"CustomArithmetic",
        "inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],
        "parameters":[{"name":"k","value":3}],
        "equations":[{"lhs":"intermediate","rhs":"k*u"},{"lhs":"y","rhs":"max(intermediate, 1)"}]});
    let document = cdl_compat::from_value(&source, Default::default()).unwrap();
    let result = Program::compile(&document, &Registry::new(document.library.clone()));
    #[cfg(not(feature = "equations"))]
    assert!(result
        .err()
        .unwrap()
        .to_string()
        .contains("equations Cargo feature"));
    #[cfg(feature = "equations")]
    {
        let mut state = result.unwrap().instantiate();
        state.set_input("u", Scalar::Real(2.0)).unwrap();
        state.step(1.0).unwrap();
        assert_eq!(state.output("y").unwrap(), Scalar::Real(6.0));
    }
}

#[test]
fn timing_can_be_inspected_before_editor_wiring_is_complete() {
    let source = json!({"name":"editing","block_type":"Composite","category":"composite",
        "blocks":[{"name":"delay","block_type":"TrueDelay",
            "inputs":[{"name":"u","type":"Boolean"}],"outputs":[{"name":"y","type":"Boolean"}],
            "parameters":[{"name":"delayTime","value":30}]}]});
    let document = cdl_compat::from_value(&source, Default::default()).unwrap();
    let registry = Registry::new(document.library.clone());
    assert!(Program::compile(&document, &registry).is_err());
    assert_eq!(
        cdl_exec::aligned_step(&document, &registry, 900).unwrap(),
        30
    );
    assert!(cdl_exec::aligned_step(&document, &registry, 0).is_err());
}
