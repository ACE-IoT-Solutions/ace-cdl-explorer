use crate::model::*;
use crate::validate::{known_type, order};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

struct Plan {
    order: Vec<usize>,
    incoming: Vec<Vec<(String, String, String)>>,
    outputs: Vec<(String, String, String)>,
}

impl Plan {
    fn compile(block: &Block) -> Result<Self, String> {
        let mut incoming = vec![Vec::new(); block.blocks.len()];
        let mut outputs = Vec::new();
        for edge in &block.connections {
            let (src, out) = edge.source();
            let (dst, input) = edge.target();
            if let Some(index) = block.blocks.iter().position(|child| child.name == dst) {
                incoming[index].push((src.clone(), out.clone(), input));
            }
            if block.outputs.iter().any(|port| port.name == dst) {
                outputs.push((src, out, dst));
            }
        }
        Ok(Self {
            order: order(block)?,
            incoming,
            outputs,
        })
    }
}

#[derive(Clone, Default)]
pub struct Executor {
    state: HashMap<String, f64>,
    sample_elapsed: HashMap<String, f64>,
    sample_input: HashMap<String, f64>,
    sample_initialized: HashSet<String>,
    flags: HashMap<String, bool>,
    moving_samples: HashMap<String, VecDeque<f64>>,
    plans: HashMap<String, Arc<Plan>>,
    pub trace_targets: Vec<String>,
    pub trace_values: HashMap<String, Value>,
    pub skip_traces: bool,
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}
fn get(inputs: &HashMap<String, Value>, key: &str) -> Result<f64, String> {
    inputs
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing numeric input {key}"))
}
pub(crate) fn param(block: &Block, key: &str, default: f64) -> f64 {
    block
        .parameters
        .iter()
        .find(|p| p.name == key)
        .map(|p| num(&p.value))
        .unwrap_or(default)
}
pub(crate) fn bool_param(block: &Block, key: &str, default: bool) -> bool {
    block
        .parameters
        .iter()
        .find(|p| p.name == key)
        .and_then(|p| p.value.as_bool())
        .unwrap_or(default)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PidControllerType {
    P,
    Pi,
    Pd,
    Pid,
}
impl PidControllerType {
    pub(crate) fn with_integral(self) -> bool {
        matches!(self, Self::Pi | Self::Pid)
    }
    pub(crate) fn with_derivative(self) -> bool {
        matches!(self, Self::Pd | Self::Pid)
    }
}
pub(crate) fn pid_controller_type(block: &Block) -> Option<PidControllerType> {
    let Some(parameter) = block.parameters.iter().find(|p| p.name == "controllerType") else {
        return Some(PidControllerType::Pi);
    };
    match parameter.value.as_str()?.rsplit('.').next()? {
        "P" => Some(PidControllerType::P),
        "PI" => Some(PidControllerType::Pi),
        "PD" => Some(PidControllerType::Pd),
        "PID" => Some(PidControllerType::Pid),
        _ => None,
    }
}
pub(crate) fn clamp(x: f64, low: f64, high: f64) -> f64 {
    x.max(low).min(high)
}

impl Executor {
    pub fn execute(
        &mut self,
        block: &Block,
        inputs: &HashMap<String, Value>,
        dt: f64,
        path: &str,
    ) -> Result<HashMap<String, Value>, String> {
        fn pre_keys(block: &Block, path: &str, keys: &mut Vec<(String, bool)>) {
            let key = format!("{path}{}", block.name);
            if block.block_type.rsplit('.').next() == Some("Pre") && block.blocks.is_empty() {
                let start = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "pre_u_start")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false);
                keys.push((key.clone(), start));
            }
            for child in &block.blocks {
                pre_keys(child, &format!("{key}."), keys);
            }
        }
        let mut keys = Vec::new();
        pre_keys(block, path, &mut keys);
        if keys.is_empty() {
            return self.execute_once(block, inputs, dt, path);
        }
        let baseline = self.clone();
        let mut guess: Vec<bool> = keys
            .iter()
            .map(|(key, start)| *baseline.flags.get(key).unwrap_or(start))
            .collect();
        for _ in 0..32 {
            let mut trial = baseline.clone();
            for ((key, _), value) in keys.iter().zip(&guess) {
                trial.flags.insert(key.clone(), *value);
            }
            let outputs = trial.execute_once(block, inputs, dt, path)?;
            let next: Vec<bool> = keys
                .iter()
                .map(|(key, _)| *trial.flags.get(key).unwrap())
                .collect();
            if next == guess {
                *self = trial;
                return Ok(outputs);
            }
            guess = next;
        }
        Err("Boolean Pre event iteration did not converge".into())
    }

    fn execute_once(
        &mut self,
        block: &Block,
        inputs: &HashMap<String, Value>,
        dt: f64,
        path: &str,
    ) -> Result<HashMap<String, Value>, String> {
        let key = format!("{path}{}", block.name);
        if !block.blocks.is_empty()
            || (block.category.as_deref() == Some("composite") && !known_type(&block.block_type))
        {
            if !self.plans.contains_key(&key) {
                self.plans
                    .insert(key.clone(), Arc::new(Plan::compile(block)?));
            }
            let plan = Arc::clone(self.plans.get(&key).unwrap());
            let mut values: HashMap<(String, String), Value> = HashMap::new();
            for (name, value) in inputs {
                values.insert((name.clone(), String::new()), value.clone());
                values.insert((name.clone(), name.clone()), value.clone());
            }
            let mut sample_events = HashSet::new();
            for child in &block.blocks {
                if child.block_type.rsplit('.').next() == Some("UnitDelay")
                    && child.blocks.is_empty()
                {
                    let state_key = format!("{key}.{}", child.name);
                    let period = param(child, "samplePeriod", dt).max(0.001);
                    let initialized = self.sample_initialized.contains(&state_key);
                    let elapsed = self.sample_elapsed.entry(state_key.clone()).or_default();
                    if initialized {
                        *elapsed += dt;
                    }
                    let event = initialized && *elapsed + 1e-9 >= period;
                    if event {
                        *elapsed %= period;
                        let prior_input = *self
                            .sample_input
                            .get(&state_key)
                            .unwrap_or(&param(child, "y_start", 0.0));
                        self.state.insert(state_key.clone(), prior_input);
                        sample_events.insert(state_key.clone());
                    }
                    let value = *self
                        .state
                        .get(&state_key)
                        .unwrap_or(&param(child, "y_start", 0.0));
                    values.insert((child.name.clone(), "y".into()), json!(value));
                }
                if child.block_type.rsplit('.').next() == Some("Pre") && child.blocks.is_empty() {
                    let state_key = format!("{key}.{}", child.name);
                    let start = child
                        .parameters
                        .iter()
                        .find(|p| p.name == "pre_u_start")
                        .and_then(|p| p.value.as_bool())
                        .unwrap_or(false);
                    values.insert(
                        (child.name.clone(), "y".into()),
                        json!(*self.flags.get(&state_key).unwrap_or(&start)),
                    );
                }
            }
            for &idx in &plan.order {
                let child = &block.blocks[idx];
                if matches!(
                    child.block_type.rsplit('.').next(),
                    Some("UnitDelay" | "Pre")
                ) && child.blocks.is_empty()
                {
                    continue;
                }
                let mut child_inputs = HashMap::new();
                for (src, out, input) in &plan.incoming[idx] {
                    let value = values
                        .get(&(src.clone(), out.clone()))
                        .ok_or_else(|| format!("{key}: missing value {src}.{out}"))?;
                    child_inputs.insert(input.clone(), value.clone());
                }
                let result = self.execute_once(child, &child_inputs, dt, &format!("{key}."))?;
                for (port, value) in result {
                    values.insert((child.name.clone(), port), value);
                }
            }
            for (idx, child) in block.blocks.iter().enumerate() {
                if child.block_type.rsplit('.').next() != Some("UnitDelay")
                    || !child.blocks.is_empty()
                {
                    continue;
                }
                let (src, out, _) = plan.incoming[idx]
                    .iter()
                    .find(|(_, _, input)| input == "u")
                    .ok_or_else(|| format!("{key}: missing UnitDelay input {}.u", child.name))?;
                let input = values
                    .get(&(src.clone(), out.clone()))
                    .and_then(Value::as_f64)
                    .ok_or_else(|| format!("{key}: missing numeric value {src}.{out}"))?;
                let state_key = format!("{key}.{}", child.name);
                if sample_events.contains(&state_key)
                    || !self.sample_initialized.contains(&state_key)
                {
                    self.sample_input.insert(state_key.clone(), input);
                    self.sample_initialized.insert(state_key);
                }
            }
            for (idx, child) in block.blocks.iter().enumerate() {
                if child.block_type.rsplit('.').next() != Some("Pre") || !child.blocks.is_empty() {
                    continue;
                }
                let (src, out, _) = plan.incoming[idx]
                    .iter()
                    .find(|(_, _, input)| input == "u")
                    .ok_or_else(|| format!("{key}: missing Pre input {}.u", child.name))?;
                let value = values
                    .get(&(src.clone(), out.clone()))
                    .and_then(Value::as_bool)
                    .ok_or_else(|| format!("{key}: missing Boolean value {src}.{out}"))?;
                self.flags.insert(format!("{key}.{}", child.name), value);
            }
            let mut outputs = HashMap::new();
            for (src, out, dst) in &plan.outputs {
                let value = values
                    .get(&(src.clone(), out.clone()))
                    .ok_or_else(|| format!("{key}: missing value {src}.{out}"))?;
                outputs.insert(dst.clone(), value.clone());
            }
            if !self.skip_traces && !self.trace_targets.is_empty() {
                let prefix = format!("{key}|");
                for target in &self.trace_targets {
                    if let Some(point) = target.strip_prefix(&prefix) {
                        let value = if let Some(port) = point.strip_prefix("output.") {
                            outputs.get(port).cloned()
                        } else if let Some(index) = point
                            .strip_prefix("edge.")
                            .and_then(|s| s.parse::<usize>().ok())
                        {
                            block.connections.get(index).and_then(|edge| {
                                let (src, out) = edge.source();
                                values.get(&(src, out)).cloned()
                            })
                        } else {
                            let (src, out) = split_path(point);
                            values
                                .get(&(src.clone(), out.clone()))
                                .or_else(|| values.get(&(src, String::new())))
                                .cloned()
                        };
                        if let Some(value) = value {
                            self.trace_values.insert(target.clone(), value);
                        }
                    }
                }
            }
            return Ok(outputs);
        }
        let kind = block
            .block_type
            .rsplit('.')
            .next()
            .unwrap_or(&block.block_type);
        let mut out = HashMap::new();
        let u = || get(inputs, "u");
        let y: Value = match kind {
            "Constant" => {
                if block
                    .outputs
                    .first()
                    .is_some_and(|port| port.kind == "Boolean")
                {
                    json!(block
                        .parameters
                        .iter()
                        .find(|parameter| parameter.name == "k")
                        .and_then(|parameter| parameter.value.as_bool())
                        .unwrap_or(false))
                } else {
                    json!(param(block, "k", 0.0))
                }
            }
            "SampleTrigger" => {
                let period = param(block, "period", 1.0);
                let shift = param(block, "shift", 0.0);
                if !period.is_finite() || period <= 0.0 || !shift.is_finite() {
                    return Err("SampleTrigger requires a positive period and finite shift".into());
                }
                let shift = shift.rem_euclid(period);
                let clock = self.state.entry(format!("{key}.clock")).or_default();
                let before = *clock;
                let initialized = self
                    .flags
                    .insert(format!("{key}.initialized"), true)
                    .is_some();
                if initialized {
                    *clock += dt;
                }
                let crossed = initialized
                    && ((before - shift + 1e-9) / period).floor()
                        < ((*clock - shift + 1e-9) / period).floor();
                json!((!initialized && shift <= 1e-9) || crossed)
            }
            "Add" => json!(
                param(block, "k1", 1.0) * get(inputs, "u1")?
                    + param(block, "k2", 1.0) * get(inputs, "u2")?
            ),
            "AddParameter" => json!(u()? + param(block, "p", 0.0)),
            "Min" => json!(get(inputs, "u1")?.min(get(inputs, "u2")?)),
            "Max" => json!(get(inputs, "u1")?.max(get(inputs, "u2")?)),
            "Line" => {
                let x1 = get(inputs, "x1")?;
                let x2 = get(inputs, "x2")?;
                if x2 <= x1 {
                    return Err(format!("{key}: Line requires x2 > x1"));
                }
                let mut x = u()?;
                if bool_param(block, "limitBelow", true) {
                    x = x.max(x1);
                }
                if bool_param(block, "limitAbove", true) {
                    x = x.min(x2);
                }
                let f1 = get(inputs, "f1")?;
                let f2 = get(inputs, "f2")?;
                json!(f1 + (f2 - f1) * (x - x1) / (x2 - x1))
            }
            "BooleanToInteger" => json!(if inputs
                .get("u")
                .and_then(Value::as_bool)
                .ok_or("missing Boolean input u")?
            {
                param(block, "integerTrue", 1.0) as i64
            } else {
                param(block, "integerFalse", 0.0) as i64
            }),
            "RealToInteger" => json!(u()?.round() as i64),
            "BooleanToReal" => json!(if inputs
                .get("u")
                .and_then(Value::as_bool)
                .ok_or("missing Boolean input u")?
            {
                param(block, "realTrue", 1.0)
            } else {
                param(block, "realFalse", 0.0)
            }),
            "IntegerToReal" => json!(u()?),
            "UnitDelay" => {
                let period = param(block, "samplePeriod", dt).max(0.001);
                let initialized = self.sample_initialized.contains(&key);
                let elapsed = self.sample_elapsed.entry(key.clone()).or_default();
                if initialized {
                    *elapsed += dt;
                }
                let event = initialized && *elapsed + 1e-9 >= period;
                if event {
                    *elapsed %= period;
                    let prior_input = *self
                        .sample_input
                        .get(&key)
                        .unwrap_or(&param(block, "y_start", 0.0));
                    self.state.insert(key.clone(), prior_input);
                }
                if event || !self.sample_initialized.contains(&key) {
                    self.sample_input.insert(key.clone(), u()?);
                    self.sample_initialized.insert(key.clone());
                }
                let previous = *self
                    .state
                    .get(&key)
                    .unwrap_or(&param(block, "y_start", 0.0));
                json!(previous)
            }
            "Sampler" => {
                let period = param(block, "samplePeriod", dt).max(0.001);
                let initialized = self.sample_initialized.contains(&key);
                let elapsed = self.sample_elapsed.entry(key.clone()).or_default();
                if initialized {
                    *elapsed += dt;
                }
                let event = initialized && *elapsed + 1e-9 >= period;
                if event {
                    *elapsed %= period;
                }
                if event || !self.sample_initialized.contains(&key) {
                    self.state.insert(key.clone(), u()?);
                    self.sample_initialized.insert(key.clone());
                }
                json!(*self.state.get(&key).unwrap())
            }
            "Pre" => {
                let input = inputs
                    .get("u")
                    .and_then(Value::as_bool)
                    .ok_or("missing Boolean input u")?;
                let start = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "pre_u_start")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false);
                let previous = *self.flags.get(&key).unwrap_or(&start);
                self.flags.insert(key.clone(), input);
                json!(previous)
            }
            "TriggeredSampler" => {
                let trigger = inputs
                    .get("trigger")
                    .and_then(Value::as_bool)
                    .ok_or("missing Boolean input trigger")?;
                let prior = self.flags.insert(key.clone(), trigger).unwrap_or(false);
                if trigger && !prior {
                    self.state.insert(key.clone(), u()?);
                }
                json!(self
                    .state
                    .get(&key)
                    .copied()
                    .unwrap_or(param(block, "y_start", 0.0)))
            }
            "TriggeredMovingMean" => {
                let trigger = inputs
                    .get("trigger")
                    .and_then(Value::as_bool)
                    .ok_or("missing Boolean input trigger")?;
                let prior = self.flags.insert(key.clone(), trigger);
                let samples = self.moving_samples.entry(key.clone()).or_default();
                if prior.is_none() || (trigger && prior == Some(false)) {
                    samples.push_back(u()?);
                    let capacity = (param(block, "n", 1.0) as usize).max(1);
                    if samples.len() > capacity {
                        samples.pop_front();
                    }
                }
                json!(samples.iter().sum::<f64>() / samples.len() as f64)
            }
            "Edge" | "FallingEdge" => {
                let active = inputs
                    .get("u")
                    .and_then(Value::as_bool)
                    .ok_or("missing Boolean input u")?;
                let start = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "pre_u_start")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false);
                let prior = self.flags.insert(key.clone(), active).unwrap_or(start);
                json!(if kind == "Edge" {
                    active && !prior
                } else {
                    !active && prior
                })
            }
            "Timer" => {
                let active = inputs
                    .get("u")
                    .and_then(Value::as_bool)
                    .ok_or("missing Boolean input u")?;
                let prior = self.flags.insert(key.clone(), active).unwrap_or(false);
                let elapsed = if !active {
                    0.0
                } else if !prior {
                    0.0
                } else {
                    self.state.get(&key).copied().unwrap_or(0.0) + dt
                };
                self.state.insert(key.clone(), elapsed);
                out.insert(
                    "passed".into(),
                    json!(active && elapsed >= param(block, "t", 0.0)),
                );
                json!(elapsed)
            }
            "MultiplyByParameter" | "Gain" => json!(param(block, "k", 1.0) * u()?),
            "Limiter" => json!(clamp(
                u()?,
                param(block, "uMin", param(block, "yMin", 0.0)),
                param(block, "uMax", param(block, "yMax", 1.0))
            )),
            "Filter" => {
                let x = u()?;
                let old = *self.state.entry(key.clone()).or_insert(x);
                let y = old + (1.0 - (-dt / param(block, "T", 10.0).max(0.001)).exp()) * (x - old);
                self.state.insert(key.clone(), y);
                json!(y)
            }
            "LimRateLimiter" => {
                let x = u()?;
                let old = *self.state.entry(key.clone()).or_insert(x);
                let y = clamp(
                    x,
                    old - param(block, "fallRate", 0.1) * dt,
                    old + param(block, "riseRate", 0.1) * dt,
                );
                self.state.insert(key.clone(), y);
                json!(y)
            }
            "Hysteresis" => {
                let x = u()?;
                let start = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "pre_y_start")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false);
                let old = *self.flags.get(&key).unwrap_or(&start);
                let y = if x > param(block, "uHigh", 1.0) {
                    true
                } else if x < param(block, "uLow", -1.0) {
                    false
                } else {
                    old
                };
                self.flags.insert(key.clone(), y);
                json!(y)
            }
            "Switch" => {
                let standard_ports = block
                    .inputs
                    .iter()
                    .any(|p| p.name == "u2" && p.kind == "Boolean");
                let selector = if standard_ports { "u2" } else { "u3" };
                let choice = inputs
                    .get(selector)
                    .and_then(Value::as_bool)
                    .ok_or_else(|| format!("missing Boolean input {selector}"))?;
                if choice {
                    inputs.get("u1").cloned().ok_or("missing input u1")?
                } else {
                    let other = if standard_ports { "u3" } else { "u2" };
                    inputs
                        .get(other)
                        .cloned()
                        .ok_or_else(|| format!("missing input {other}"))?
                }
            }
            "LimPID" => {
                let reverse = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "reverseActing")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false);
                let e =
                    (get(inputs, "u_s")? - get(inputs, "u_m")?) * if reverse { 1.0 } else { -1.0 };
                let k = param(block, "k", 0.5);
                let low = param(block, "yMin", 0.0);
                let high = param(block, "yMax", 1.0);
                let i = *self.state.get(&key).unwrap_or(&0.0);
                let proposal = i + k * e * dt / param(block, "Ti", 60.0).max(0.001);
                let bounded = clamp(proposal, low - k * e, high - k * e);
                self.state.insert(key.clone(), bounded);
                json!(clamp(k * e + bounded, low, high))
            }
            "PIDWithReset" => {
                let trigger = inputs
                    .get("trigger")
                    .and_then(Value::as_bool)
                    .ok_or("missing Boolean input trigger")?;
                let reverse = bool_param(block, "reverseActing", true);
                let scale = param(block, "r", 1.0).max(1e-12);
                let e = (get(inputs, "u_s")? - get(inputs, "u_m")?)
                    * if reverse { 1.0 } else { -1.0 }
                    / scale;
                let gain = param(block, "k", 1.0);
                let controller_type = pid_controller_type(block).ok_or("invalid controllerType")?;
                let with_integral = controller_type.with_integral();
                let with_derivative = controller_type.with_derivative();
                let low = param(block, "yMin", 0.0);
                let high = param(block, "yMax", 1.0);
                let proportional = gain * e;
                let derivative = if with_derivative {
                    let td = param(block, "Td", 0.1);
                    let filter_time = td / param(block, "Nd", 10.0);
                    let derivative_gain = gain * td;
                    let derivative_key = format!("{key}.derivative_x");
                    let previous_x = *self.state.get(&derivative_key).unwrap_or(
                        &(e - filter_time * param(block, "yd_start", 0.0) / derivative_gain),
                    );
                    let next_x = e + (previous_x - e) * (-dt / filter_time).exp();
                    self.state.insert(derivative_key, next_x);
                    derivative_gain / filter_time * (e - next_x)
                } else {
                    0.0
                };
                let feedforward = proportional + derivative;
                // Modelica edge(trigger) is false at initialization even when
                // trigger starts true. Reset only after an observed false-to-true
                // transition, as in the pinned OBC Common damper validation.
                let previous_trigger = *self.flags.get(&key).unwrap_or(&trigger);
                self.flags.insert(key.clone(), trigger);
                let integral = if !with_integral {
                    0.0
                } else if trigger && !previous_trigger {
                    param(block, "y_reset", param(block, "xi_start", 0.0)) - feedforward
                } else {
                    let old = *self
                        .state
                        .get(&key)
                        .unwrap_or(&param(block, "xi_start", 0.0));
                    let unsaturated = feedforward + old;
                    let saturated = clamp(unsaturated, low, high);
                    let antiwindup =
                        (unsaturated - saturated) / (gain * param(block, "Ni", 0.9)).max(1e-12);
                    old + gain * dt / param(block, "Ti", 0.5).max(1e-12) * (e - antiwindup)
                };
                if with_integral {
                    self.state.insert(key.clone(), integral);
                }
                json!(clamp(feedforward + integral, low, high))
            }
            "Greater" => {
                let a = get(inputs, "u1")?;
                let b = get(inputs, "u2")?;
                let h = param(block, "h", 0.0);
                let previous = *self.flags.get(&key).unwrap_or(
                    &block
                        .parameters
                        .iter()
                        .find(|p| p.name == "pre_y_start")
                        .and_then(|p| p.value.as_bool())
                        .unwrap_or(false),
                );
                let next = a > b - if h >= 1e-10 && previous { h } else { 0.0 };
                self.flags.insert(key.clone(), next);
                json!(next)
            }
            "GreaterThreshold" => {
                let value = u()?;
                let threshold = param(block, "t", 0.0);
                let hysteresis = param(block, "h", 0.0);
                let old = *self.flags.get(&key).unwrap_or(
                    &block
                        .parameters
                        .iter()
                        .find(|p| p.name == "pre_y_start")
                        .and_then(|p| p.value.as_bool())
                        .unwrap_or(false),
                );
                let next = if hysteresis >= 1e-10 {
                    if old {
                        value > threshold - hysteresis
                    } else {
                        value > threshold
                    }
                } else {
                    value > threshold
                };
                self.flags.insert(key.clone(), next);
                json!(next)
            }
            "LessThreshold" => {
                let value = u()?;
                let threshold = param(block, "t", 0.0);
                let hysteresis = param(block, "h", 0.0);
                let old = *self.flags.get(&key).unwrap_or(
                    &block
                        .parameters
                        .iter()
                        .find(|p| p.name == "pre_y_start")
                        .and_then(|p| p.value.as_bool())
                        .unwrap_or(false),
                );
                let next = if hysteresis >= 1e-10 {
                    if old {
                        value < threshold + hysteresis
                    } else {
                        value < threshold
                    }
                } else {
                    value < threshold
                };
                self.flags.insert(key.clone(), next);
                json!(next)
            }
            "Equal" => json!(get(inputs, "u1")? == get(inputs, "u2")?),
            "Less" => {
                let a = get(inputs, "u1")?;
                let b = get(inputs, "u2")?;
                let h = param(block, "h", 0.0);
                let previous = *self.flags.get(&key).unwrap_or(
                    &block
                        .parameters
                        .iter()
                        .find(|p| p.name == "pre_y_start")
                        .and_then(|p| p.value.as_bool())
                        .unwrap_or(false),
                );
                let next = a < b + if h >= 1e-10 && previous { h } else { 0.0 };
                self.flags.insert(key.clone(), next);
                json!(next)
            }
            "Subtract" => json!(get(inputs, "u1")? - get(inputs, "u2")?),
            "Multiply" => json!(get(inputs, "u1")? * get(inputs, "u2")?),
            "Divide" => json!(get(inputs, "u1")? / get(inputs, "u2")?),
            "And" => json!(
                inputs.get("u1").and_then(Value::as_bool).unwrap_or(false)
                    && inputs.get("u2").and_then(Value::as_bool).unwrap_or(false)
            ),
            "Or" => json!(
                inputs.get("u1").and_then(Value::as_bool).unwrap_or(false)
                    || inputs.get("u2").and_then(Value::as_bool).unwrap_or(false)
            ),
            "Not" => json!(!inputs.get("u").and_then(Value::as_bool).unwrap_or(false)),
            "Delay" => {
                let old = *self.state.entry(key.clone()).or_insert(u()?);
                self.state.insert(key.clone(), u()?);
                json!(old)
            }
            "TrueDelay" => {
                let active = inputs
                    .get("u")
                    .and_then(Value::as_bool)
                    .ok_or("missing Boolean input u")?;
                let delay = param(block, "delayTime", 0.0).max(0.0);
                let previous = self.flags.get(&format!("{key}.previous_input")).copied();
                let delay_on_init = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "delayOnInit")
                    .and_then(|p| p.value.as_bool())
                    .unwrap_or(false);
                let elapsed = if active {
                    if previous.is_none() && !delay_on_init {
                        delay
                    } else if previous == Some(true) {
                        self.state.get(&key).copied().unwrap_or(0.0) + dt
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                self.flags.insert(format!("{key}.previous_input"), active);
                self.state.insert(key.clone(), elapsed);
                json!(active && elapsed >= delay)
            }
            "TrueFalseHold" => {
                let active = inputs
                    .get("u")
                    .and_then(Value::as_bool)
                    .ok_or("missing Boolean input u")?;
                let true_duration = param(block, "trueHoldDuration", 0.0);
                let false_duration = param(block, "falseHoldDuration", true_duration);
                let initialized = self
                    .flags
                    .get(&format!("{key}.initialized"))
                    .copied()
                    .unwrap_or(false);
                let mut held = self.flags.get(&key).copied().unwrap_or(active);
                let mut age = self.state.get(&key).copied().unwrap_or(0.0);
                if initialized {
                    let previous_age = age;
                    age += dt;
                    let minimum = if held { true_duration } else { false_duration };
                    if active != held && age >= minimum {
                        let input_changed =
                            self.flags.get(&format!("{key}.previous_input")).copied()
                                != Some(active);
                        age = if input_changed && previous_age >= minimum {
                            0.0
                        } else {
                            age - minimum
                        };
                        held = active;
                    }
                }
                self.flags.insert(format!("{key}.initialized"), true);
                self.flags.insert(format!("{key}.previous_input"), active);
                self.flags.insert(key.clone(), held);
                self.state.insert(key.clone(), age);
                json!(held)
            }
            "Latch" => {
                let set = inputs
                    .get("u")
                    .and_then(Value::as_bool)
                    .ok_or("missing Boolean input u")?;
                let clear = inputs
                    .get("clr")
                    .and_then(Value::as_bool)
                    .ok_or("missing Boolean input clr")?;
                let previous_set = self.flags.get(&format!("{key}.previous_set")).copied();
                let previous_clear = self.flags.get(&format!("{key}.previous_clear")).copied();
                let mut latched = self.flags.get(&key).copied().unwrap_or(set && !clear);
                if previous_set.is_some()
                    && ((!previous_set.unwrap() && set) || (!previous_clear.unwrap() && clear))
                {
                    latched = set && !clear;
                }
                self.flags.insert(format!("{key}.previous_set"), set);
                self.flags.insert(format!("{key}.previous_clear"), clear);
                self.flags.insert(key.clone(), latched);
                json!(latched)
            }
            _ => {
                if block.equations.is_empty() {
                    return Err(format!(
                        "{key}: unsupported block type {}",
                        block.block_type
                    ));
                }
                let mut vars = HashMap::<String, f64>::new();
                for p in &block.parameters {
                    vars.insert(p.name.clone(), num(&p.value));
                }
                for (name, value) in inputs {
                    if let Some(n) = value.as_f64() {
                        vars.insert(name.clone(), n);
                    }
                }
                for eq in &block.equations {
                    let expr = eq.rhs.parse::<meval::Expr>().map_err(|e| e.to_string())?;
                    let mut ctx = meval::Context::new();
                    for (name, value) in &vars {
                        ctx.var(name, *value);
                    }
                    ctx.func2("min", f64::min);
                    ctx.func2("max", f64::max);
                    let value = expr
                        .eval_with_context(ctx)
                        .map_err(|e| format!("{key}: {e}"))?;
                    vars.insert(eq.lhs.clone(), value);
                    if block.outputs.iter().any(|p| p.name == eq.lhs) {
                        out.insert(eq.lhs.clone(), json!(value));
                    }
                }
                return Ok(out);
            }
        };
        let y = if block
            .outputs
            .iter()
            .any(|port| port.name == "y" && port.kind == "Integer")
        {
            let number = y
                .as_f64()
                .or_else(|| y.as_i64().map(|n| n as f64))
                .ok_or_else(|| format!("{key}: nonnumeric Integer output"))?;
            if !number.is_finite()
                || number.fract() != 0.0
                || number < i64::MIN as f64
                || number >= i64::MAX as f64
            {
                return Err(format!("{key}: invalid Integer output {number}"));
            }
            json!(number as i64)
        } else {
            y
        };
        out.insert("y".into(), y);
        Ok(out)
    }
}
