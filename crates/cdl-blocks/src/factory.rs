use super::*;
use crate::config::pid_controller_type;
use crate::{ConfiguredInstance, SlotAlloc};
use cdl_model::{DataType, Diagnostic, DiagnosticCode};
fn param(block: &ConfiguredInstance<'_>, name: &str, default: f64) -> f64 {
    block.number(name, default)
}
fn bool_param(block: &ConfiguredInstance<'_>, name: &str, default: bool) -> bool {
    block.boolean(name, default)
}
fn comparator_hysteresis(block: &ConfiguredInstance<'_>) -> f64 {
    if !block.is_legacy()
        && block
            .class
            .inputs
            .first()
            .is_some_and(|p| p.data_type == DataType::Integer)
    {
        0.0
    } else {
        param(block, "h", 0.0)
    }
}
fn comparator_initial(block: &ConfiguredInstance<'_>) -> bool {
    if !block.is_legacy()
        && block
            .class
            .inputs
            .first()
            .is_some_and(|p| p.data_type == DataType::Integer)
    {
        false
    } else {
        block.boolean("pre_y_start", false)
    }
}
fn diagnostic(block: &ConfiguredInstance<'_>, message: &str) -> Diagnostic {
    Diagnostic::new(block.path, DiagnosticCode::InvalidValue, message)
}
pub(crate) fn build(block: &ConfiguredInstance<'_>, io: &SlotAlloc) -> Result<Kernel, Diagnostic> {
    let kind = block.family();
    let input = |name: &str| io.input(name, block.path);
    let out = io.output("y", block.path)?;
    let timer = kind == "Timer";
    let passed = if timer {
        Some(io.output("passed", block.path)?)
    } else {
        None
    };
    let op = match kind {
        "Constant" => Some(Op::Constant {
            out,
            value: if block
                .connector("y")
                .is_some_and(|p| p.data_type == DataType::Boolean)
            {
                if block.boolean("k", false) {
                    1.0
                } else {
                    0.0
                }
            } else {
                param(block, "k", 0.0)
            },
        }),
        "Add" => Some(Op::Add {
            a: input("u1")?,
            b: input("u2")?,
            out,
            k1: if block.is_legacy() {
                param(block, "k1", 1.0)
            } else {
                1.0
            },
            k2: if block.is_legacy() {
                param(block, "k2", 1.0)
            } else {
                1.0
            },
        }),
        "AddParameter" => Some(Op::AddParameter {
            a: input("u")?,
            out,
            p: param(block, "p", 0.0),
        }),
        "Min" => Some(Op::Min {
            a: input("u1")?,
            b: input("u2")?,
            out,
        }),
        "Max" => Some(Op::Max {
            a: input("u1")?,
            b: input("u2")?,
            out,
        }),
        "Line" => Some(Op::Line {
            x1: input("x1")?,
            f1: input("f1")?,
            u: input("u")?,
            x2: input("x2")?,
            f2: input("f2")?,
            out,
            limit_below: bool_param(block, "limitBelow", true),
            limit_above: bool_param(block, "limitAbove", true),
        }),
        "BooleanToInteger" => Some(Op::BooleanToInteger {
            a: input("u")?,
            out,
            integer_true: param(block, "integerTrue", 1.0),
            integer_false: param(block, "integerFalse", 0.0),
        }),
        "RealToInteger" => Some(Op::RealToInteger {
            a: input("u")?,
            out,
        }),
        "BooleanToReal" => Some(Op::BooleanToReal {
            a: input("u")?,
            out,
            real_true: param(block, "realTrue", 1.0),
            real_false: param(block, "realFalse", 0.0),
        }),
        "IntegerToReal" => Some(Op::IntegerToReal {
            a: input("u")?,
            out,
        }),
        "UnitDelay" => Some(Op::UnitDelay(SampledDelay {
            input: input("u")?,
            output: out,
            period: param(block, "samplePeriod", 1.0).max(0.001),
            elapsed: 0.0,
            state: param(block, "y_start", 0.0),
            last_input: param(block, "y_start", 0.0),
            initialized: false,
            sampled_now: false,
        })),
        "Pre" => Some(Op::Pre(PreviousBoolean {
            input: input("u")?,
            output: out,
            state: if block.boolean("pre_u_start", false) {
                1.0
            } else {
                0.0
            },
        })),
        "TriggeredSampler" => Some(Op::TriggeredSampler {
            a: input("u")?,
            trigger: input("trigger")?,
            out,
            old_trigger: false,
        }),
        "Sampler" => Some(Op::Sampler {
            a: input("u")?,
            out,
            period: param(block, "samplePeriod", 1.0).max(0.001),
            elapsed: 0.0,
            initialized: false,
        }),
        "TriggeredMovingMean" => Some(Op::TriggeredMovingMean {
            a: input("u")?,
            trigger: input("trigger")?,
            out,
            old_trigger: false,
            initialized: false,
            samples: VecDeque::new(),
            capacity: (param(block, "n", 1.0) as usize).max(1),
        }),
        "SampleTrigger" => {
            let period = param(block, "period", 1.0);
            let shift = param(block, "shift", 0.0);
            if !period.is_finite() || period <= 0.0 || !shift.is_finite() {
                return Err(diagnostic(
                    block,
                    "SampleTrigger requires a positive period and finite shift",
                ));
            }
            Some(Op::SampleTrigger {
                out,
                period,
                shift: shift.rem_euclid(period),
                clock: 0.0,
                initialized: false,
            })
        }
        "Edge" | "FallingEdge" => Some(Op::Edge {
            a: input("u")?,
            out,
            old: block.boolean("pre_u_start", false),
            falling: kind == "FallingEdge",
        }),
        "Timer" => Some(Op::Timer {
            a: input("u")?,
            out,
            passed: passed.unwrap(),
            threshold: param(block, "t", 0.0),
            old_active: false,
            elapsed: 0.0,
        }),
        "TrueFalseHold" => {
            let true_duration = param(block, "trueHoldDuration", 0.0);
            Some(Op::TrueFalseHold {
                a: input("u")?,
                out,
                true_duration,
                false_duration: param(block, "falseHoldDuration", true_duration),
                old_input: false,
                state: false,
                age: 0.0,
                initialized: false,
            })
        }
        "MultiplyByParameter" | "Gain" => Some(Op::Gain {
            a: input("u")?,
            out,
            k: param(block, "k", 1.0),
        }),
        "Limiter" => Some(Op::Limit {
            a: input("u")?,
            out,
            low: param(
                block,
                "uMin",
                if block.is_legacy() {
                    param(block, "yMin", 0.0)
                } else {
                    0.0
                },
            ),
            high: param(
                block,
                "uMax",
                if block.is_legacy() {
                    param(block, "yMax", 1.0)
                } else {
                    1.0
                },
            ),
        }),
        "Filter" => Some(Op::Filter {
            a: input("u")?,
            out,
            alpha_time: param(block, "T", 10.0).max(0.001),
            old: None,
        }),
        "LimRateLimiter" => Some(Op::Rate {
            a: input("u")?,
            out,
            rise: param(block, "riseRate", 0.1),
            fall: param(block, "fallRate", 0.1),
            old: None,
        }),
        "Hysteresis" => Some(Op::Hysteresis {
            a: input("u")?,
            out,
            low: param(block, "uLow", -1.0),
            high: param(block, "uHigh", 1.0),
            old: block.boolean("pre_y_start", false),
        }),
        "Switch" => {
            let official_ports = block
                .connector("u2")
                .is_some_and(|port| port.data_type == DataType::Boolean);
            Some(Op::Switch {
                a: input("u1")?,
                b: input(if official_ports { "u3" } else { "u2" })?,
                choice: input(if official_ports { "u2" } else { "u3" })?,
                out,
            })
        }
        "LimPID" => Some(Op::Pi {
            setpoint: input("u_s")?,
            measured: input("u_m")?,
            out,
            reverse: block.boolean("reverseActing", false),
            k: param(block, "k", 0.5),
            ti: param(block, "Ti", 60.0).max(0.001),
            low: param(block, "yMin", 0.0),
            high: param(block, "yMax", 1.0),
            integral: 0.0,
        }),
        "PIDWithReset" => Some(Op::PiWithReset {
            setpoint: input("u_s")?,
            measured: input("u_m")?,
            trigger: input("trigger")?,
            out,
            reverse: block.boolean("reverseActing", true),
            k: param(block, "k", 1.0),
            ti: param(block, "Ti", 0.5).max(1e-12),
            scale: param(block, "r", 1.0).max(1e-12),
            low: param(block, "yMin", 0.0),
            high: param(block, "yMax", 1.0),
            ni: param(block, "Ni", 0.9).max(1e-12),
            controller_type: pid_controller_type(block)
                .ok_or_else(|| diagnostic(block, "invalid controllerType"))?,
            td: param(block, "Td", 0.1),
            nd: param(block, "Nd", 10.0),
            derivative_x: None,
            derivative_start: param(block, "yd_start", 0.0),
            reset: param(block, "y_reset", param(block, "xi_start", 0.0)),
            integral: param(block, "xi_start", 0.0),
            previous_trigger: false,
            trigger_initialized: false,
        }),
        "Greater" => Some(Op::Greater {
            a: input("u1")?,
            b: input("u2")?,
            out,
            hysteresis: comparator_hysteresis(block),
            old: comparator_initial(block),
        }),
        "GreaterThreshold" => Some(Op::GreaterThreshold {
            a: input("u")?,
            out,
            threshold: param(block, "t", 0.0),
            hysteresis: comparator_hysteresis(block),
            old: comparator_initial(block),
        }),
        "LessThreshold" => Some(Op::LessThreshold {
            a: input("u")?,
            out,
            threshold: param(block, "t", 0.0),
            hysteresis: comparator_hysteresis(block),
            old: comparator_initial(block),
        }),
        "Equal" => Some(Op::Equal {
            a: input("u1")?,
            b: input("u2")?,
            out,
        }),
        "Less" => Some(Op::Less {
            a: input("u1")?,
            b: input("u2")?,
            out,
            hysteresis: comparator_hysteresis(block),
            old: comparator_initial(block),
        }),
        "Subtract" => Some(Op::Subtract {
            a: input("u1")?,
            b: input("u2")?,
            out,
        }),
        "Multiply" => Some(Op::Multiply {
            a: input("u1")?,
            b: input("u2")?,
            out,
        }),
        "Divide" => Some(Op::Divide {
            a: input("u1")?,
            b: input("u2")?,
            out,
        }),
        "And" => Some(Op::And {
            a: input("u1")?,
            b: input("u2")?,
            out,
        }),
        "Or" => Some(Op::Or {
            a: input("u1")?,
            b: input("u2")?,
            out,
        }),
        "Not" => Some(Op::Not {
            a: input("u")?,
            out,
        }),
        "Delay" => Some(Op::Delay {
            a: input("u")?,
            out,
            old: None,
        }),
        "TrueDelay" => Some(Op::TrueDelay {
            a: input("u")?,
            out,
            delay: param(block, "delayTime", 0.0).max(0.0),
            delay_on_init: block.boolean("delayOnInit", false),
            elapsed: None,
            old_active: None,
        }),
        "Latch" => Some(Op::Latch {
            set: input("u")?,
            clear: input("clr")?,
            out,
            old_set: None,
            old_clear: None,
            state: false,
        }),
        _ => return Err(diagnostic(block, "unsupported leaf")),
    };
    let initial = match &op {
        Some(Op::Constant { out, value }) => Some((*out, *value)),
        Some(Op::UnitDelay(delay)) => Some((delay.output, delay.state)),
        Some(Op::Pre(previous)) => Some((previous.output, previous.state)),
        Some(Op::TriggeredSampler { .. }) => Some((out, param(block, "y_start", 0.0))),
        _ => None,
    };
    Ok(Kernel {
        op: op.ok_or_else(|| diagnostic(block, "unsupported leaf"))?,
        initial,
    })
}
