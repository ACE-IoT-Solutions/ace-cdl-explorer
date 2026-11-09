use super::{bool_param, clamp, known_type, order, param, Block};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};

type Slot = usize;
type Ports = HashMap<String, Slot>;

#[derive(Clone)]
struct SampledDelay {
    input: Slot,
    output: Slot,
    period: f64,
    elapsed: f64,
    state: f64,
    last_input: f64,
    initialized: bool,
    sampled_now: bool,
}

#[derive(Clone)]
struct PreviousBoolean {
    input: Slot,
    output: Slot,
    state: f64,
}

#[derive(Clone)]
enum Op {
    Add {
        a: Slot,
        b: Slot,
        out: Slot,
        k1: f64,
        k2: f64,
    },
    AddParameter {
        a: Slot,
        out: Slot,
        p: f64,
    },
    Min {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Max {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Line {
        x1: Slot,
        f1: Slot,
        u: Slot,
        x2: Slot,
        f2: Slot,
        out: Slot,
        limit_below: bool,
        limit_above: bool,
    },
    BooleanToInteger {
        a: Slot,
        out: Slot,
        integer_true: f64,
        integer_false: f64,
    },
    RealToInteger {
        a: Slot,
        out: Slot,
    },
    BooleanToReal {
        a: Slot,
        out: Slot,
        real_true: f64,
        real_false: f64,
    },
    IntegerToReal {
        a: Slot,
        out: Slot,
    },
    Gain {
        a: Slot,
        out: Slot,
        k: f64,
    },
    Limit {
        a: Slot,
        out: Slot,
        low: f64,
        high: f64,
    },
    Filter {
        a: Slot,
        out: Slot,
        alpha_time: f64,
        old: Option<f64>,
    },
    Rate {
        a: Slot,
        out: Slot,
        rise: f64,
        fall: f64,
        old: Option<f64>,
    },
    Hysteresis {
        a: Slot,
        out: Slot,
        low: f64,
        high: f64,
        old: bool,
    },
    Switch {
        a: Slot,
        b: Slot,
        choice: Slot,
        out: Slot,
    },
    Pi {
        setpoint: Slot,
        measured: Slot,
        out: Slot,
        reverse: bool,
        k: f64,
        ti: f64,
        low: f64,
        high: f64,
        integral: f64,
    },
    PiWithReset {
        setpoint: Slot,
        measured: Slot,
        trigger: Slot,
        out: Slot,
        reverse: bool,
        k: f64,
        ti: f64,
        scale: f64,
        low: f64,
        high: f64,
        ni: f64,
        reset: f64,
        integral: f64,
        previous_trigger: bool,
    },
    Greater {
        a: Slot,
        b: Slot,
        out: Slot,
        hysteresis: f64,
        old: bool,
    },
    GreaterThreshold {
        a: Slot,
        out: Slot,
        threshold: f64,
        hysteresis: f64,
        old: bool,
    },
    Equal {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Less {
        a: Slot,
        b: Slot,
        out: Slot,
        hysteresis: f64,
        old: bool,
    },
    Subtract {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Multiply {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Divide {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    And {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Or {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Not {
        a: Slot,
        out: Slot,
    },
    Delay {
        a: Slot,
        out: Slot,
        old: Option<f64>,
    },
    TrueDelay {
        a: Slot,
        out: Slot,
        delay: f64,
        delay_on_init: bool,
        elapsed: Option<f64>,
        old_active: Option<bool>,
    },
    Latch {
        set: Slot,
        clear: Slot,
        out: Slot,
        old_set: Option<bool>,
        old_clear: Option<bool>,
        state: bool,
    },
    TrueFalseHold {
        a: Slot,
        out: Slot,
        true_duration: f64,
        false_duration: f64,
        old_input: bool,
        state: bool,
        age: f64,
        initialized: bool,
    },
    TriggeredSampler {
        a: Slot,
        trigger: Slot,
        out: Slot,
        old_trigger: bool,
    },
    Sampler {
        a: Slot,
        out: Slot,
        period: f64,
        elapsed: f64,
        initialized: bool,
    },
    TriggeredMovingMean {
        a: Slot,
        trigger: Slot,
        out: Slot,
        old_trigger: bool,
        initialized: bool,
        samples: VecDeque<f64>,
        capacity: usize,
    },
    SampleTrigger {
        out: Slot,
        period: f64,
        shift: f64,
        clock: f64,
        initialized: bool,
    },
    Edge {
        a: Slot,
        out: Slot,
        old: bool,
        falling: bool,
    },
    Timer {
        a: Slot,
        out: Slot,
        passed: Slot,
        threshold: f64,
        old_active: bool,
        elapsed: f64,
    },
}

impl Op {
    fn is_stateful(&self) -> bool {
        matches!(
            self,
            Self::Filter { .. }
                | Self::Rate { .. }
                | Self::Hysteresis { .. }
                | Self::Pi { .. }
                | Self::PiWithReset { .. }
                | Self::GreaterThreshold { .. }
                | Self::Delay { .. }
                | Self::TrueDelay { .. }
                | Self::Latch { .. }
                | Self::TrueFalseHold { .. }
                | Self::TriggeredSampler { .. }
                | Self::Sampler { .. }
                | Self::TriggeredMovingMean { .. }
                | Self::SampleTrigger { .. }
                | Self::Edge { .. }
                | Self::Timer { .. }
        )
    }
}

#[derive(Clone)]
pub struct CompiledExecutor {
    slots: Vec<f64>,
    ops: Vec<Op>,
    stateful_ops: Vec<usize>,
    inputs: Ports,
    outputs: Ports,
    integer_outputs: HashSet<String>,
    boolean_outputs: HashSet<String>,
    sampled_delays: Vec<SampledDelay>,
    previous_booleans: Vec<PreviousBoolean>,
    trace_slots: HashMap<String, Slot>,
    selected_traces: Vec<(String, Slot)>,
}

impl CompiledExecutor {
    pub fn try_new(graph: &Block) -> Result<Option<Self>, String> {
        let mut this = Self {
            slots: Vec::new(),
            ops: Vec::new(),
            stateful_ops: Vec::new(),
            inputs: Ports::new(),
            outputs: Ports::new(),
            integer_outputs: graph
                .outputs
                .iter()
                .filter(|port| port.kind == "Integer")
                .map(|port| port.name.clone())
                .collect(),
            boolean_outputs: graph
                .outputs
                .iter()
                .filter(|port| port.kind == "Boolean")
                .map(|port| port.name.clone())
                .collect(),
            sampled_delays: Vec::new(),
            previous_booleans: Vec::new(),
            trace_slots: HashMap::new(),
            selected_traces: Vec::new(),
        };
        for input in &graph.inputs {
            let slot = this.slot(0.0);
            this.inputs.insert(input.name.clone(), slot);
        }
        match this.compile(graph, &this.inputs.clone(), "") {
            Ok(outputs) => {
                this.outputs = outputs;
                this.stateful_ops = this
                    .ops
                    .iter()
                    .enumerate()
                    .filter_map(|(index, op)| op.is_stateful().then_some(index))
                    .collect();
                Ok(Some(this))
            }
            Err(error) if error == "unsupported leaf" => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn slot(&mut self, value: f64) -> Slot {
        let slot = self.slots.len();
        self.slots.push(value);
        slot
    }

    fn compile(&mut self, block: &Block, inputs: &Ports, path: &str) -> Result<Ports, String> {
        let key = format!("{path}{}", block.name);
        if block.blocks.is_empty()
            && (block.category.as_deref() != Some("composite") || known_type(&block.block_type))
        {
            return self.compile_leaf(block, inputs);
        }
        let mut values: HashMap<(String, String), Slot> = HashMap::new();
        for (name, slot) in inputs {
            values.insert((name.clone(), String::new()), *slot);
            values.insert((name.clone(), name.clone()), *slot);
            self.trace_slots.insert(format!("{key}|{name}"), *slot);
        }
        let mut delayed_children = Vec::new();
        for child in &block.blocks {
            if matches!(
                child.block_type.rsplit('.').next(),
                Some("UnitDelay" | "Pre")
            ) && child.blocks.is_empty()
            {
                let value = if child.block_type.rsplit('.').next() == Some("Pre") {
                    child
                        .parameters
                        .iter()
                        .find(|p| p.name == "pre_u_start")
                        .and_then(|p| p.value.as_bool())
                        .unwrap_or(false) as u8 as f64
                } else {
                    param(child, "y_start", 0.0)
                };
                let slot = self.slot(value);
                values.insert((child.name.clone(), "y".into()), slot);
                self.trace_slots
                    .insert(format!("{key}|{}.y", child.name), slot);
                delayed_children.push((child, slot));
            }
        }
        for index in order(block)? {
            let child = &block.blocks[index];
            if matches!(
                child.block_type.rsplit('.').next(),
                Some("UnitDelay" | "Pre")
            ) && child.blocks.is_empty()
            {
                continue;
            }
            let mut child_inputs = Ports::new();
            for edge in &block.connections {
                let (dst, input) = edge.target();
                if dst != child.name {
                    continue;
                }
                let source = edge.source();
                let slot = *values
                    .get(&source)
                    .ok_or_else(|| format!("{key}: missing source {}.{}", source.0, source.1))?;
                child_inputs.insert(input, slot);
            }
            let outputs = self.compile(child, &child_inputs, &format!("{key}."))?;
            for (port, slot) in outputs {
                values.insert((child.name.clone(), port.clone()), slot);
                self.trace_slots
                    .insert(format!("{key}|{}.{}", child.name, port), slot);
            }
        }
        for (child, output) in delayed_children {
            let edge = block
                .connections
                .iter()
                .find(|edge| {
                    let (dst, input) = edge.target();
                    dst == child.name && input == "u"
                })
                .ok_or_else(|| format!("{key}: missing state block input {}.u", child.name))?;
            let source = edge.source();
            let input = *values
                .get(&source)
                .ok_or_else(|| format!("{key}: missing source {}.{}", source.0, source.1))?;
            if child.block_type.rsplit('.').next() == Some("Pre") {
                self.previous_booleans.push(PreviousBoolean {
                    input,
                    output,
                    state: self.slots[output],
                });
            } else {
                self.sampled_delays.push(SampledDelay {
                    input,
                    output,
                    period: param(child, "samplePeriod", 1.0).max(0.001),
                    elapsed: 0.0,
                    state: param(child, "y_start", 0.0),
                    last_input: param(child, "y_start", 0.0),
                    initialized: false,
                    sampled_now: false,
                });
            }
        }
        for (index, edge) in block.connections.iter().enumerate() {
            let source = edge.source();
            let slot = *values
                .get(&source)
                .ok_or_else(|| format!("{key}: missing edge source {}.{}", source.0, source.1))?;
            self.trace_slots.insert(format!("{key}|edge.{index}"), slot);
        }
        let mut outputs = Ports::new();
        for edge in &block.connections {
            let (dst, _) = edge.target();
            if block.outputs.iter().any(|port| port.name == dst) {
                let source = edge.source();
                let slot = *values.get(&source).ok_or_else(|| {
                    format!("{key}: missing output source {}.{}", source.0, source.1)
                })?;
                outputs.insert(dst.clone(), slot);
                self.trace_slots.insert(format!("{key}|output.{dst}"), slot);
            }
        }
        Ok(outputs)
    }

    fn compile_leaf(&mut self, block: &Block, inputs: &Ports) -> Result<Ports, String> {
        let timer = block.block_type.rsplit('.').next() == Some("Timer");
        if !block.outputs.iter().any(|port| port.name == "y")
            || block.outputs.len() != if timer { 2 } else { 1 }
            || (timer && !block.outputs.iter().any(|port| port.name == "passed"))
        {
            return Err("unsupported leaf".into());
        }
        let kind = block
            .block_type
            .rsplit('.')
            .next()
            .unwrap_or(&block.block_type);
        if !block.equations.is_empty() {
            let exact_builtin = block.equations.len() == 1 && block.equations[0].lhs == "y" && {
                let expression: String = block.equations[0]
                    .rhs
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                matches!(
                    (kind, expression.as_str()),
                    ("MultiplyByParameter", "k*u") | ("Min", "min(u1,u2)")
                )
            };
            if !exact_builtin {
                return Err("unsupported leaf".into());
            }
        }
        let input = |name: &str| {
            inputs
                .get(name)
                .copied()
                .ok_or_else(|| format!("{}: missing input {name}", block.name))
        };
        let out = self.slot(0.0);
        let passed = if timer { Some(self.slot(0.0)) } else { None };
        let op = match kind {
            "Constant" => {
                self.slots[out] = if block
                    .outputs
                    .first()
                    .is_some_and(|port| port.kind == "Boolean")
                {
                    if block
                        .parameters
                        .iter()
                        .find(|parameter| parameter.name == "k")
                        .and_then(|parameter| parameter.value.as_bool())
                        .unwrap_or(false)
                    {
                        1.0
                    } else {
                        0.0
                    }
                } else {
                    param(block, "k", 0.0)
                };
                None
            }
            "Add" => Some(Op::Add {
                a: input("u1")?,
                b: input("u2")?,
                out,
                k1: param(block, "k1", 1.0),
                k2: param(block, "k2", 1.0),
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
            "UnitDelay" => {
                let start = param(block, "y_start", 0.0);
                self.slots[out] = start;
                self.sampled_delays.push(SampledDelay {
                    input: input("u")?,
                    output: out,
                    period: param(block, "samplePeriod", 1.0).max(0.001),
                    elapsed: 0.0,
                    state: start,
                    last_input: start,
                    initialized: false,
                    sampled_now: false,
                });
                None
            }
            "Pre" => {
                let start = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "pre_u_start")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false) as u8 as f64;
                self.slots[out] = start;
                self.previous_booleans.push(PreviousBoolean {
                    input: input("u")?,
                    output: out,
                    state: start,
                });
                None
            }
            "TriggeredSampler" => {
                self.slots[out] = param(block, "y_start", 0.0);
                Some(Op::TriggeredSampler {
                    a: input("u")?,
                    trigger: input("trigger")?,
                    out,
                    old_trigger: false,
                })
            }
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
                    return Err("SampleTrigger requires a positive period and finite shift".into());
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
                old: block
                    .parameters
                    .iter()
                    .find(|p| p.name == "pre_u_start")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false),
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
                low: param(block, "uMin", param(block, "yMin", 0.0)),
                high: param(block, "uMax", param(block, "yMax", 1.0)),
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
                old: block
                    .parameters
                    .iter()
                    .find(|p| p.name == "pre_y_start")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false),
            }),
            "Switch" => {
                let official_ports = block
                    .inputs
                    .iter()
                    .any(|port| port.name == "u2" && port.kind == "Boolean");
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
                reverse: block
                    .parameters
                    .iter()
                    .find(|p| p.name == "reverseActing")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false),
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
                reverse: block
                    .parameters
                    .iter()
                    .find(|p| p.name == "reverseActing")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(true),
                k: param(block, "k", 1.0),
                ti: param(block, "Ti", 0.5).max(1e-12),
                scale: param(block, "r", 1.0).max(1e-12),
                low: param(block, "yMin", 0.0),
                high: param(block, "yMax", 1.0),
                ni: param(block, "Ni", 0.9).max(1e-12),
                reset: param(block, "y_reset", param(block, "xi_start", 0.0)),
                integral: param(block, "xi_start", 0.0),
                previous_trigger: false,
            }),
            "Greater" => Some(Op::Greater {
                a: input("u1")?,
                b: input("u2")?,
                out,
                hysteresis: param(block, "h", 0.0),
                old: block
                    .parameters
                    .iter()
                    .find(|p| p.name == "pre_y_start")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false),
            }),
            "GreaterThreshold" => Some(Op::GreaterThreshold {
                a: input("u")?,
                out,
                threshold: param(block, "t", 0.0),
                hysteresis: param(block, "h", 0.0),
                old: block
                    .parameters
                    .iter()
                    .find(|p| p.name == "pre_y_start")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false),
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
                hysteresis: param(block, "h", 0.0),
                old: block
                    .parameters
                    .iter()
                    .find(|p| p.name == "pre_y_start")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false),
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
                delay_on_init: block
                    .parameters
                    .iter()
                    .find(|p| p.name == "delayOnInit")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false),
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
            _ => return Err("unsupported leaf".into()),
        };
        if let Some(op) = op {
            self.ops.push(op);
        }
        let mut ports = Ports::from([("y".into(), out)]);
        if let Some(passed) = passed {
            ports.insert("passed".into(), passed);
        }
        Ok(ports)
    }

    pub(crate) fn set_traces(&mut self, targets: &[String]) -> Result<(), String> {
        self.selected_traces = targets
            .iter()
            .map(|target| {
                self.trace_slots
                    .get(target)
                    .copied()
                    .map(|slot| (target.clone(), slot))
                    .ok_or_else(|| format!("Selected graph signal {target} has no value"))
            })
            .collect::<Result<_, _>>()?;
        Ok(())
    }

    pub fn execute(
        &mut self,
        inputs: &HashMap<String, Value>,
        dt: f64,
        record_traces: bool,
    ) -> Result<(HashMap<String, Value>, serde_json::Map<String, Value>), String> {
        if self.previous_booleans.is_empty() {
            return self.execute_once(inputs, dt, record_traces);
        }
        let baseline_slots = self.slots.clone();
        let baseline_ops: Vec<(usize, Op)> = self
            .stateful_ops
            .iter()
            .map(|&index| (index, self.ops[index].clone()))
            .collect();
        let baseline_delays = self.sampled_delays.clone();
        let baseline_previous = self.previous_booleans.clone();
        let mut guess: Vec<f64> = baseline_previous.iter().map(|p| p.state).collect();
        for _ in 0..32 {
            self.restore_event_state(
                &baseline_slots,
                &baseline_ops,
                &baseline_delays,
                &baseline_previous,
            );
            for (pre, state) in self.previous_booleans.iter_mut().zip(&guess) {
                pre.state = *state;
            }
            let result = match self.execute_once(inputs, dt, record_traces) {
                Ok(result) => result,
                Err(error) => {
                    self.restore_event_state(
                        &baseline_slots,
                        &baseline_ops,
                        &baseline_delays,
                        &baseline_previous,
                    );
                    return Err(error);
                }
            };
            let next: Vec<f64> = self.previous_booleans.iter().map(|p| p.state).collect();
            if next == guess {
                return Ok(result);
            }
            guess = next;
        }
        self.restore_event_state(
            &baseline_slots,
            &baseline_ops,
            &baseline_delays,
            &baseline_previous,
        );
        Err("Boolean Pre event iteration did not converge".into())
    }

    fn restore_event_state(
        &mut self,
        slots: &[f64],
        ops: &[(usize, Op)],
        delays: &[SampledDelay],
        previous: &[PreviousBoolean],
    ) {
        self.slots.clone_from_slice(slots);
        for (index, op) in ops {
            self.ops[*index].clone_from(op);
        }
        self.sampled_delays.clone_from_slice(delays);
        self.previous_booleans.clone_from_slice(previous);
    }

    fn execute_once(
        &mut self,
        inputs: &HashMap<String, Value>,
        dt: f64,
        record_traces: bool,
    ) -> Result<(HashMap<String, Value>, serde_json::Map<String, Value>), String> {
        for (name, slot) in &self.inputs {
            let value = inputs
                .get(name)
                .ok_or_else(|| format!("Missing input {name}"))?;
            self.slots[*slot] = value
                .as_f64()
                .or_else(|| value.as_bool().map(|v| if v { 1.0 } else { 0.0 }))
                .ok_or_else(|| format!("Invalid input {name}"))?;
        }
        for delay in &mut self.sampled_delays {
            if delay.initialized {
                delay.elapsed += dt;
            }
            delay.sampled_now = delay.initialized && delay.elapsed + 1e-9 >= delay.period;
            if delay.sampled_now {
                delay.elapsed %= delay.period;
                delay.state = delay.last_input;
            }
            self.slots[delay.output] = delay.state;
        }
        for previous in &self.previous_booleans {
            self.slots[previous.output] = previous.state;
        }
        for op in &mut self.ops {
            let s = &mut self.slots;
            match op {
                Op::Add { a, b, out, k1, k2 } => s[*out] = *k1 * s[*a] + *k2 * s[*b],
                Op::AddParameter { a, out, p } => s[*out] = s[*a] + *p,
                Op::Min { a, b, out } => s[*out] = s[*a].min(s[*b]),
                Op::Max { a, b, out } => s[*out] = s[*a].max(s[*b]),
                Op::Line {
                    x1,
                    f1,
                    u,
                    x2,
                    f2,
                    out,
                    limit_below,
                    limit_above,
                } => {
                    if s[*x2] <= s[*x1] {
                        return Err("Line requires x2 > x1".into());
                    }
                    let mut x = s[*u];
                    if *limit_below {
                        x = x.max(s[*x1]);
                    }
                    if *limit_above {
                        x = x.min(s[*x2]);
                    }
                    s[*out] = s[*f1] + (s[*f2] - s[*f1]) * (x - s[*x1]) / (s[*x2] - s[*x1]);
                }
                Op::BooleanToInteger {
                    a,
                    out,
                    integer_true,
                    integer_false,
                } => {
                    s[*out] = if s[*a] != 0.0 {
                        *integer_true
                    } else {
                        *integer_false
                    }
                }
                Op::RealToInteger { a, out } => s[*out] = s[*a].round(),
                Op::BooleanToReal {
                    a,
                    out,
                    real_true,
                    real_false,
                } => {
                    s[*out] = if s[*a] != 0.0 {
                        *real_true
                    } else {
                        *real_false
                    }
                }
                Op::IntegerToReal { a, out } => s[*out] = s[*a],
                Op::Gain { a, out, k } => s[*out] = *k * s[*a],
                Op::Limit { a, out, low, high } => s[*out] = clamp(s[*a], *low, *high),
                Op::Filter {
                    a,
                    out,
                    alpha_time,
                    old,
                } => {
                    let previous = old.unwrap_or(s[*a]);
                    let value = previous + (1.0 - (-dt / *alpha_time).exp()) * (s[*a] - previous);
                    *old = Some(value);
                    s[*out] = value;
                }
                Op::Rate {
                    a,
                    out,
                    rise,
                    fall,
                    old,
                } => {
                    let previous = old.unwrap_or(s[*a]);
                    let value = clamp(s[*a], previous - *fall * dt, previous + *rise * dt);
                    *old = Some(value);
                    s[*out] = value;
                }
                Op::Hysteresis {
                    a,
                    out,
                    low,
                    high,
                    old,
                } => {
                    if s[*a] > *high {
                        *old = true
                    } else if s[*a] < *low {
                        *old = false
                    };
                    s[*out] = if *old { 1.0 } else { 0.0 };
                }
                Op::Switch { a, b, choice, out } => {
                    s[*out] = if s[*choice] != 0.0 { s[*a] } else { s[*b] }
                }
                Op::Pi {
                    setpoint,
                    measured,
                    out,
                    reverse,
                    k,
                    ti,
                    low,
                    high,
                    integral,
                } => {
                    let e = (s[*setpoint] - s[*measured]) * if *reverse { 1.0 } else { -1.0 };
                    let proposal = *integral + *k * e * dt / *ti;
                    *integral = clamp(proposal, *low - *k * e, *high - *k * e);
                    s[*out] = clamp(*k * e + *integral, *low, *high);
                }
                Op::PiWithReset {
                    setpoint,
                    measured,
                    trigger,
                    out,
                    reverse,
                    k,
                    ti,
                    scale,
                    low,
                    high,
                    ni,
                    reset,
                    integral,
                    previous_trigger,
                } => {
                    let e =
                        (s[*setpoint] - s[*measured]) * if *reverse { 1.0 } else { -1.0 } / *scale;
                    let proportional = *k * e;
                    let triggered = s[*trigger] != 0.0;
                    if triggered && !*previous_trigger {
                        *integral = *reset - proportional;
                    } else {
                        let unsaturated = proportional + *integral;
                        let saturated = clamp(unsaturated, *low, *high);
                        let antiwindup = (unsaturated - saturated) / (*k * *ni).max(1e-12);
                        *integral += *k * dt / *ti * (e - antiwindup);
                    }
                    *previous_trigger = triggered;
                    s[*out] = clamp(proportional + *integral, *low, *high);
                }
                Op::Greater {
                    a,
                    b,
                    out,
                    hysteresis,
                    old,
                } => {
                    *old = s[*a]
                        > s[*b]
                            - if *hysteresis >= 1e-10 && *old {
                                *hysteresis
                            } else {
                                0.0
                            };
                    s[*out] = if *old { 1.0 } else { 0.0 };
                }
                Op::GreaterThreshold {
                    a,
                    out,
                    threshold,
                    hysteresis,
                    old,
                } => {
                    *old = if *hysteresis >= 1e-10 {
                        if *old {
                            s[*a] > *threshold - *hysteresis
                        } else {
                            s[*a] > *threshold
                        }
                    } else {
                        s[*a] > *threshold
                    };
                    s[*out] = if *old { 1.0 } else { 0.0 };
                }
                Op::Equal { a, b, out } => s[*out] = if s[*a] == s[*b] { 1.0 } else { 0.0 },
                Op::Less {
                    a,
                    b,
                    out,
                    hysteresis,
                    old,
                } => {
                    *old = s[*a]
                        < s[*b]
                            + if *hysteresis >= 1e-10 && *old {
                                *hysteresis
                            } else {
                                0.0
                            };
                    s[*out] = if *old { 1.0 } else { 0.0 };
                }
                Op::Subtract { a, b, out } => s[*out] = s[*a] - s[*b],
                Op::Multiply { a, b, out } => s[*out] = s[*a] * s[*b],
                Op::Divide { a, b, out } => s[*out] = s[*a] / s[*b],
                Op::And { a, b, out } => {
                    s[*out] = if s[*a] != 0.0 && s[*b] != 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                Op::Or { a, b, out } => {
                    s[*out] = if s[*a] != 0.0 || s[*b] != 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                Op::Not { a, out } => s[*out] = if s[*a] == 0.0 { 1.0 } else { 0.0 },
                Op::Delay { a, out, old } => {
                    let previous = old.unwrap_or(s[*a]);
                    *old = Some(s[*a]);
                    s[*out] = previous;
                }
                Op::TrueDelay {
                    a,
                    out,
                    delay,
                    delay_on_init,
                    elapsed,
                    old_active,
                } => {
                    let active = s[*a] != 0.0;
                    let next = if active {
                        if old_active.is_none() && !*delay_on_init {
                            *delay
                        } else if *old_active == Some(true) {
                            elapsed.unwrap_or(0.0) + dt
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };
                    *elapsed = Some(next);
                    *old_active = Some(active);
                    s[*out] = if active && next >= *delay { 1.0 } else { 0.0 };
                }
                Op::Latch {
                    set,
                    clear,
                    out,
                    old_set,
                    old_clear,
                    state,
                } => {
                    let set_now = s[*set] != 0.0;
                    let clear_now = s[*clear] != 0.0;
                    if let (Some(before_set), Some(before_clear)) = (*old_set, *old_clear) {
                        if (!before_set && set_now) || (!before_clear && clear_now) {
                            *state = set_now && !clear_now;
                        }
                    } else {
                        *state = set_now && !clear_now;
                    }
                    *old_set = Some(set_now);
                    *old_clear = Some(clear_now);
                    s[*out] = if *state { 1.0 } else { 0.0 };
                }
                Op::TrueFalseHold {
                    a,
                    out,
                    true_duration,
                    false_duration,
                    old_input,
                    state,
                    age,
                    initialized,
                } => {
                    let active = s[*a] != 0.0;
                    if *initialized {
                        let previous_age = *age;
                        *age += dt;
                        let minimum = if *state {
                            *true_duration
                        } else {
                            *false_duration
                        };
                        if active != *state && *age >= minimum {
                            *age = if active != *old_input && previous_age >= minimum {
                                0.0
                            } else {
                                *age - minimum
                            };
                            *state = active;
                        }
                    } else {
                        *state = active;
                        *initialized = true;
                    }
                    *old_input = active;
                    s[*out] = if *state { 1.0 } else { 0.0 };
                }
                Op::TriggeredSampler {
                    a,
                    trigger,
                    out,
                    old_trigger,
                } => {
                    let active = s[*trigger] != 0.0;
                    if active && !*old_trigger {
                        s[*out] = s[*a];
                    }
                    *old_trigger = active;
                }
                Op::Sampler {
                    a,
                    out,
                    period,
                    elapsed,
                    initialized,
                } => {
                    if *initialized {
                        *elapsed += dt;
                    }
                    let event = *initialized && *elapsed + 1e-9 >= *period;
                    if event {
                        *elapsed %= *period;
                    }
                    if event || !*initialized {
                        s[*out] = s[*a];
                        *initialized = true;
                    }
                }
                Op::TriggeredMovingMean {
                    a,
                    trigger,
                    out,
                    old_trigger,
                    initialized,
                    samples,
                    capacity,
                } => {
                    let active = s[*trigger] != 0.0;
                    if !*initialized || (active && !*old_trigger) {
                        samples.push_back(s[*a]);
                        if samples.len() > *capacity {
                            samples.pop_front();
                        }
                        s[*out] = samples.iter().sum::<f64>() / samples.len() as f64;
                    }
                    *initialized = true;
                    *old_trigger = active;
                }
                Op::SampleTrigger {
                    out,
                    period,
                    shift,
                    clock,
                    initialized,
                } => {
                    let before = *clock;
                    if *initialized {
                        *clock += dt;
                    }
                    let crossed = *initialized
                        && ((before - *shift + 1e-9) / *period).floor()
                            < ((*clock - *shift + 1e-9) / *period).floor();
                    s[*out] = if (!*initialized && *shift <= 1e-9) || crossed {
                        1.0
                    } else {
                        0.0
                    };
                    *initialized = true;
                }
                Op::Edge {
                    a,
                    out,
                    old,
                    falling,
                } => {
                    let active = s[*a] != 0.0;
                    s[*out] = if if *falling {
                        !active && *old
                    } else {
                        active && !*old
                    } {
                        1.0
                    } else {
                        0.0
                    };
                    *old = active;
                }
                Op::Timer {
                    a,
                    out,
                    passed,
                    threshold,
                    old_active,
                    elapsed,
                } => {
                    let active = s[*a] != 0.0;
                    *elapsed = if !active || !*old_active {
                        0.0
                    } else {
                        *elapsed + dt
                    };
                    s[*out] = *elapsed;
                    s[*passed] = if active && *elapsed >= *threshold {
                        1.0
                    } else {
                        0.0
                    };
                    *old_active = active;
                }
            }
        }
        for previous in &mut self.previous_booleans {
            previous.state = if self.slots[previous.input] != 0.0 {
                1.0
            } else {
                0.0
            };
        }
        let outputs = self
            .outputs
            .iter()
            .map(|(name, slot)| {
                let value = self.slots[*slot];
                (
                    name.clone(),
                    if self.integer_outputs.contains(name) {
                        json!(value as i64)
                    } else if self.boolean_outputs.contains(name) {
                        json!(value != 0.0)
                    } else {
                        json!(value)
                    },
                )
            })
            .collect();
        let traces = if record_traces {
            self.selected_traces
                .iter()
                .map(|(name, slot)| (name.clone(), json!(self.slots[*slot])))
                .collect()
        } else {
            serde_json::Map::new()
        };
        for delay in &mut self.sampled_delays {
            if delay.sampled_now || !delay.initialized {
                delay.last_input = self.slots[delay.input];
                delay.initialized = true;
            }
        }
        Ok((outputs, traces))
    }
}
