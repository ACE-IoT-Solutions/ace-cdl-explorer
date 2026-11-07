use super::{clamp, known_type, order, param, Block};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::collections::HashSet;

type Slot = usize;
type Ports = HashMap<String, Slot>;

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

enum Op {
    Add {
        a: Slot,
        b: Slot,
        out: Slot,
        k1: f64,
        k2: f64,
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
    BooleanToInteger {
        a: Slot,
        out: Slot,
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
    Greater {
        a: Slot,
        b: Slot,
        out: Slot,
    },
    Less {
        a: Slot,
        b: Slot,
        out: Slot,
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
}

pub struct CompiledExecutor {
    slots: Vec<f64>,
    ops: Vec<Op>,
    inputs: Ports,
    outputs: Ports,
    integer_outputs: HashSet<String>,
    sampled_delays: Vec<SampledDelay>,
    trace_slots: HashMap<String, Slot>,
    selected_traces: Vec<(String, Slot)>,
}

impl CompiledExecutor {
    pub fn try_new(graph: &Block) -> Result<Option<Self>, String> {
        let mut this = Self {
            slots: Vec::new(),
            ops: Vec::new(),
            inputs: Ports::new(),
            outputs: Ports::new(),
            integer_outputs: graph
                .outputs
                .iter()
                .filter(|port| port.kind == "Integer")
                .map(|port| port.name.clone())
                .collect(),
            sampled_delays: Vec::new(),
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
            if child.block_type.rsplit('.').next() == Some("UnitDelay") && child.blocks.is_empty() {
                let value = param(child, "y_start", 0.0);
                let slot = self.slot(value);
                values.insert((child.name.clone(), "y".into()), slot);
                self.trace_slots
                    .insert(format!("{key}|{}.y", child.name), slot);
                delayed_children.push((child, slot));
            }
        }
        for index in order(block)? {
            let child = &block.blocks[index];
            if child.block_type.rsplit('.').next() == Some("UnitDelay") && child.blocks.is_empty() {
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
                .ok_or_else(|| format!("{key}: missing UnitDelay input {}.u", child.name))?;
            let source = edge.source();
            let input = *values
                .get(&source)
                .ok_or_else(|| format!("{key}: missing source {}.{}", source.0, source.1))?;
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
        if block.outputs.len() != 1 || block.outputs[0].name != "y" {
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
        let op = match kind {
            "Constant" => {
                self.slots[out] = param(block, "k", 0.0);
                None
            }
            "Add" => Some(Op::Add {
                a: input("u1")?,
                b: input("u2")?,
                out,
                k1: param(block, "k1", 1.0),
                k2: param(block, "k2", 1.0),
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
            "BooleanToInteger" => Some(Op::BooleanToInteger {
                a: input("u")?,
                out,
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
            "Greater" => Some(Op::Greater {
                a: input("u1")?,
                b: input("u2")?,
                out,
            }),
            "Less" => Some(Op::Less {
                a: input("u1")?,
                b: input("u2")?,
                out,
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
        Ok(Ports::from([("y".into(), out)]))
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
            delay.elapsed += dt;
            delay.sampled_now = delay.elapsed + 1e-9 >= delay.period;
            if delay.sampled_now {
                delay.elapsed %= delay.period;
                delay.state = delay.last_input;
            }
            self.slots[delay.output] = delay.state;
        }
        for op in &mut self.ops {
            let s = &mut self.slots;
            match op {
                Op::Add { a, b, out, k1, k2 } => s[*out] = *k1 * s[*a] + *k2 * s[*b],
                Op::Min { a, b, out } => s[*out] = s[*a].min(s[*b]),
                Op::Max { a, b, out } => s[*out] = s[*a].max(s[*b]),
                Op::BooleanToInteger { a, out } => s[*out] = if s[*a] != 0.0 { 1.0 } else { 0.0 },
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
                Op::Greater { a, b, out } => s[*out] = if s[*a] > s[*b] { 1.0 } else { 0.0 },
                Op::Less { a, b, out } => s[*out] = if s[*a] < s[*b] { 1.0 } else { 0.0 },
                Op::Subtract { a, b, out } => s[*out] = s[*a] - s[*b],
                Op::Multiply { a, b, out } => s[*out] = s[*a] * s[*b],
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
            }
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
