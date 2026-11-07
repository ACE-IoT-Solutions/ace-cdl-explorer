use arrow_array::{ArrayRef, Float64Array, RecordBatch, UInt32Array};
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{DataType, Field, Schema};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

mod compiled;
pub use compiled::CompiledExecutor;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Connector {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Parameter {
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Equation {
    pub lhs: String,
    pub rhs: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Connection {
    #[serde(default)]
    pub from_block: String,
    #[serde(default)]
    pub from_output: String,
    #[serde(default)]
    pub to_block: String,
    #[serde(default)]
    pub to_input: String,
    #[serde(default, rename = "from")]
    pub from_path: String,
    #[serde(default, rename = "to")]
    pub to_path: String,
}

impl Connection {
    fn source(&self) -> (String, String) {
        if self.from_path.is_empty() {
            (self.from_block.clone(), self.from_output.clone())
        } else {
            split_path(&self.from_path)
        }
    }
    fn target(&self) -> (String, String) {
        if self.to_path.is_empty() {
            (self.to_block.clone(), self.to_input.clone())
        } else {
            split_path(&self.to_path)
        }
    }
}

fn split_path(path: &str) -> (String, String) {
    match path.split_once('.') {
        Some((a, b)) => (a.into(), b.into()),
        None => (path.into(), String::new()),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Block {
    pub name: String,
    #[serde(alias = "type")]
    pub block_type: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub inputs: Vec<Connector>,
    #[serde(default)]
    pub outputs: Vec<Connector>,
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    #[serde(default)]
    pub equations: Vec<Equation>,
    #[serde(default)]
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub connections: Vec<Connection>,
}

fn known_type(kind: &str) -> bool {
    const NAMES: &[&str] = &[
        "Constant",
        "Add",
        "Min",
        "Max",
        "BooleanToInteger",
        "MultiplyByParameter",
        "Limiter",
        "LimPID",
        "Filter",
        "LimRateLimiter",
        "Hysteresis",
        "Switch",
        "VAVBoxController",
        "Gain",
        "Greater",
        "Less",
        "Subtract",
        "Multiply",
        "Divide",
        "And",
        "Or",
        "Not",
        "Delay",
        "TrueDelay",
        "Latch",
        "UnitDelay",
        "TriggeredSampler",
        "TriggeredMovingMean",
        "Edge",
        "FallingEdge",
        "Timer",
        "IntegerToReal",
    ];
    NAMES
        .iter()
        .any(|n| kind == *n || kind.ends_with(&format!(".{n}")))
}

fn validate_block(block: &Block, path: &str, errors: &mut Vec<String>) {
    let at = format!("{path}{}", block.name);
    if block.name.trim().is_empty() {
        errors.push(format!("{at}: empty block name"));
    }
    if block.blocks.is_empty()
        && (block.category.as_deref() != Some("composite") || known_type(&block.block_type))
    {
        if block.equations.is_empty() && !known_type(&block.block_type) {
            errors.push(format!("{at}: unsupported block type {}", block.block_type));
        }
        if known_type(&block.block_type) {
            for parameter in &block.parameters {
                if parameter.value.is_string() || parameter.value.is_null() {
                    errors.push(format!(
                        "{at}: unsupported parameter value {} = {}",
                        parameter.name, parameter.value
                    ));
                }
            }
            if block.block_type.rsplit('.').next() == Some("UnitDelay")
                && !block.parameters.iter().any(|parameter| {
                    parameter.name == "samplePeriod"
                        && parameter
                            .value
                            .as_f64()
                            .is_some_and(|value| value.is_finite() && value > 0.0)
                })
            {
                errors.push(format!("{at}: UnitDelay requires a positive samplePeriod"));
            }
            if block.block_type.rsplit('.').next() == Some("TriggeredMovingMean")
                && !block.parameters.iter().any(|parameter| {
                    parameter.name == "n" && parameter.value.as_u64().is_some_and(|value| value > 0)
                })
            {
                errors.push(format!(
                    "{at}: TriggeredMovingMean requires a positive integer n"
                ));
            }
            if block.block_type.rsplit('.').next() == Some("Timer")
                && block.parameters.iter().any(|parameter| {
                    parameter.name == "t"
                        && !parameter
                            .value
                            .as_f64()
                            .is_some_and(|value| value.is_finite() && value >= 0.0)
                })
            {
                errors.push(format!("{at}: Timer requires a nonnegative finite t"));
            }
        }
        for eq in &block.equations {
            if eq.rhs.parse::<meval::Expr>().is_err() {
                errors.push(format!(
                    "{at}: unsupported equation {} = {}",
                    eq.lhs, eq.rhs
                ));
            }
        }
        return;
    }
    let mut names = HashSet::new();
    for child in &block.blocks {
        if !names.insert(child.name.as_str()) {
            errors.push(format!("{at}: duplicate child {}", child.name));
        }
        validate_block(child, &format!("{at}."), errors);
    }
    let mut drivers = HashSet::new();
    for edge in &block.connections {
        let (src, out) = edge.source();
        let (dst, input) = edge.target();
        let source = block.blocks.iter().find(|c| c.name == src);
        let target = block.blocks.iter().find(|c| c.name == dst);
        let source_port = source.and_then(|c| c.outputs.iter().find(|p| p.name == out));
        let target_port = target.and_then(|c| c.inputs.iter().find(|p| p.name == input));
        let parent_input = block
            .inputs
            .iter()
            .find(|p| p.name == src && (out.is_empty() || out == src));
        let parent_output = block
            .outputs
            .iter()
            .find(|p| p.name == dst && (input.is_empty() || input == dst));
        if source_port.is_none() && parent_input.is_none() {
            errors.push(format!("{at}: missing source {src}.{out}"));
        }
        if target_port.is_none() && parent_output.is_none() {
            errors.push(format!("{at}: missing target {dst}.{input}"));
        }
        if let (Some(a), Some(b)) = (source_port.or(parent_input), target_port.or(parent_output)) {
            if a.kind != b.kind {
                errors.push(format!("{at}: type mismatch {src}.{out} -> {dst}.{input}"));
            }
        }
        if !drivers.insert((dst.clone(), input.clone())) {
            errors.push(format!("{at}: multiple drivers for {dst}.{input}"));
        }
    }
    for child in &block.blocks {
        for port in &child.inputs {
            if !drivers.contains(&(child.name.clone(), port.name.clone())) {
                errors.push(format!("{at}: missing input {}.{}", child.name, port.name));
            }
        }
    }
    if order(block).is_err() {
        errors.push(format!("{at}: connection cycle"));
    }
}

fn order(block: &Block) -> Result<Vec<usize>, String> {
    let mut done = HashSet::new();
    let mut result = Vec::new();
    while result.len() < block.blocks.len() {
        let next = block.blocks.iter().enumerate().find(|(_, child)| {
            !done.contains(&child.name)
                && block.connections.iter().all(|e| {
                    let (src, _) = e.source();
                    let (dst, _) = e.target();
                    dst != child.name
                        || child.block_type.rsplit('.').next() == Some("UnitDelay")
                        || src == child.name
                        || !block.blocks.iter().any(|b| b.name == src)
                        || done.contains(&src)
                })
        });
        let Some((idx, child)) = next else {
            return Err("connection cycle".into());
        };
        done.insert(child.name.clone());
        result.push(idx);
    }
    Ok(result)
}

pub fn validate(block: &Block) -> Vec<String> {
    let mut errors = Vec::new();
    validate_block(block, "", &mut errors);
    errors
}

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

#[derive(Default)]
pub struct Executor {
    state: HashMap<String, f64>,
    sample_elapsed: HashMap<String, f64>,
    sample_input: HashMap<String, f64>,
    sample_initialized: HashSet<String>,
    flags: HashMap<String, bool>,
    moving_samples: HashMap<String, VecDeque<f64>>,
    plans: HashMap<String, Arc<Plan>>,
    trace_targets: Vec<String>,
    trace_values: HashMap<String, Value>,
    skip_traces: bool,
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
fn param(block: &Block, key: &str, default: f64) -> f64 {
    block
        .parameters
        .iter()
        .find(|p| p.name == key)
        .map(|p| num(&p.value))
        .unwrap_or(default)
}
fn clamp(x: f64, low: f64, high: f64) -> f64 {
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
                    let elapsed = self.sample_elapsed.entry(state_key.clone()).or_default();
                    *elapsed += dt;
                    let event = *elapsed + 1e-9 >= period;
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
            }
            for &idx in &plan.order {
                let child = &block.blocks[idx];
                if child.block_type.rsplit('.').next() == Some("UnitDelay")
                    && child.blocks.is_empty()
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
                let result = self.execute(child, &child_inputs, dt, &format!("{key}."))?;
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
            "Constant" => json!(param(block, "k", 0.0)),
            "Add" => json!(
                param(block, "k1", 1.0) * get(inputs, "u1")?
                    + param(block, "k2", 1.0) * get(inputs, "u2")?
            ),
            "Min" => json!(get(inputs, "u1")?.min(get(inputs, "u2")?)),
            "Max" => json!(get(inputs, "u1")?.max(get(inputs, "u2")?)),
            "BooleanToInteger" => json!(if inputs
                .get("u")
                .and_then(Value::as_bool)
                .ok_or("missing Boolean input u")?
            {
                1
            } else {
                0
            }),
            "IntegerToReal" => json!(u()?),
            "UnitDelay" => {
                let period = param(block, "samplePeriod", dt).max(0.001);
                let elapsed = self.sample_elapsed.entry(key.clone()).or_default();
                *elapsed += dt;
                let event = *elapsed + 1e-9 >= period;
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
            "Greater" => json!(get(inputs, "u1")? > get(inputs, "u2")?),
            "Less" => json!(get(inputs, "u1")? < get(inputs, "u2")?),
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
            "VAVBoxController" => {
                let room = get(inputs, "room_temp")?;
                let cool = param(block, "cooling_setpoint", 24.0);
                let heat = param(block, "heating_setpoint", 21.0);
                let min_d = param(block, "min_damper_position", 0.15);
                let cool_err = (room - cool).max(0.0);
                let heat_err = (heat - room).max(0.0);
                let ci_key = format!("{key}.cool_i");
                let hi_key = format!("{key}.heat_i");
                let (damper, reheat) = if cool_err > 0.5 {
                    let i = clamp(
                        self.state.get(&ci_key).copied().unwrap_or(0.0)
                            + param(block, "ki_damper", 0.12) * cool_err * dt / 900.0,
                        0.0,
                        1.0 - min_d,
                    );
                    self.state.insert(ci_key, i);
                    self.state.insert(hi_key, 0.0);
                    (
                        clamp(
                            min_d + param(block, "kp_damper", 0.6) * cool_err + i,
                            min_d,
                            1.0,
                        ),
                        0.0,
                    )
                } else if heat_err > 0.0 {
                    let i = clamp(
                        self.state.get(&hi_key).copied().unwrap_or(0.0)
                            + param(block, "ki_reheat", 0.06) * heat_err * dt / 900.0,
                        0.0,
                        1.0,
                    );
                    self.state.insert(hi_key, i);
                    self.state.insert(ci_key, 0.0);
                    (
                        min_d,
                        clamp(param(block, "kp_reheat", 0.35) * heat_err + i, 0.0, 1.0),
                    )
                } else {
                    self.state.insert(ci_key, 0.0);
                    self.state.insert(hi_key, 0.0);
                    (min_d, 0.0)
                };
                let flow = param(block, "min_airflow", 0.2)
                    + (param(block, "max_airflow", 0.95) - param(block, "min_airflow", 0.2))
                        * damper
                        * get(inputs, "supply_pressure").unwrap_or(1.0);
                out.insert("damper_position".into(), json!(damper));
                out.insert("reheat_valve_position".into(), json!(reheat));
                out.insert("airflow".into(), json!(flow));
                out.insert(
                    "cooling_demand".into(),
                    json!(clamp(cool_err / 2.0, 0.0, 1.0)),
                );
                out.insert(
                    "heating_demand".into(),
                    json!(clamp(heat_err / 2.0, 0.0, 1.0)),
                );
                return Ok(out);
            }
            _ => {
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Weather {
    pub outdoor_c: f64,
    pub solar_kw: f64,
    pub internal_kw: f64,
    pub occupied: bool,
}

const MEASURE_COLUMNS: &[&str] = &[
    "schedule_occupied",
    "preconditioning_mode",
    "outdoor_c",
    "avg_zone_c",
    "cool_kw",
    "heat_kw",
    "cool_request_kw",
    "heat_request_kw",
    "electric_kw",
    "gas_kw",
    "requested_airflow",
    "delivered_airflow",
    "ahu_1_supply_actual_c",
    "ahu_2_supply_actual_c",
    "ahu_1_supply_setpoint_c",
    "ahu_2_supply_setpoint_c",
    "ahu_1_duct_pressure",
    "ahu_2_duct_pressure",
    "ahu_1_outdoor_air_fraction",
    "ahu_2_outdoor_air_fraction",
    "ahu_1_intake_opening",
    "ahu_2_intake_opening",
    "ahu_1_heating_coil_valve_position",
    "ahu_2_heating_coil_valve_position",
    "ahu_1_cooling_coil_valve_position",
    "ahu_2_cooling_coil_valve_position",
    "ahu_1_heating_coil_request_kw",
    "ahu_2_heating_coil_request_kw",
    "ahu_1_cooling_coil_request_kw",
    "ahu_2_cooling_coil_request_kw",
    "ahu_1_heating_coil_delivered_kw",
    "ahu_2_heating_coil_delivered_kw",
    "ahu_1_cooling_coil_delivered_kw",
    "ahu_2_cooling_coil_delivered_kw",
    "ahu_1_outdoor_airflow",
    "ahu_2_outdoor_airflow",
    "ahu_1_mixed_air_c",
    "ahu_2_mixed_air_c",
    "heat_pump_heating_used_kw",
    "heat_pump_cooling_used_kw",
    "heat_pump_electric_kw",
    "heat_pump_ambient_exchange_kw",
    "heat_pump_available_capacity_kw",
    "heat_pump_heating_cop",
    "heat_pump_cooling_cop",
    "heat_pump_available",
    "boiler_available",
    "chiller_available",
    "heat_pump_running",
    "plant_heat_pump_start_fault",
    "plant_boiler_start_fault",
    "plant_chiller_start_fault",
    "boiler_running",
    "chiller_running",
    "boiler_used_kw",
    "chiller_used_kw",
    "hot_water_supply_temp_c",
    "hot_water_supply_setpoint_c",
    "hot_water_return_temp_c",
    "chilled_water_supply_temp_c",
    "chilled_water_supply_setpoint_c",
    "chilled_water_return_temp_c",
    "hot_water_flow_kg_s",
    "chilled_water_flow_kg_s",
    "hot_water_differential_pressure_pa",
    "chilled_water_differential_pressure_pa",
    "hot_water_pump_speed",
    "chilled_water_pump_speed",
    "hot_water_enable",
    "chilled_water_enable",
    "water_pump_kw",
];

fn arrow_batch(rows: &[Value], targets: &[String]) -> Result<Vec<u8>, String> {
    let mut fields = vec![Field::new("index", DataType::UInt32, false)];
    let mut arrays: Vec<ArrayRef> = vec![Arc::new(UInt32Array::from(
        rows.iter()
            .map(|row| {
                row["index"]
                    .as_u64()
                    .and_then(|index| u32::try_from(index).ok())
                    .ok_or("Invalid Arrow row index".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?,
    ))];
    for name in MEASURE_COLUMNS
        .iter()
        .map(|name| name.to_string())
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_c")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_occupied")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_presence_raw")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_airflow")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_discharge_c")))
        .chain(targets.iter().map(|target| format!("trace:{target}")))
    {
        fields.push(Field::new(&name, DataType::Float64, false));
        let values = rows
            .iter()
            .map(|row| {
                let value = if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_airflow"))
                {
                    row["zone_airflow"][zone.parse::<usize>().expect("Known zone column") - 1]
                        .as_f64()
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_discharge_c"))
                {
                    row["zone_discharge_c"][zone.parse::<usize>().expect("Known zone column") - 1]
                        .as_f64()
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_c"))
                {
                    row["zone_c"][zone.parse::<usize>().expect("Known zone column") - 1].as_f64()
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_occupied"))
                {
                    row["zone_occupied"][zone.parse::<usize>().expect("Known zone column") - 1]
                        .as_f64()
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_presence_raw"))
                {
                    row["zone_presence_raw"][zone.parse::<usize>().expect("Known zone column") - 1]
                        .as_f64()
                } else if let Some(target) = name.strip_prefix("trace:") {
                    row["traces"][target].as_f64()
                } else {
                    row[&name].as_f64()
                };
                value.ok_or_else(|| format!("Missing numeric Arrow column {name}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        arrays.push(Arc::new(Float64Array::from(values)));
    }
    let schema = Arc::new(Schema::new(fields));
    let batch = RecordBatch::try_new(schema.clone(), arrays).map_err(|e| e.to_string())?;
    let mut writer = StreamWriter::try_new(Vec::new(), &schema).map_err(|e| e.to_string())?;
    writer.write(&batch).map_err(|e| e.to_string())?;
    writer.into_inner().map_err(|e| e.to_string())
}

#[derive(Clone, Debug, Deserialize)]
pub struct HeatPumpPerformancePoint {
    pub outdoor_c: f64,
    pub capacity_fraction: f64,
    pub heating_cop: f64,
    pub cooling_cop: f64,
}

fn default_heat_pump_performance() -> Vec<HeatPumpPerformancePoint> {
    vec![
        HeatPumpPerformancePoint {
            outdoor_c: -15.0,
            capacity_fraction: 0.55,
            heating_cop: 2.0,
            cooling_cop: 3.0,
        },
        HeatPumpPerformancePoint {
            outdoor_c: 10.0,
            capacity_fraction: 1.0,
            heating_cop: 3.2,
            cooling_cop: 3.8,
        },
        HeatPumpPerformancePoint {
            outdoor_c: 35.0,
            capacity_fraction: 0.8,
            heating_cop: 2.7,
            cooling_cop: 3.0,
        },
    ]
}

fn heat_pump_performance(points: &[HeatPumpPerformancePoint], outdoor_c: f64) -> (f64, f64, f64) {
    let low = points.partition_point(|point| point.outdoor_c < outdoor_c);
    let (a, b) = if low == 0 {
        (&points[0], &points[0])
    } else if low >= points.len() {
        let last = &points[points.len() - 1];
        (last, last)
    } else {
        (&points[low - 1], &points[low])
    };
    let fraction = if a.outdoor_c == b.outdoor_c {
        0.0
    } else {
        (outdoor_c - a.outdoor_c) / (b.outdoor_c - a.outdoor_c)
    };
    let interpolate = |start: f64, end: f64| start + fraction * (end - start);
    (
        interpolate(a.capacity_fraction, b.capacity_fraction),
        interpolate(a.heating_cop, b.heating_cop),
        interpolate(a.cooling_cop, b.cooling_cop),
    )
}

#[derive(Clone, Debug, Deserialize)]
pub struct EquipmentOutage {
    pub equipment: String,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EquipmentStartupSeconds {
    #[serde(default)]
    pub heat_pump: f64,
    #[serde(default)]
    pub boiler: f64,
    #[serde(default)]
    pub chiller: f64,
}

impl Default for EquipmentStartupSeconds {
    fn default() -> Self {
        Self {
            heat_pump: 0.0,
            boiler: 0.0,
            chiller: 0.0,
        }
    }
}

fn equipment_available(outages: &[EquipmentOutage], equipment: &str, hour: f64) -> bool {
    !outages.iter().any(|outage| {
        outage.equipment == equipment
            && hour >= outage.start_hour
            && hour < outage.start_hour + outage.duration_hours
    })
}

#[derive(Clone, Debug, Deserialize)]
pub struct Scenario {
    #[serde(default = "default_ua")]
    pub zone_ua_kw_per_k: f64,
    #[serde(default = "default_cap")]
    pub zone_capacity_kwh_per_k: f64,
    #[serde(default = "default_heat_pump_capacity")]
    pub heat_pump_capacity_kw: f64,
    #[serde(default = "default_heat_pump_performance")]
    pub heat_pump_performance: Vec<HeatPumpPerformancePoint>,
    #[serde(default)]
    pub equipment_outages: Vec<EquipmentOutage>,
    #[serde(default)]
    pub equipment_start_failures: Vec<EquipmentOutage>,
    #[serde(default)]
    pub window_openings: Vec<WindowOpening>,
    #[serde(default)]
    pub occupancy_overrides: Vec<ZoneOverride>,
    #[serde(default)]
    pub equipment_startup_seconds: EquipmentStartupSeconds,
    #[serde(default = "default_disabled_hour")]
    pub heat_pump_fault_reset_hour: f64,
    #[serde(default = "default_disabled_hour")]
    pub boiler_fault_reset_hour: f64,
    #[serde(default = "default_disabled_hour")]
    pub chiller_fault_reset_hour: f64,
    #[serde(default = "default_boiler_capacity")]
    pub boiler_capacity_kw: f64,
    #[serde(default = "default_chiller_capacity")]
    pub chiller_capacity_kw: f64,
    #[serde(default = "default_step_seconds")]
    pub step_seconds: usize,
    #[serde(default)]
    pub freeze_test_ahu: usize,
    #[serde(default = "default_disabled_hour")]
    pub freeze_test_start_hour: f64,
    #[serde(default)]
    pub freeze_test_duration_hours: f64,
    #[serde(default = "default_disabled_hour")]
    pub freeze_test_reset_hour: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WindowOpening {
    pub zone: usize,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ZoneOverride {
    pub zone: usize,
    pub start_hour: f64,
    pub duration_hours: f64,
}
fn default_disabled_hour() -> f64 {
    -1.0
}
fn default_step_seconds() -> usize {
    900
}
fn default_ua() -> f64 {
    0.15
}
fn default_cap() -> f64 {
    5.0
}
fn default_heat_pump_capacity() -> f64 {
    80.0
}
fn default_boiler_capacity() -> f64 {
    100.0
}
fn default_chiller_capacity() -> f64 {
    100.0
}

/// Equivalent pump curve and parallel-load resistance for one water loop.
/// Full speed and full valve conductance produce 40 kPa and 4.31 kg/s.
fn water_loop_hydraulics(pump_speed: f64, valve_area: f64) -> (f64, f64) {
    let speed = clamp(pump_speed, 0.0, 1.0);
    let area = clamp(valve_area, 0.0, 1.0);
    let pressure_pa = 80_000.0 * speed.powi(2) / (1.0 + area.powi(2));
    let flow_kg_s = 4.31 * area * (pressure_pa / 40_000.0).sqrt();
    (pressure_pa, flow_kg_s)
}

#[wasm_bindgen]
pub struct Simulation {
    graph: Block,
    weather: Vec<Weather>,
    seconds_until_occupied: Vec<f64>,
    scenario: Scenario,
    effective_step_seconds: usize,
    executor: Executor,
    compiled: Option<CompiledExecutor>,
    temps: [f64; 10],
    cursor: usize,
    electricity_kwh: f64,
    gas_kwh: f64,
    unmet_hours: f64,
    occupied_unmet_hours: f64,
    last_heat_request: f64,
    last_cool_request: f64,
    last_flow_request: [f64; 2],
    last_supply_c: [f64; 2],
    last_mixed_air_c: [f64; 2],
    last_duct_pressure: [f64; 2],
    last_outdoor_airflow: [f64; 2],
    last_zone_airflow: [f64; 10],
    last_zone_discharge_c: [f64; 10],
    last_water_supply_c: [f64; 2],
    last_water_return_c: [f64; 2],
    last_water_flow_kg_s: [f64; 2],
    last_water_dp_pa: [f64; 2],
    source_start_elapsed_s: [f64; 3],
    source_running: [bool; 3],
    zone_occupied: [bool; 10],
    raw_occupancy: [bool; 10],
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn sampled_step(graph: &Block, requested: usize) -> Result<usize, String> {
    fn visit(block: &Block, result: &mut Vec<f64>) {
        if block.block_type.rsplit('.').next() == Some("UnitDelay") {
            if let Some(period) = block
                .parameters
                .iter()
                .find(|p| p.name == "samplePeriod")
                .and_then(|p| p.value.as_f64())
            {
                result.push(period);
            }
        }
        for child in &block.blocks {
            visit(child, result);
        }
    }
    let mut periods = Vec::new();
    visit(graph, &mut periods);
    let mut step = requested;
    for period in periods {
        let integer = period.round();
        if !period.is_finite()
            || period < 15.0
            || (integer - period).abs() > 1e-9
            || integer > usize::MAX as f64
        {
            return Err(format!("Building simulation requires UnitDelay samplePeriod to be a whole number of seconds at least 15; got {period}"));
        }
        step = gcd(step, integer as usize);
    }
    if step < 15 {
        return Err("UnitDelay sample periods require a simulation step below the supported 15-second minimum".into());
    }
    Ok(step)
}

fn g36_control_contract(graph: &Block) -> Vec<String> {
    if graph.name != "FourPipeG36Controls" {
        return Vec::new();
    }
    let mut errors = Vec::new();
    let plant_outputs = [
        "plant_hot_water_enable",
        "plant_chilled_water_enable",
        "plant_hot_water_pump_speed",
        "plant_chilled_water_pump_speed",
        "plant_hot_water_supply_setpoint_c",
        "plant_chilled_water_supply_setpoint_c",
        "plant_heat_pump_heating_kw",
        "plant_heat_pump_cooling_kw",
        "plant_heat_pump_requested_mode",
        "plant_heat_pump_start_fault",
        "plant_boiler_start_fault",
        "plant_chiller_start_fault",
        "plant_boiler_kw",
        "plant_chiller_kw",
    ];
    let ahu_outputs = [
        "supply_air_temp",
        "fan_speed",
        "outdoor_air_damper_position",
        "return_air_damper_position",
        "heating_coil_valve_position",
        "cooling_coil_valve_position",
    ];
    let zone_outputs = [
        "occupancy_confirmed",
        "conditioning_state",
        "airflow_request",
        "damper_position",
        "reheat_valve_position",
    ];
    let names = std::iter::once("preconditioning_mode".to_string())
        .chain(plant_outputs.into_iter().map(str::to_string))
        .chain((1..=2).flat_map(|ahu| ahu_outputs.map(move |name| format!("ahu_{ahu}_{name}"))))
        .chain(
            (1..=10)
                .flat_map(|zone| zone_outputs.map(move |name| format!("zone_{zone:02}_{name}"))),
        );
    for name in names {
        let declared = graph.outputs.iter().any(|port| port.name == name);
        let driven = graph.connections.iter().any(|edge| edge.target().0 == name);
        if !declared || !driven {
            errors.push(format!(
                "G36 simulation requires a CDL-driven {name} output"
            ));
        }
    }
    errors
}

/// Deterministic per-zone presence, sampled with the 15-minute weather data.
/// Arrival, departure, and occasional midday absences differ by zone.
fn raw_zone_occupancy(weather_index: usize, zone: usize, scheduled: bool) -> bool {
    if !scheduled {
        return false;
    }
    let quarter = weather_index % 96;
    let day = weather_index / 96;
    quarter >= 31 + zone % 5
        && quarter < 70 - zone % 4
        && !(zone % 3 == 0 && (48..52).contains(&quarter))
        && !((day + zone) % 7 == 0 && (56..60).contains(&quarter))
}

#[wasm_bindgen]
impl Simulation {
    #[wasm_bindgen(constructor)]
    pub fn new(
        graph_json: &str,
        weather_json: &str,
        scenario_json: &str,
    ) -> Result<Simulation, JsValue> {
        let graph: Block =
            serde_json::from_str(graph_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mut errors = validate(&graph);
        errors.extend(g36_control_contract(&graph));
        if !errors.is_empty() {
            return Err(JsValue::from_str(&errors.join("; ")));
        }
        if graph
            .blocks
            .iter()
            .filter(|b| b.block_type == "VAVBoxController")
            .count()
            != 10
            || !graph.blocks.iter().any(|b| b.block_type == "PlantDispatch")
            || graph
                .blocks
                .iter()
                .filter(|b| b.block_type == "AHUController")
                .count()
                != 2
        {
            return Err(JsValue::from_str(
                "Building simulation requires ten VAV, two AHU, and one plant composite",
            ));
        }
        let weather: Vec<Weather> =
            serde_json::from_str(weather_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        if weather.len() != 35_040 {
            return Err(JsValue::from_str(
                "Expected 35,040 quarter-hour weather samples",
            ));
        }
        // Calendar information is an exogenous input. The graph decides when
        // that lookahead warrants preconditioning.
        let mut seconds_until_occupied = vec![0.0; weather.len()];
        let mut next_occupied = weather.len();
        for index in (0..weather.len()).rev() {
            if weather[index].occupied {
                next_occupied = index;
            }
            seconds_until_occupied[index] = if next_occupied == weather.len() {
                (weather.len() * 900) as f64
            } else {
                ((next_occupied - index) * 900) as f64
            };
        }
        let scenario: Scenario =
            serde_json::from_str(scenario_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        if scenario.zone_ua_kw_per_k <= 0.0
            || scenario.zone_capacity_kwh_per_k <= 0.0
            || !scenario.heat_pump_capacity_kw.is_finite()
            || scenario.heat_pump_capacity_kw <= 0.0
            || scenario.heat_pump_performance.is_empty()
            || scenario.heat_pump_performance.iter().any(|point| {
                !point.outdoor_c.is_finite()
                    || !point.capacity_fraction.is_finite()
                    || point.capacity_fraction <= 0.0
                    || point.capacity_fraction > 1.0
                    || !point.heating_cop.is_finite()
                    || point.heating_cop <= 0.0
                    || !point.cooling_cop.is_finite()
                    || point.cooling_cop <= 0.0
            })
            || scenario
                .heat_pump_performance
                .windows(2)
                .any(|pair| pair[0].outdoor_c >= pair[1].outdoor_c)
            || scenario.equipment_outages.iter().any(|outage| {
                !matches!(
                    outage.equipment.as_str(),
                    "heat_pump" | "boiler" | "chiller"
                ) || !outage.start_hour.is_finite()
                    || outage.start_hour < 0.0
                    || !outage.duration_hours.is_finite()
                    || outage.duration_hours <= 0.0
            })
            || scenario.equipment_start_failures.iter().any(|failure| {
                !matches!(
                    failure.equipment.as_str(),
                    "heat_pump" | "boiler" | "chiller"
                ) || !failure.start_hour.is_finite()
                    || failure.start_hour < 0.0
                    || !failure.duration_hours.is_finite()
                    || failure.duration_hours <= 0.0
            })
            || scenario.window_openings.iter().any(|opening| {
                !(1..=10).contains(&opening.zone)
                    || !opening.start_hour.is_finite()
                    || opening.start_hour < 0.0
                    || !opening.duration_hours.is_finite()
                    || opening.duration_hours <= 0.0
            })
            || scenario.occupancy_overrides.iter().any(|override_event| {
                !(1..=10).contains(&override_event.zone)
                    || !override_event.start_hour.is_finite()
                    || override_event.start_hour < 0.0
                    || !override_event.duration_hours.is_finite()
                    || override_event.duration_hours <= 0.0
            })
            || !scenario.heat_pump_fault_reset_hour.is_finite()
            || scenario.heat_pump_fault_reset_hour < -1.0
            || !scenario.boiler_fault_reset_hour.is_finite()
            || scenario.boiler_fault_reset_hour < -1.0
            || !scenario.chiller_fault_reset_hour.is_finite()
            || scenario.chiller_fault_reset_hour < -1.0
            || [
                scenario.equipment_startup_seconds.heat_pump,
                scenario.equipment_startup_seconds.boiler,
                scenario.equipment_startup_seconds.chiller,
            ]
            .iter()
            .any(|seconds| !seconds.is_finite() || *seconds < 0.0)
            || !scenario.boiler_capacity_kw.is_finite()
            || scenario.boiler_capacity_kw <= 0.0
            || !scenario.chiller_capacity_kw.is_finite()
            || scenario.chiller_capacity_kw <= 0.0
            || !matches!(scenario.step_seconds, 15 | 30 | 60 | 300 | 900)
            || scenario.freeze_test_ahu > 2
            || !scenario.freeze_test_start_hour.is_finite()
            || scenario.freeze_test_start_hour < -1.0
            || !scenario.freeze_test_duration_hours.is_finite()
            || scenario.freeze_test_duration_hours < 0.0
            || !scenario.freeze_test_reset_hour.is_finite()
            || scenario.freeze_test_reset_hour < -1.0
        {
            return Err(JsValue::from_str(
                "Invalid building assumptions or simulation timestep",
            ));
        }
        let effective_step_seconds =
            sampled_step(&graph, scenario.step_seconds).map_err(|e| JsValue::from_str(&e))?;
        let compiled = CompiledExecutor::try_new(&graph).map_err(|e| JsValue::from_str(&e))?;
        Ok(Self {
            graph,
            weather,
            seconds_until_occupied,
            scenario,
            effective_step_seconds,
            executor: Executor::default(),
            compiled,
            temps: [22.0; 10],
            cursor: 0,
            electricity_kwh: 0.0,
            gas_kwh: 0.0,
            unmet_hours: 0.0,
            occupied_unmet_hours: 0.0,
            last_heat_request: 0.0,
            last_cool_request: 0.0,
            last_flow_request: [0.0; 2],
            last_supply_c: [16.0; 2],
            last_mixed_air_c: [20.0; 2],
            last_duct_pressure: [1.0; 2],
            last_outdoor_airflow: [0.0; 2],
            last_zone_airflow: [0.0; 10],
            last_zone_discharge_c: [16.0; 10],
            last_water_supply_c: [45.0, 7.0],
            last_water_return_c: [45.0, 7.0],
            last_water_flow_kg_s: [0.0; 2],
            last_water_dp_pa: [0.0; 2],
            source_start_elapsed_s: [0.0; 3],
            source_running: [false; 3],
            zone_occupied: [false; 10],
            raw_occupancy: [false; 10],
        })
    }

    pub fn step_chunk(&mut self, count: usize) -> Result<String, JsValue> {
        let rows = self.advance_chunk(count)?;
        serde_json::to_string(&json!({"rows":rows,"progress":self.cursor,"total":self.total(),"done":self.done(),"summary":{"electricity_kwh":self.electricity_kwh,"gas_kwh":self.gas_kwh,"unmet_zone_hours":self.unmet_hours,"occupied_unmet_zone_hours":self.occupied_unmet_hours}})).map_err(|e|JsValue::from_str(&e.to_string()))
    }

    pub fn step_chunk_arrow(&mut self, count: usize) -> Result<Vec<u8>, JsValue> {
        let rows = self.advance_chunk(count)?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        arrow_batch(&rows, &self.executor.trace_targets).map_err(|e| JsValue::from_str(&e))
    }

    pub fn progress(&self) -> usize {
        self.cursor
    }
    pub fn effective_step_seconds(&self) -> usize {
        self.effective_step_seconds
    }
    pub fn total(&self) -> usize {
        self.weather.len() * (900 / self.effective_step_seconds)
    }
    pub fn done(&self) -> bool {
        self.cursor == self.total()
    }
    pub fn summary_json(&self) -> String {
        json!({"electricity_kwh":self.electricity_kwh,"gas_kwh":self.gas_kwh,"unmet_zone_hours":self.unmet_hours,"occupied_unmet_zone_hours":self.occupied_unmet_hours}).to_string()
    }

    fn advance_chunk(&mut self, count: usize) -> Result<Vec<Value>, JsValue> {
        let mut rows = Vec::new();
        let steps_per_weather = 900 / self.effective_step_seconds;
        let total_steps = self.weather.len() * steps_per_weather;
        let dt_seconds = self.effective_step_seconds as f64;
        let dt_hours = dt_seconds / 3600.0;
        let end = (self.cursor + count.min(672)).min(total_steps);
        while self.cursor < end {
            let index = self.cursor / steps_per_weather;
            let w = &self.weather[index];
            let elapsed_hours = self.cursor as f64 * dt_hours;
            let equipment_status = ["heat_pump", "boiler", "chiller"].map(|name| {
                equipment_available(&self.scenario.equipment_outages, name, elapsed_hours)
            });
            let start_failure = ["heat_pump", "boiler", "chiller"].map(|name| {
                !equipment_available(&self.scenario.equipment_start_failures, name, elapsed_hours)
            });
            let mut cool_request = 0.0;
            let mut heat_request = 0.0;
            let mut requested_flow = [0.0_f64; 10];
            let mut damper_command = [0.0_f64; 10];
            let mut requested_total = [0.0_f64; 2];
            let mut reheat_command = [0.0_f64; 10];
            let mut control_inputs = HashMap::new();
            control_inputs.insert("outdoor_c".into(), json!(w.outdoor_c));
            control_inputs.insert("schedule_occupied".into(), json!(w.occupied));
            control_inputs.insert(
                "seconds_until_occupied".into(),
                json!((self.seconds_until_occupied[index]
                    - (self.cursor % steps_per_weather) as f64 * dt_seconds)
                    .max(0.0)),
            );
            control_inputs.insert("heat_request_kw".into(), json!(self.last_heat_request));
            control_inputs.insert("cool_request_kw".into(), json!(self.last_cool_request));
            for (name, available) in ["heat_pump", "boiler", "chiller"]
                .into_iter()
                .zip(equipment_status)
            {
                control_inputs.insert(format!("{name}_available"), json!(available));
            }
            // Running status is measured at the end of the preceding physical step.
            for (name, running) in ["heat_pump", "boiler", "chiller"]
                .into_iter()
                .zip(self.source_running)
            {
                control_inputs.insert(format!("{name}_running"), json!(running));
            }
            for (equipment, reset_hour) in [
                ("heat_pump", self.scenario.heat_pump_fault_reset_hour),
                ("boiler", self.scenario.boiler_fault_reset_hour),
                ("chiller", self.scenario.chiller_fault_reset_hour),
            ] {
                control_inputs.insert(
                    format!("{equipment}_fault_reset"),
                    json!(
                        reset_hour >= 0.0
                            && elapsed_hours <= reset_hour
                            && elapsed_hours + dt_hours > reset_hour
                    ),
                );
            }
            let (hp_capacity_fraction, heating_cop, cooling_cop) =
                heat_pump_performance(&self.scenario.heat_pump_performance, w.outdoor_c);
            let hp_available_capacity_kw =
                self.scenario.heat_pump_capacity_kw * hp_capacity_fraction;
            control_inputs.insert(
                "heat_pump_available_capacity_kw".into(),
                json!(hp_available_capacity_kw),
            );
            control_inputs.insert(
                "hot_water_supply_temp_c".into(),
                json!(self.last_water_supply_c[0]),
            );
            control_inputs.insert(
                "hot_water_return_temp_c".into(),
                json!(self.last_water_return_c[0]),
            );
            control_inputs.insert(
                "chilled_water_supply_temp_c".into(),
                json!(self.last_water_supply_c[1]),
            );
            control_inputs.insert(
                "chilled_water_return_temp_c".into(),
                json!(self.last_water_return_c[1]),
            );
            control_inputs.insert(
                "hot_water_flow_kg_s".into(),
                json!(self.last_water_flow_kg_s[0]),
            );
            control_inputs.insert(
                "chilled_water_flow_kg_s".into(),
                json!(self.last_water_flow_kg_s[1]),
            );
            control_inputs.insert(
                "hot_water_differential_pressure_pa".into(),
                json!(self.last_water_dp_pa[0]),
            );
            control_inputs.insert(
                "chilled_water_differential_pressure_pa".into(),
                json!(self.last_water_dp_pa[1]),
            );
            control_inputs.insert(
                "ahu_1_flow_request".into(),
                json!(self.last_flow_request[0]),
            );
            control_inputs.insert(
                "ahu_2_flow_request".into(),
                json!(self.last_flow_request[1]),
            );
            control_inputs.insert(
                "ahu_1_actual_supply_air_temp".into(),
                json!(self.last_supply_c[0]),
            );
            control_inputs.insert(
                "ahu_2_actual_supply_air_temp".into(),
                json!(self.last_supply_c[1]),
            );
            let next_hours = elapsed_hours + dt_seconds / 3600.0;
            for ahu in 0..2 {
                control_inputs.insert(
                    format!("ahu_{}_supply_fan_proven", ahu + 1),
                    json!(
                        self.last_zone_airflow[ahu * 5..ahu * 5 + 5]
                            .iter()
                            .sum::<f64>()
                            > 0.01
                    ),
                );
                let selected = self.scenario.freeze_test_ahu == ahu + 1;
                let fault_start = self.scenario.freeze_test_start_hour;
                control_inputs.insert(
                    format!("ahu_{}_freeze_stat_trip", ahu + 1),
                    json!(
                        selected
                            && fault_start >= 0.0
                            && self.scenario.freeze_test_duration_hours > 0.0
                            && elapsed_hours >= fault_start
                            && elapsed_hours
                                < fault_start + self.scenario.freeze_test_duration_hours
                    ),
                );
                let reset_hour = self.scenario.freeze_test_reset_hour;
                control_inputs.insert(
                    format!("ahu_{}_freeze_reset", ahu + 1),
                    json!(
                        selected
                            && reset_hour >= 0.0
                            && elapsed_hours <= reset_hour
                            && next_hours > reset_hour
                    ),
                );
                control_inputs.insert(
                    format!("ahu_{}_mixed_air_temp", ahu + 1),
                    json!(self.last_mixed_air_c[ahu]),
                );
                control_inputs.insert(
                    format!("ahu_{}_duct_pressure", ahu + 1),
                    json!(self.last_duct_pressure[ahu]),
                );
                control_inputs.insert(
                    format!("ahu_{}_outdoor_airflow", ahu + 1),
                    json!(self.last_outdoor_airflow[ahu]),
                );
                control_inputs.insert(
                    format!("ahu_{}_return_air_temp", ahu + 1),
                    json!(self.temps[ahu * 5..ahu * 5 + 5].iter().sum::<f64>() / 5.0),
                );
            }
            for zone in 0..10 {
                let name = format!("zone_{:02}", zone + 1);
                let raw = raw_zone_occupancy(index, zone, w.occupied);
                self.raw_occupancy[zone] = raw;
                control_inputs.insert(format!("{name}_occupied"), json!(raw));
                let window_closed = !self.scenario.window_openings.iter().any(|opening| {
                    opening.zone == zone + 1
                        && elapsed_hours >= opening.start_hour
                        && elapsed_hours < opening.start_hour + opening.duration_hours
                });
                control_inputs.insert(format!("{name}_window_closed"), json!(window_closed));
                let local_override = self.scenario.occupancy_overrides.iter().any(|event| {
                    event.zone == zone + 1
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                });
                control_inputs.insert(format!("{name}_override"), json!(local_override));
                control_inputs.insert(format!("{name}_room_temp"), json!(self.temps[zone]));
                control_inputs.insert(
                    format!("{name}_measured_airflow"),
                    json!(self.last_zone_airflow[zone]),
                );
                control_inputs.insert(
                    format!("{name}_discharge_temp"),
                    json!(self.last_zone_discharge_c[zone]),
                );
                control_inputs.insert(
                    format!("{name}_supply_pressure"),
                    json!(self.last_duct_pressure[zone / 5]),
                );
            }
            let record_traces = (self.cursor + 1) % steps_per_weather == 0;
            let (controls, traces) = if let Some(compiled) = &mut self.compiled {
                compiled
                    .execute(&control_inputs, dt_seconds, record_traces)
                    .map_err(|e| JsValue::from_str(&e))?
            } else {
                self.executor.trace_values.clear();
                self.executor.skip_traces = !record_traces;
                let controls = self
                    .executor
                    .execute(&self.graph, &control_inputs, dt_seconds, "")
                    .map_err(|e| JsValue::from_str(&e))?;
                let mut traces = serde_json::Map::new();
                if record_traces {
                    for target in &self.executor.trace_targets {
                        let value = self.executor.trace_values.get(target).ok_or_else(|| {
                            JsValue::from_str(&format!(
                                "Selected graph signal {target} has no value"
                            ))
                        })?;
                        let numeric = value
                            .as_f64()
                            .or_else(|| value.as_bool().map(|flag| if flag { 1.0 } else { 0.0 }))
                            .ok_or_else(|| {
                                JsValue::from_str("Selected graph signal is not numeric or Boolean")
                            })?;
                        traces.insert(target.clone(), json!(numeric));
                    }
                }
                (controls, traces)
            };
            let output = |name: &str| -> Result<f64, JsValue> {
                controls
                    .get(name)
                    .and_then(Value::as_f64)
                    .ok_or_else(|| JsValue::from_str(&format!("Missing {name} output")))
            };
            let control_flag = |name: &str| -> Option<bool> {
                controls
                    .get(name)
                    .and_then(|value| value.as_bool().or_else(|| value.as_f64().map(|v| v != 0.0)))
            };
            let g36 = self.graph.name == "FourPipeG36Controls";
            let preconditioning = if g36 {
                control_flag("preconditioning_mode")
                    .ok_or_else(|| JsValue::from_str("Missing CDL preconditioning_mode output"))?
            } else {
                control_flag("preconditioning_mode").unwrap_or(false)
            };
            for zone in 0..10 {
                let name = format!("zone_{:02}_occupancy_confirmed", zone + 1);
                self.zone_occupied[zone] = if g36 {
                    control_flag(&name)
                        .ok_or_else(|| JsValue::from_str(&format!("Missing CDL {name} output")))?
                } else {
                    control_flag(&name).unwrap_or(self.raw_occupancy[zone])
                };
            }
            let hot_water_enabled = control_flag("plant_hot_water_enable")
                .ok_or_else(|| JsValue::from_str("Missing plant_hot_water_enable output"))?;
            let chilled_water_enabled = control_flag("plant_chilled_water_enable")
                .ok_or_else(|| JsValue::from_str("Missing plant_chilled_water_enable output"))?;
            let hot_water_pump_speed = clamp(output("plant_hot_water_pump_speed")?, 0.0, 1.0);
            let chilled_water_pump_speed =
                clamp(output("plant_chilled_water_pump_speed")?, 0.0, 1.0);
            let water_setpoint_c = [
                output("plant_hot_water_supply_setpoint_c")?,
                output("plant_chilled_water_supply_setpoint_c")?,
            ];
            let water_supply_c = self.last_water_supply_c;
            let hp_heat_command = output("plant_heat_pump_heating_kw")?.max(0.0);
            let hp_cool_command = output("plant_heat_pump_cooling_kw")?.max(0.0);
            let boiler_command = output("plant_boiler_kw")?.max(0.0);
            let chiller_command = output("plant_chiller_kw")?.max(0.0);
            let source_start_faults = ["heat_pump", "boiler", "chiller"].map(|equipment| {
                let name = format!("plant_{equipment}_start_fault");
                if g36 {
                    control_flag(&name)
                        .ok_or_else(|| JsValue::from_str(&format!("Missing CDL {name} output")))
                } else {
                    Ok(false)
                }
            });
            let source_start_faults = source_start_faults
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;
            let source_requests = [
                hp_heat_command + hp_cool_command,
                boiler_command,
                chiller_command,
            ];
            let startup_seconds = [
                self.scenario.equipment_startup_seconds.heat_pump,
                self.scenario.equipment_startup_seconds.boiler,
                self.scenario.equipment_startup_seconds.chiller,
            ];
            for source in 0..3 {
                if equipment_status[source]
                    && !start_failure[source]
                    && source_requests[source] > 0.0
                {
                    self.source_start_elapsed_s[source] += dt_seconds;
                    self.source_running[source] =
                        self.source_start_elapsed_s[source] >= startup_seconds[source];
                } else {
                    self.source_start_elapsed_s[source] = 0.0;
                    self.source_running[source] = false;
                }
            }
            let heat_pump_mode = if g36 {
                output("plant_heat_pump_requested_mode")?
            } else {
                0.0
            };
            let physical_hp_capacity_kw = if self.source_running[0] {
                hp_available_capacity_kw
            } else {
                0.0
            };
            let hp_command_limit = if hp_heat_command + hp_cool_command > 0.0 {
                (physical_hp_capacity_kw / (hp_heat_command + hp_cool_command)).min(1.0)
            } else {
                1.0
            };
            let hp_heat = hp_heat_command * hp_command_limit;
            let hp_cool = hp_cool_command * hp_command_limit;
            let boiler = boiler_command.min(if self.source_running[1] {
                self.scenario.boiler_capacity_kw
            } else {
                0.0
            });
            let chiller = chiller_command.min(if self.source_running[2] {
                self.scenario.chiller_capacity_kw
            } else {
                0.0
            });
            let plant_heat = hp_heat + boiler;
            let plant_cool = hp_cool + chiller;
            let supply_setpoint = [
                output("ahu_1_supply_air_temp")?,
                output("ahu_2_supply_air_temp")?,
            ];
            let fan_speed = [output("ahu_1_fan_speed")?, output("ahu_2_fan_speed")?];
            let mut outdoor_air_fraction = [0.0; 2];
            let mut intake_opening = [1.0; 2];
            for (ahu, fraction) in outdoor_air_fraction.iter_mut().enumerate() {
                let prefix = format!("ahu_{}", ahu + 1);
                let outdoor_name = format!("{prefix}_outdoor_air_damper_position");
                let return_name = format!("{prefix}_return_air_damper_position");
                *fraction = if g36
                    || (controls.contains_key(&outdoor_name) && controls.contains_key(&return_name))
                {
                    let outdoor = clamp(output(&outdoor_name)?, 0.0, 1.0);
                    let return_air = clamp(output(&return_name)?, 0.0, 1.0);
                    let open_area = outdoor + return_air;
                    intake_opening[ahu] = clamp(open_area, 0.0, 1.0);
                    if open_area > 0.0 {
                        outdoor / open_area
                    } else {
                        0.0
                    }
                } else {
                    clamp(output(&format!("{prefix}_outdoor_air_fraction"))?, 0.0, 1.0)
                };
            }
            let heating_coil_valve = [
                clamp(output("ahu_1_heating_coil_valve_position")?, 0.0, 1.0),
                clamp(output("ahu_2_heating_coil_valve_position")?, 0.0, 1.0),
            ];
            let cooling_coil_valve = [
                clamp(output("ahu_1_cooling_coil_valve_position")?, 0.0, 1.0),
                clamp(output("ahu_2_cooling_coil_valve_position")?, 0.0, 1.0),
            ];
            for zone in 0..10 {
                let name = format!("zone_{:02}", zone + 1);
                let ahu = zone / 5;
                requested_flow[zone] = output(&format!("{name}_airflow_request"))?.max(0.0);
                damper_command[zone] = clamp(output(&format!("{name}_damper_position"))?, 0.0, 1.0);
                reheat_command[zone] =
                    clamp(output(&format!("{name}_reheat_valve_position"))?, 0.0, 1.0);
                requested_total[ahu] += requested_flow[zone];
            }
            // Two parallel valve groups and a small bypass form a simplified
            // hydraulic network. The remote differential pressure follows a
            // quadratic pump curve and resistance law; opening valves raises
            // flow and lowers pressure at a fixed pump speed.
            let hot_valve_area = 0.2
                + 0.4 * heating_coil_valve.iter().sum::<f64>() / 2.0
                + 0.4 * reheat_command.iter().sum::<f64>() / 10.0;
            let chilled_valve_area = 0.2 + 0.8 * cooling_coil_valve.iter().sum::<f64>() / 2.0;
            let valve_area = [hot_valve_area, chilled_valve_area];
            let pump_speed = [hot_water_pump_speed, chilled_water_pump_speed];
            let hydraulics = [0, 1].map(|loop_index| {
                water_loop_hydraulics(pump_speed[loop_index], valve_area[loop_index])
            });
            let water_dp_pa = hydraulics.map(|(pressure, _)| pressure);
            let water_flow_kg_s = hydraulics.map(|(_, flow)| flow);
            // The harness resolves a simple fan/terminal air network from CDL
            // actuator commands. Airflow setpoints do not directly deliver air.
            // At pressure 1 each fully open terminal passes its 0.95 design
            // airflow; the AHU fan can supply five units at full speed.
            let terminal_design_flow = 0.95;
            let damper_area = [0, 1].map(|ahu| {
                terminal_design_flow * damper_command[ahu * 5..ahu * 5 + 5].iter().sum::<f64>()
            });
            let duct_pressure = [0, 1].map(|ahu| {
                if damper_area[ahu] > 0.0 {
                    clamp(
                        (5.0 * clamp(fan_speed[ahu], 0.0, 1.0) * intake_opening[ahu]
                            / damper_area[ahu])
                            .powi(2),
                        0.0,
                        1.0,
                    )
                } else if fan_speed[ahu] > 0.0 && intake_opening[ahu] > 0.0 {
                    1.0
                } else {
                    0.0
                }
            });
            let mut delivered_flow = [0.0_f64; 10];
            let mut delivered_by_ahu = [0.0_f64; 2];
            let mut reheat_request = [0.0_f64; 10];
            for zone in 0..10 {
                let ahu = zone / 5;
                delivered_flow[zone] =
                    terminal_design_flow * damper_command[zone] * duct_pressure[ahu].sqrt();
                delivered_by_ahu[ahu] += delivered_flow[zone];
                let discharge_limit_kw =
                    delivered_flow[zone] * 1.2 * (40.0 - supply_setpoint[ahu]).max(0.0);
                reheat_request[zone] = (reheat_command[zone] * 8.0).min(discharge_limit_kw);
                heat_request += reheat_request[zone];
            }
            let mut mixed_air_c = [0.0_f64; 2];
            let outdoor_airflow =
                [0, 1].map(|ahu| delivered_by_ahu[ahu] * outdoor_air_fraction[ahu]);
            let mut coil_heat_request = [0.0_f64; 2];
            let mut coil_cool_request = [0.0_f64; 2];
            for ahu in 0..2 {
                let return_c = self.temps[ahu * 5..ahu * 5 + 5].iter().sum::<f64>() / 5.0;
                mixed_air_c[ahu] = (1.0 - outdoor_air_fraction[ahu]) * return_c
                    + outdoor_air_fraction[ahu] * w.outdoor_c;
                let coil_capacity_kw = delivered_by_ahu[ahu] * 1.2 * 20.0;
                coil_heat_request[ahu] = (coil_capacity_kw * heating_coil_valve[ahu])
                    .min(delivered_by_ahu[ahu] * 1.2 * (40.0 - mixed_air_c[ahu]).max(0.0));
                coil_cool_request[ahu] = (coil_capacity_kw * cooling_coil_valve[ahu])
                    .min(delivered_by_ahu[ahu] * 1.2 * (mixed_air_c[ahu] - 8.0).max(0.0));
                heat_request += coil_heat_request[ahu];
                cool_request += coil_cool_request[ahu];
            }
            let water_heat_capacity_kw =
                water_flow_kg_s[0] * 4.18 * (water_supply_c[0] - 35.0).max(0.0);
            let water_cool_capacity_kw =
                water_flow_kg_s[1] * 4.18 * (17.0 - water_supply_c[1]).max(0.0);
            let heat_availability = if heat_request > 0.0 {
                clamp(water_heat_capacity_kw / heat_request, 0.0, 1.0)
            } else {
                1.0
            };
            let cool_availability = if cool_request > 0.0 {
                clamp(water_cool_capacity_kw / cool_request, 0.0, 1.0)
            } else {
                1.0
            };
            let actual_supply_c = [0, 1].map(|ahu| {
                if delivered_by_ahu[ahu] > 0.0 {
                    mixed_air_c[ahu]
                        + (coil_heat_request[ahu] * heat_availability
                            - coil_cool_request[ahu] * cool_availability)
                            / (1.2 * delivered_by_ahu[ahu])
                } else {
                    mixed_air_c[ahu]
                }
            });
            let mut discharge_c = [0.0_f64; 10];
            for zone in 0..10 {
                let ahu = zone / 5;
                let reheat_kw = reheat_request[zone] * heat_availability;
                discharge_c[zone] = if delivered_flow[zone] > 0.0 {
                    actual_supply_c[ahu] + reheat_kw / (1.2 * delivered_flow[zone])
                } else {
                    actual_supply_c[ahu]
                };
                let air_exchange_kw =
                    delivered_flow[zone] * 1.2 * (discharge_c[zone] - self.temps[zone]);
                let solar_share = w.solar_kw * (0.7 + 0.08 * (zone % 5) as f64) / 10.0;
                let load = self.scenario.zone_ua_kw_per_k * (w.outdoor_c - self.temps[zone])
                    + solar_share
                    + w.internal_kw / 10.0
                    + air_exchange_kw;
                self.temps[zone] = clamp(
                    self.temps[zone] + load * dt_hours / self.scenario.zone_capacity_kwh_per_k,
                    -10.0,
                    50.0,
                );
                let (lo, hi) = if w.occupied {
                    (20.0, 25.0)
                } else {
                    (17.0, 29.0)
                };
                if self.temps[zone] < lo || self.temps[zone] > hi {
                    self.unmet_hours += dt_hours;
                    if w.occupied && self.raw_occupancy[zone] {
                        self.occupied_unmet_hours += dt_hours;
                    }
                }
            }
            self.last_heat_request = heat_request;
            self.last_cool_request = cool_request;
            self.last_flow_request = requested_total;
            self.last_supply_c = actual_supply_c;
            self.last_mixed_air_c = mixed_air_c;
            self.last_duct_pressure = duct_pressure;
            self.last_outdoor_airflow = outdoor_airflow;
            self.last_zone_airflow = delivered_flow;
            self.last_zone_discharge_c = discharge_c;
            let heat_kw = heat_request * heat_availability;
            let cool_kw = cool_request * cool_availability;
            let water_return_c = [
                if water_flow_kg_s[0] > 0.0 {
                    water_supply_c[0] - heat_kw / (water_flow_kg_s[0] * 4.18)
                } else {
                    water_supply_c[0]
                },
                if water_flow_kg_s[1] > 0.0 {
                    water_supply_c[1] + cool_kw / (water_flow_kg_s[1] * 4.18)
                } else {
                    water_supply_c[1]
                },
            ];
            // A 5,000 kg mixed volume on each loop stores sensible energy.
            // CDL commands source production; this physical state only balances
            // heat produced and heat transferred to coils and terminal reheat.
            let water_capacity_kwh_per_k = 5_000.0 * 4.18 / 3_600.0;
            let source_heat_kw = plant_heat.min(
                heat_kw + (60.0 - water_supply_c[0]).max(0.0) * water_capacity_kwh_per_k / dt_hours,
            );
            let source_cool_kw = plant_cool.min(
                cool_kw + (water_supply_c[1] - 4.0).max(0.0) * water_capacity_kwh_per_k / dt_hours,
            );
            self.last_water_supply_c = [
                clamp(
                    water_supply_c[0]
                        + (source_heat_kw - heat_kw) * dt_hours / water_capacity_kwh_per_k,
                    35.0,
                    60.0,
                ),
                clamp(
                    water_supply_c[1]
                        + (cool_kw - source_cool_kw) * dt_hours / water_capacity_kwh_per_k,
                    4.0,
                    17.0,
                ),
            ];
            self.last_water_return_c = water_return_c;
            self.last_water_flow_kg_s = water_flow_kg_s;
            self.last_water_dp_pa = water_dp_pa;
            let hp_heat_used = if plant_heat > 0.0 {
                source_heat_kw * hp_heat / plant_heat
            } else {
                0.0
            };
            let boiler_used = source_heat_kw - hp_heat_used;
            let hp_cool_used = if plant_cool > 0.0 {
                source_cool_kw * hp_cool / plant_cool
            } else {
                0.0
            };
            let chiller_used = source_cool_kw - hp_cool_used;
            let fan_kw = fan_speed
                .iter()
                .map(|s| 2.0 * clamp(*s, 0.0, 1.0).powi(3))
                .sum::<f64>();
            let water_pump_kw =
                3.0 * (hot_water_pump_speed.powi(3) + chilled_water_pump_speed.powi(3));
            // A CDL request for simultaneous service shares one compressor.
            // The remaining heat exchange goes to or from ambient air.
            let heat_pump_electric_kw = if g36 && heat_pump_mode == 3.0 {
                (hp_heat_used / heating_cop).max(hp_cool_used / cooling_cop)
            } else {
                hp_heat_used / heating_cop + hp_cool_used / cooling_cop
            };
            let heat_pump_ambient_exchange_kw = hp_heat_used - hp_cool_used - heat_pump_electric_kw;
            let electric_kw = heat_pump_electric_kw + chiller_used / 3.5 + fan_kw + water_pump_kw;
            let gas_kw = boiler_used / 0.9;
            self.electricity_kwh += electric_kw * dt_hours;
            self.gas_kwh += gas_kw * dt_hours;
            if (self.cursor + 1) % steps_per_weather == 0 {
                let mut row = json!({"index":index,"schedule_occupied":if w.occupied {1.0} else {0.0},"preconditioning_mode":if preconditioning {1.0} else {0.0},"zone_occupied":self.zone_occupied.map(|occupied|if occupied {1.0} else {0.0}),"zone_presence_raw":self.raw_occupancy.map(|occupied|if occupied {1.0} else {0.0}),"zone_airflow":delivered_flow,"zone_discharge_c":discharge_c,"outdoor_c":w.outdoor_c,"avg_zone_c":self.temps.iter().sum::<f64>()/10.0,"zone_c":self.temps,"cool_kw":cool_kw,"heat_kw":heat_kw,"cool_request_kw":cool_request,"heat_request_kw":heat_request,"electric_kw":electric_kw,"gas_kw":gas_kw,"requested_airflow":requested_total.iter().sum::<f64>(),"delivered_airflow":delivered_by_ahu.iter().sum::<f64>(),"ahu_1_supply_actual_c":actual_supply_c[0],"ahu_2_supply_actual_c":actual_supply_c[1],"ahu_1_supply_setpoint_c":supply_setpoint[0],"ahu_2_supply_setpoint_c":supply_setpoint[1],"ahu_1_duct_pressure":duct_pressure[0],"ahu_2_duct_pressure":duct_pressure[1],"ahu_1_outdoor_air_fraction":outdoor_air_fraction[0],"ahu_2_outdoor_air_fraction":outdoor_air_fraction[1],"ahu_1_intake_opening":intake_opening[0],"ahu_2_intake_opening":intake_opening[1],"ahu_1_heating_coil_valve_position":heating_coil_valve[0],"ahu_2_heating_coil_valve_position":heating_coil_valve[1],"ahu_1_cooling_coil_valve_position":cooling_coil_valve[0],"ahu_2_cooling_coil_valve_position":cooling_coil_valve[1],"ahu_1_outdoor_airflow":outdoor_airflow[0],"ahu_2_outdoor_airflow":outdoor_airflow[1],"ahu_1_mixed_air_c":mixed_air_c[0],"ahu_2_mixed_air_c":mixed_air_c[1],"heat_pump_heating_used_kw":hp_heat_used,"heat_pump_cooling_used_kw":hp_cool_used,"boiler_used_kw":boiler_used,"chiller_used_kw":chiller_used});
                row.as_object_mut()
                    .unwrap()
                    .insert("heat_pump_electric_kw".into(), json!(heat_pump_electric_kw));
                row.as_object_mut().unwrap().insert(
                    "heat_pump_ambient_exchange_kw".into(),
                    json!(heat_pump_ambient_exchange_kw),
                );
                row.as_object_mut().unwrap().insert(
                    "heat_pump_available_capacity_kw".into(),
                    json!(hp_available_capacity_kw),
                );
                row.as_object_mut()
                    .unwrap()
                    .insert("heat_pump_heating_cop".into(), json!(heating_cop));
                row.as_object_mut()
                    .unwrap()
                    .insert("heat_pump_cooling_cop".into(), json!(cooling_cop));
                for (name, available) in ["heat_pump", "boiler", "chiller"]
                    .into_iter()
                    .zip(equipment_status)
                {
                    row.as_object_mut().unwrap().insert(
                        format!("{name}_available"),
                        json!(if available { 1.0 } else { 0.0 }),
                    );
                }
                for (name, running) in ["heat_pump", "boiler", "chiller"]
                    .into_iter()
                    .zip(self.source_running)
                {
                    row.as_object_mut().unwrap().insert(
                        format!("{name}_running"),
                        json!(if running { 1.0 } else { 0.0 }),
                    );
                }
                for (equipment, fault) in ["heat_pump", "boiler", "chiller"]
                    .into_iter()
                    .zip(source_start_faults.iter())
                {
                    row.as_object_mut().unwrap().insert(
                        format!("plant_{equipment}_start_fault"),
                        json!(if *fault { 1.0 } else { 0.0 }),
                    );
                }
                for ahu in 0..2 {
                    let prefix = format!("ahu_{}", ahu + 1);
                    let measures = [
                        ("heating_coil_request_kw", coil_heat_request[ahu]),
                        ("cooling_coil_request_kw", coil_cool_request[ahu]),
                        (
                            "heating_coil_delivered_kw",
                            coil_heat_request[ahu] * heat_availability,
                        ),
                        (
                            "cooling_coil_delivered_kw",
                            coil_cool_request[ahu] * cool_availability,
                        ),
                    ];
                    for (name, value) in measures {
                        row.as_object_mut()
                            .unwrap()
                            .insert(format!("{prefix}_{name}"), json!(value));
                    }
                }
                let water_measures = [
                    ("hot_water_supply_temp_c", water_supply_c[0]),
                    ("hot_water_supply_setpoint_c", water_setpoint_c[0]),
                    ("hot_water_return_temp_c", water_return_c[0]),
                    ("chilled_water_supply_temp_c", water_supply_c[1]),
                    ("chilled_water_supply_setpoint_c", water_setpoint_c[1]),
                    ("chilled_water_return_temp_c", water_return_c[1]),
                    ("hot_water_flow_kg_s", water_flow_kg_s[0]),
                    ("chilled_water_flow_kg_s", water_flow_kg_s[1]),
                    ("hot_water_differential_pressure_pa", water_dp_pa[0]),
                    ("chilled_water_differential_pressure_pa", water_dp_pa[1]),
                    ("hot_water_pump_speed", hot_water_pump_speed),
                    ("chilled_water_pump_speed", chilled_water_pump_speed),
                    (
                        "hot_water_enable",
                        if hot_water_enabled { 1.0 } else { 0.0 },
                    ),
                    (
                        "chilled_water_enable",
                        if chilled_water_enabled { 1.0 } else { 0.0 },
                    ),
                    ("water_pump_kw", water_pump_kw),
                ];
                for (name, value) in water_measures {
                    row.as_object_mut()
                        .unwrap()
                        .insert(name.to_string(), json!(value));
                }
                if !traces.is_empty() {
                    row["traces"] = Value::Object(traces);
                }
                rows.push(row);
            }
            self.cursor += 1;
        }
        Ok(rows)
    }

    pub fn set_traces(&mut self, targets_json: &str) -> Result<(), JsValue> {
        let targets: Vec<String> =
            serde_json::from_str(targets_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        if let Some(compiled) = &mut self.compiled {
            compiled
                .set_traces(&targets)
                .map_err(|e| JsValue::from_str(&e))?;
        }
        self.executor.trace_targets = targets;
        Ok(())
    }
}

#[wasm_bindgen]
pub fn validate_json(graph_json: &str) -> String {
    match serde_json::from_str::<Block>(graph_json) {
        Ok(graph) => serde_json::to_string(&validate(&graph)).unwrap(),
        Err(e) => serde_json::to_string(&vec![e.to_string()]).unwrap(),
    }
}

#[wasm_bindgen]
pub fn execute_json(graph_json: &str, inputs_json: &str, dt: f64) -> Result<String, JsValue> {
    let graph: Block =
        serde_json::from_str(graph_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let inputs: HashMap<String, Value> =
        serde_json::from_str(inputs_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let result = Executor::default()
        .execute(&graph, &inputs, dt, "")
        .map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn triggered_cdl_blocks_match_in_dynamic_and_compiled_execution() {
        let cases = [
            (
                "TriggeredSampler",
                json!([{"name":"y_start","value":-1}]),
                vec![
                    (1.0, false, -1.0),
                    (2.0, true, 2.0),
                    (3.0, true, 2.0),
                    (4.0, false, 2.0),
                    (5.0, true, 5.0),
                ],
            ),
            (
                "TriggeredMovingMean",
                json!([{"name":"n","value":2}]),
                vec![
                    (1.0, false, 1.0),
                    (2.0, true, 1.5),
                    (3.0, true, 1.5),
                    (4.0, false, 1.5),
                    (5.0, true, 3.5),
                ],
            ),
        ];
        for (kind, parameters, sequence) in cases {
            let graph: Block = serde_json::from_value(json!({
                "name":"sampler", "block_type":format!("Buildings.Controls.OBC.CDL.Discrete.{kind}"),
                "inputs":[{"name":"u","type":"Real"},{"name":"trigger","type":"Boolean"}],
                "outputs":[{"name":"y","type":"Real"}], "parameters":parameters,
            })).unwrap();
            assert!(validate(&graph).is_empty());
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
            for (value, trigger, expected) in sequence {
                let inputs = HashMap::from([
                    ("u".into(), json!(value)),
                    ("trigger".into(), json!(trigger)),
                ]);
                assert_eq!(
                    dynamic.execute(&graph, &inputs, 15.0, "").unwrap()["y"],
                    json!(expected)
                );
                assert_eq!(
                    compiled.execute(&inputs, 15.0, false).unwrap().0["y"],
                    json!(expected)
                );
            }
        }
    }
    #[test]
    fn cdl_edges_and_timer_match_in_dynamic_and_compiled_execution() {
        for (kind, expected) in [
            ("Edge", vec![false, true, false, false, true]),
            ("FallingEdge", vec![false, false, false, true, false]),
        ] {
            let graph: Block = serde_json::from_value(json!({
                "name":"edge", "block_type":format!("Buildings.Controls.OBC.CDL.Logical.{kind}"),
                "inputs":[{"name":"u","type":"Boolean"}],
                "outputs":[{"name":"y","type":"Boolean"}],
            }))
            .unwrap();
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
            for (active, wanted) in [false, true, true, false, true].into_iter().zip(expected) {
                let inputs = HashMap::from([("u".into(), json!(active))]);
                assert_eq!(
                    dynamic.execute(&graph, &inputs, 15.0, "").unwrap()["y"],
                    json!(wanted)
                );
                assert_eq!(
                    compiled.execute(&inputs, 15.0, false).unwrap().0["y"],
                    json!(wanted as i32 as f64)
                );
            }
        }
        let timer: Block = serde_json::from_value(json!({
            "name":"timer", "block_type":"Buildings.Controls.OBC.CDL.Logical.Timer",
            "inputs":[{"name":"u","type":"Boolean"}],
            "outputs":[{"name":"y","type":"Real"},{"name":"passed","type":"Boolean"}],
            "parameters":[{"name":"t","value":30}],
        }))
        .unwrap();
        assert!(validate(&timer).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&timer).unwrap().unwrap();
        for (active, seconds, passed) in [
            (false, 0.0, false),
            (true, 0.0, false),
            (true, 15.0, false),
            (true, 30.0, true),
            (false, 0.0, false),
            (true, 0.0, false),
        ] {
            let inputs = HashMap::from([("u".into(), json!(active))]);
            let reference = dynamic.execute(&timer, &inputs, 15.0, "").unwrap();
            let optimized = compiled.execute(&inputs, 15.0, false).unwrap().0;
            assert_eq!(reference["y"], json!(seconds));
            assert_eq!(reference["passed"], json!(passed));
            assert_eq!(optimized["y"], json!(seconds));
            assert_eq!(optimized["passed"], json!(passed as i32 as f64));
        }
    }
    #[test]
    fn water_hydraulics_couple_pump_speed_valve_area_pressure_and_flow() {
        let (design_pressure, design_flow) = water_loop_hydraulics(1.0, 1.0);
        assert_eq!(design_pressure, 40_000.0);
        assert_eq!(design_flow, 4.31);
        let (closed_pressure, closed_flow) = water_loop_hydraulics(1.0, 0.2);
        assert!(closed_pressure > design_pressure);
        assert!(closed_flow < design_flow);
        let (slow_pressure, slow_flow) = water_loop_hydraulics(0.5, 1.0);
        assert_eq!(slow_pressure, design_pressure / 4.0);
        assert_eq!(slow_flow, design_flow / 2.0);
        assert_eq!(water_loop_hydraulics(0.0, 1.0), (0.0, 0.0));
    }
    fn add_ahu_ventilation_inputs(inputs: &mut HashMap<String, Value>) {
        for zone in 1..=5 {
            let name = format!("zone_{zone:02}");
            for (suffix, value) in [
                ("ventilation_area", 0.03),
                ("ventilation_population", 0.06),
                ("ventilation_minimum_oa_flow", 0.09),
                ("measured_airflow", 0.4),
            ] {
                inputs
                    .entry(format!("{name}_{suffix}"))
                    .or_insert(json!(value));
            }
        }
    }
    fn set_equipment_ready(inputs: &mut HashMap<String, Value>) {
        for equipment in ["heat_pump", "boiler", "chiller"] {
            inputs.insert(format!("{equipment}_available"), json!(true));
            inputs.insert(format!("{equipment}_running"), json!(true));
            inputs.insert(format!("{equipment}_fault_reset"), json!(false));
        }
    }
    #[test]
    fn g36_start_fault_graph_compiles() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        assert!(validate(&graph).is_empty());
        assert!(g36_control_contract(&graph).is_empty());
        assert!(sampled_step(&graph, 900).is_ok());
        assert!(CompiledExecutor::try_new(&graph).is_ok());
    }
    #[test]
    fn g36_heat_pump_failed_start_moves_load_to_boiler_until_operator_reset() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut inputs: HashMap<String, Value> = plant
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else {
                        json!(0.0)
                    },
                )
            })
            .collect();
        set_equipment_ready(&mut inputs);
        inputs.insert("heat_pump_running".into(), json!(false));
        for (name, value) in [
            ("heat_request_kw", 100.0),
            ("heat_pump_available_capacity_kw", 80.0),
            ("hot_water_flow_kg_s", 2.0),
            ("hot_water_supply_temp_c", 45.0),
        ] {
            inputs.insert(name.into(), json!(value));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        let mut fault = HashMap::new();
        for _ in 0..12 {
            fault = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                fault["heat_pump_start_fault"].as_bool().unwrap(),
                fast["heat_pump_start_fault"].as_f64().unwrap() != 0.0
            );
            assert_eq!(fault["heat_pump_heating_kw"], fast["heat_pump_heating_kw"]);
        }
        assert_eq!(fault["heat_pump_start_fault"], json!(true));
        assert_eq!(fault["heat_pump_heating_kw"], json!(0.0));
        assert!(fault["boiler_kw"].as_f64().unwrap() > 0.0);
        inputs.insert("heat_pump_running".into(), json!(true));
        inputs.insert("heat_pump_fault_reset".into(), json!(true));
        let recovered = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(recovered["heat_pump_start_fault"], json!(false));
        assert_eq!(
            recovered["heat_pump_start_fault"].as_bool().unwrap(),
            fast["heat_pump_start_fault"].as_f64().unwrap() != 0.0
        );
        assert!(recovered["heat_pump_heating_kw"].as_f64().unwrap() > 0.0);
    }
    #[test]
    fn g36_backup_start_faults_are_latched_cdl_decisions() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        for (equipment, request, flow, supply, command) in [
            (
                "boiler",
                "heat_request_kw",
                "hot_water_flow_kg_s",
                "hot_water_supply_temp_c",
                "boiler_kw",
            ),
            (
                "chiller",
                "cool_request_kw",
                "chilled_water_flow_kg_s",
                "chilled_water_supply_temp_c",
                "chiller_kw",
            ),
        ] {
            let mut inputs: HashMap<String, Value> = plant
                .inputs
                .iter()
                .map(|port| {
                    (
                        port.name.clone(),
                        if port.kind == "Boolean" {
                            json!(false)
                        } else {
                            json!(0.0)
                        },
                    )
                })
                .collect();
            set_equipment_ready(&mut inputs);
            inputs.insert("heat_pump_available".into(), json!(false));
            inputs.insert("heat_pump_running".into(), json!(false));
            inputs.insert(format!("{equipment}_running"), json!(false));
            inputs.insert(request.into(), json!(100.0));
            inputs.insert(flow.into(), json!(2.0));
            inputs.insert(
                supply.into(),
                json!(if equipment == "boiler" { 45.0 } else { 7.0 }),
            );
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
            let fault_name = format!("{equipment}_start_fault");
            let mut fault = HashMap::new();
            for _ in 0..12 {
                fault = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
                let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
                assert_eq!(
                    fault[&fault_name].as_bool().unwrap(),
                    fast[&fault_name].as_f64().unwrap() != 0.0
                );
                assert_eq!(fault[command], fast[command]);
            }
            assert_eq!(fault[&fault_name], json!(true), "{equipment}");
            assert_eq!(fault[command], json!(0.0), "{equipment}");
            inputs.insert(format!("{equipment}_running"), json!(true));
            inputs.insert(format!("{equipment}_fault_reset"), json!(true));
            let recovered = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            assert_eq!(recovered[&fault_name], json!(false), "{equipment}");
            assert!(recovered[command].as_f64().unwrap() > 0.0, "{equipment}");
        }
    }
    #[test]
    fn boiler_start_fault_timeout_is_set_by_its_cdl_parameter() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut tuned = plant.clone();
        tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == "boiler_start_timeout")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "delayTime")
            .unwrap()
            .value = json!(120.0);
        let mut inputs: HashMap<String, Value> = plant
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else {
                        json!(0.0)
                    },
                )
            })
            .collect();
        set_equipment_ready(&mut inputs);
        inputs.insert("heat_pump_available".into(), json!(false));
        inputs.insert("heat_pump_running".into(), json!(false));
        inputs.insert("boiler_running".into(), json!(false));
        inputs.insert("heat_request_kw".into(), json!(100.0));
        inputs.insert("hot_water_flow_kg_s".into(), json!(2.0));
        inputs.insert("hot_water_supply_temp_c".into(), json!(45.0));
        let mut standard = Executor::default();
        let mut shortened = Executor::default();
        let mut original_fault = false;
        let mut tuned_fault = false;
        for _ in 0..5 {
            original_fault = standard.execute(plant, &inputs, 60.0, "").unwrap()
                ["boiler_start_fault"]
                .as_bool()
                .unwrap();
            tuned_fault = shortened.execute(&tuned, &inputs, 60.0, "").unwrap()
                ["boiler_start_fault"]
                .as_bool()
                .unwrap();
        }
        assert!(!original_fault);
        assert!(tuned_fault);
    }
    fn start_present_window(sim: &mut Simulation) {
        sim.cursor = 31 * (900 / sim.effective_step_seconds);
    }
    #[test]
    fn g36_requires_cdl_driven_occupancy_and_preconditioning_outputs() {
        let mut graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        assert!(g36_control_contract(&graph).is_empty());
        graph.connections.retain(|edge| {
            !matches!(
                edge.target().0.as_str(),
                "zone_01_occupancy_confirmed" | "preconditioning_mode"
            )
        });
        let errors = g36_control_contract(&graph);
        assert_eq!(errors.len(), 2);
        assert!(errors
            .iter()
            .any(|error| error.contains("zone_01_occupancy_confirmed")));
        assert!(errors
            .iter()
            .any(|error| error.contains("preconditioning_mode")));
    }
    #[test]
    fn g36_requires_graph_outputs_for_every_physical_actuator() {
        let mut graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        assert!(g36_control_contract(&graph).is_empty());
        let missing = [
            "plant_heat_pump_heating_kw",
            "ahu_1_outdoor_air_damper_position",
            "ahu_2_return_air_damper_position",
            "zone_01_damper_position",
        ];
        graph
            .connections
            .retain(|edge| !missing.contains(&edge.target().0.as_str()));
        let errors = g36_control_contract(&graph);
        assert_eq!(errors.len(), missing.len());
        for name in missing {
            assert!(errors.iter().any(|error| error.contains(name)));
        }
    }
    #[test]
    fn g36_source_commands_require_measured_water_flow_proof_in_both_executors() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut inputs: HashMap<String, Value> = plant
            .inputs
            .iter()
            .map(|port| (port.name.clone(), json!(0.0)))
            .collect();
        set_equipment_ready(&mut inputs);
        for (name, value) in [
            ("heat_request_kw", 100.0),
            ("cool_request_kw", 100.0),
            ("heat_pump_available_capacity_kw", 80.0),
            ("hot_water_supply_temp_c", 45.0),
            ("chilled_water_supply_temp_c", 7.0),
            ("chilled_water_flow_kg_s", 2.0),
        ] {
            inputs.insert(name.into(), json!(value));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        let cold_only = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let cold_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(cold_only["hot_source_proven_output"], json!(false));
        assert_eq!(cold_only["chilled_source_proven_output"], json!(true));
        assert_eq!(cold_only["heat_pump_heating_kw"], json!(0.0));
        assert_eq!(cold_only["boiler_kw"], json!(0.0));
        assert!(cold_only["heat_pump_cooling_kw"].as_f64().unwrap() > 0.0);
        assert!(cold_only["chiller_kw"].as_f64().unwrap() > 0.0);
        for name in [
            "heat_pump_heating_kw",
            "boiler_kw",
            "heat_pump_cooling_kw",
            "chiller_kw",
        ] {
            assert_eq!(cold_fast[name], cold_only[name]);
        }
        inputs.insert("hot_water_flow_kg_s".into(), json!(2.0));
        inputs.insert("chilled_water_flow_kg_s".into(), json!(0.0));
        let hot_only = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let hot_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(hot_only["hot_source_proven_output"], json!(true));
        assert_eq!(hot_only["chilled_source_proven_output"], json!(false));
        assert!(hot_only["heat_pump_heating_kw"].as_f64().unwrap() > 0.0);
        assert!(hot_only["boiler_kw"].as_f64().unwrap() > 0.0);
        assert_eq!(hot_only["heat_pump_cooling_kw"], json!(0.0));
        assert_eq!(hot_only["chiller_kw"], json!(0.0));
        for name in [
            "heat_pump_heating_kw",
            "boiler_kw",
            "heat_pump_cooling_kw",
            "chiller_kw",
        ] {
            assert_eq!(hot_fast[name], hot_only[name]);
        }
    }
    #[test]
    fn g36_hybrid_dispatch_allocates_capacity_and_backup_in_cdl() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut inputs: HashMap<String, Value> = plant
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else {
                        json!(0.0)
                    },
                )
            })
            .collect();
        set_equipment_ready(&mut inputs);
        for (name, value) in [
            ("heat_request_kw", 100.0),
            ("cool_request_kw", 100.0),
            ("heat_pump_available_capacity_kw", 40.0),
            ("hot_water_flow_kg_s", 2.0),
            ("chilled_water_flow_kg_s", 2.0),
            ("hot_water_supply_temp_c", 45.0),
            ("chilled_water_supply_temp_c", 7.0),
        ] {
            inputs.insert(name.into(), json!(value));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        let dual = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let dual_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for name in [
            "heat_pump_heating_kw",
            "heat_pump_cooling_kw",
            "boiler_kw",
            "chiller_kw",
            "heat_pump_requested_mode",
        ] {
            assert_eq!(dual[name], dual_fast[name], "{name}");
        }
        assert_eq!(dual["heat_pump_requested_mode"].as_f64().unwrap(), 3.0);
        let hp_total = dual["heat_pump_heating_kw"].as_f64().unwrap()
            + dual["heat_pump_cooling_kw"].as_f64().unwrap();
        assert!(
            (hp_total - 40.0).abs() < 1e-8,
            "allocated heat pump={hp_total}"
        );
        assert!(dual["boiler_kw"].as_f64().unwrap() > 0.0);
        assert!(dual["chiller_kw"].as_f64().unwrap() > 0.0);
        inputs.insert("chilled_water_flow_kg_s".into(), json!(0.0));
        let hot_only = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let hot_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(hot_only["heat_pump_requested_mode"].as_f64().unwrap(), 1.0);
        assert_eq!(hot_only["heat_pump_cooling_kw"], json!(0.0));
        assert_eq!(hot_only["chiller_kw"], json!(0.0));
        assert_eq!(
            hot_only["heat_pump_heating_kw"],
            hot_fast["heat_pump_heating_kw"]
        );
        assert!((hot_only["heat_pump_heating_kw"].as_f64().unwrap() - 40.0).abs() < 1e-8);
        let mut tuned = plant.clone();
        tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == "heat_pump_utilization_limit")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|p| p.name == "k")
            .unwrap()
            .value = json!(0.25);
        inputs.insert("chilled_water_flow_kg_s".into(), json!(2.0));
        let limited = Executor::default()
            .execute(&tuned, &inputs, 60.0, "")
            .unwrap();
        let limited_hp = limited["heat_pump_heating_kw"].as_f64().unwrap()
            + limited["heat_pump_cooling_kw"].as_f64().unwrap();
        assert!(
            (limited_hp - 10.0).abs() < 1e-8,
            "edited capacity limit={limited_hp}"
        );
        assert!(limited["boiler_kw"].as_f64().unwrap() > dual["boiler_kw"].as_f64().unwrap());
        assert!(limited["chiller_kw"].as_f64().unwrap() > dual["chiller_kw"].as_f64().unwrap());
    }
    #[test]
    fn g36_equipment_availability_gates_dispatch_in_cdl() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut inputs: HashMap<String, Value> = plant
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else {
                        json!(0.0)
                    },
                )
            })
            .collect();
        set_equipment_ready(&mut inputs);
        for (name, value) in [
            ("heat_request_kw", 100.0),
            ("cool_request_kw", 100.0),
            ("heat_pump_available_capacity_kw", 80.0),
            ("hot_water_flow_kg_s", 2.0),
            ("chilled_water_flow_kg_s", 2.0),
            ("hot_water_supply_temp_c", 45.0),
            ("chilled_water_supply_temp_c", 7.0),
        ] {
            inputs.insert(name.into(), json!(value));
        }
        let execute = |inputs: &HashMap<String, Value>| {
            let dynamic = Executor::default()
                .execute(plant, inputs, 60.0, "")
                .unwrap();
            let fast = CompiledExecutor::try_new(plant)
                .unwrap()
                .unwrap()
                .execute(inputs, 60.0, false)
                .unwrap()
                .0;
            for output in [
                "heat_pump_heating_kw",
                "heat_pump_cooling_kw",
                "boiler_kw",
                "chiller_kw",
                "heat_pump_requested_mode",
            ] {
                assert_eq!(dynamic[output], fast[output], "{output}");
            }
            dynamic
        };
        let normal = execute(&inputs);
        assert_eq!(normal["heat_pump_requested_mode"].as_i64(), Some(3));
        inputs.insert("heat_pump_available".into(), json!(false));
        let no_heat_pump = execute(&inputs);
        assert_eq!(no_heat_pump["heat_pump_heating_kw"], json!(0.0));
        assert_eq!(no_heat_pump["heat_pump_cooling_kw"], json!(0.0));
        assert_eq!(no_heat_pump["heat_pump_requested_mode"].as_i64(), Some(0));
        assert!(
            no_heat_pump["boiler_kw"].as_f64().unwrap() > normal["boiler_kw"].as_f64().unwrap()
        );
        assert!(
            no_heat_pump["chiller_kw"].as_f64().unwrap() > normal["chiller_kw"].as_f64().unwrap()
        );
        inputs.insert("boiler_available".into(), json!(false));
        assert_eq!(execute(&inputs)["boiler_kw"], json!(0.0));
        inputs.insert("chiller_available".into(), json!(false));
        assert_eq!(execute(&inputs)["chiller_kw"], json!(0.0));
    }
    #[test]
    fn raw_equipment_outage_reaches_graph_and_physical_delivery() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: -5.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let scenario = json!({"step_seconds":60,"equipment_outages":[{
            "equipment":"heat_pump","start_hour":7.75,"duration_hours":0.25
        }]});
        let mut sim = Simulation::new(graph, &weather, &scenario.to_string()).unwrap();
        start_present_window(&mut sim);
        sim.temps = [16.0; 10];
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        let rows = run_weather_intervals(&mut sim, 2);
        let outage = &rows["rows"][0];
        let restored = &rows["rows"][1];
        assert_eq!(outage["heat_pump_available"], json!(0.0));
        assert_eq!(outage["heat_pump_heating_used_kw"], json!(0.0));
        assert_eq!(outage["heat_pump_cooling_used_kw"], json!(0.0));
        assert!(outage["boiler_used_kw"].as_f64().unwrap() > 0.0);
        assert_eq!(restored["heat_pump_available"], json!(1.0));
        assert!(restored["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
    }
    #[test]
    fn physical_source_start_delay_withholds_running_proof_and_delivery() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: -5.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let scenario = json!({"step_seconds":60,"equipment_startup_seconds":{
            "heat_pump":300.0,"boiler":0.0,"chiller":0.0
        }});
        let mut sim = Simulation::new(graph, &weather, &scenario.to_string()).unwrap();
        start_present_window(&mut sim);
        sim.temps = [16.0; 10];
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        for step in 1..5 {
            sim.last_heat_request = 100.0;
            sim.last_water_flow_kg_s = [2.0, 2.0];
            sim.step_chunk(1).unwrap();
            assert_eq!(sim.source_start_elapsed_s[0], step as f64 * 60.0);
            assert!(!sim.source_running[0]);
        }
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        sim.step_chunk(1).unwrap();
        assert!(sim.source_running[0]);
        let remaining = run_weather_intervals(&mut sim, 1);
        let row = &remaining["rows"][0];
        assert_eq!(row["heat_pump_running"], json!(1.0));
        assert!(row["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
    }
    #[test]
    fn failed_start_keeps_availability_true_and_cdl_fault_dispatches_backup() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: -5.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let scenario = json!({"step_seconds":60,"equipment_start_failures":[{
            "equipment":"heat_pump","start_hour":7.75,"duration_hours":0.5
        }],"heat_pump_fault_reset_hour":8.25});
        let mut sim = Simulation::new(graph, &weather, &scenario.to_string()).unwrap();
        start_present_window(&mut sim);
        sim.temps = [16.0; 10];
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        let rows = run_weather_intervals(&mut sim, 3);
        let fault = &rows["rows"][0];
        assert_eq!(fault["heat_pump_available"], json!(1.0));
        assert_eq!(fault["heat_pump_running"], json!(0.0));
        assert_eq!(fault["plant_heat_pump_start_fault"], json!(1.0));
        assert_eq!(fault["heat_pump_heating_used_kw"], json!(0.0));
        assert!(fault["boiler_used_kw"].as_f64().unwrap() > 0.0);
        assert_eq!(rows["rows"][1]["plant_heat_pump_start_fault"], json!(1.0));
        assert_eq!(rows["rows"][2]["plant_heat_pump_start_fault"], json!(0.0));
    }
    #[test]
    fn physical_backup_start_failures_reach_cdl_alarms_and_reset() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        for (equipment, outdoor, room, request) in [
            ("boiler", -5.0, 16.0, "last_heat_request"),
            ("chiller", 35.0, 30.0, "last_cool_request"),
        ] {
            let weather = serde_json::to_string(&vec![
                Weather {
                    outdoor_c: outdoor,
                    solar_kw: 0.0,
                    internal_kw: 0.0,
                    occupied: true
                };
                35_040
            ])
            .unwrap();
            let mut scenario = json!({"step_seconds":60,
                "equipment_outages":[{"equipment":"heat_pump","start_hour":7.75,"duration_hours":1.0}],
                "equipment_start_failures":[{"equipment":equipment,"start_hour":7.75,"duration_hours":0.5}]
            });
            scenario[format!("{equipment}_fault_reset_hour")] = json!(8.25);
            let mut sim = Simulation::new(graph, &weather, &scenario.to_string()).unwrap();
            start_present_window(&mut sim);
            sim.temps = [room; 10];
            if request == "last_heat_request" {
                sim.last_heat_request = 100.0;
            } else {
                sim.last_cool_request = 100.0;
            }
            sim.last_water_flow_kg_s = [2.0, 2.0];
            let rows = run_weather_intervals(&mut sim, 3);
            let alarm = format!("plant_{equipment}_start_fault");
            assert_eq!(
                rows["rows"][0][format!("{equipment}_available")],
                json!(1.0),
                "{equipment}"
            );
            assert_eq!(
                rows["rows"][0][format!("{equipment}_running")],
                json!(0.0),
                "{equipment}"
            );
            assert_eq!(rows["rows"][0][&alarm], json!(1.0), "{equipment}");
            assert_eq!(rows["rows"][1][&alarm], json!(1.0), "{equipment}");
            assert_eq!(rows["rows"][2][&alarm], json!(0.0), "{equipment}");
        }
    }
    #[test]
    fn outdoor_performance_changes_the_capacity_offered_to_cdl_dispatch() {
        let points = default_heat_pump_performance();
        assert_eq!(heat_pump_performance(&points, -30.0), (0.55, 2.0, 3.0));
        assert_eq!(heat_pump_performance(&points, 45.0), (0.8, 2.7, 3.0));
        let (fraction, heating_cop, cooling_cop) = heat_pump_performance(&points, -2.5);
        assert!((fraction - 0.775).abs() < 1e-10);
        assert!((heating_cop - 2.6).abs() < 1e-10);
        assert!((cooling_cop - 3.4).abs() < 1e-10);

        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut inputs: HashMap<String, Value> = plant
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else {
                        json!(0.0)
                    },
                )
            })
            .collect();
        set_equipment_ready(&mut inputs);
        for (name, value) in [
            ("heat_request_kw", 100.0),
            ("cool_request_kw", 100.0),
            ("hot_water_flow_kg_s", 2.0),
            ("chilled_water_flow_kg_s", 2.0),
            ("hot_water_supply_temp_c", 45.0),
            ("chilled_water_supply_temp_c", 7.0),
        ] {
            inputs.insert(name.into(), json!(value));
        }
        let run = |available_kw: f64| {
            let mut inputs = inputs.clone();
            inputs.insert(
                "heat_pump_available_capacity_kw".into(),
                json!(available_kw),
            );
            Executor::default()
                .execute(plant, &inputs, 60.0, "")
                .unwrap()
        };
        let cold = run(80.0 * heat_pump_performance(&points, -15.0).0);
        let mild = run(80.0 * heat_pump_performance(&points, 10.0).0);
        assert!(
            cold["heat_pump_heating_kw"].as_f64().unwrap()
                < mild["heat_pump_heating_kw"].as_f64().unwrap()
        );
        assert!(cold["boiler_kw"].as_f64().unwrap() > mild["boiler_kw"].as_f64().unwrap());
        assert!(cold["chiller_kw"].as_f64().unwrap() > mild["chiller_kw"].as_f64().unwrap());
    }
    #[test]
    fn heat_pump_cop_table_changes_physical_electricity_without_changing_cdl_commands() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 10.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let run = |cop: f64| {
            let scenario = json!({
                "heat_pump_performance": [
                    {"outdoor_c": 0.0, "capacity_fraction": 1.0, "heating_cop": cop, "cooling_cop": cop},
                    {"outdoor_c": 20.0, "capacity_fraction": 1.0, "heating_cop": cop, "cooling_cop": cop}
                ]
            });
            let mut sim = Simulation::new(graph, &weather, &scenario.to_string()).unwrap();
            start_present_window(&mut sim);
            sim.temps = [16.0; 10];
            sim.last_heat_request = 100.0;
            sim.last_water_flow_kg_s = [2.0, 2.0];
            run_weather_intervals(&mut sim, 1)["rows"][0].clone()
        };
        let low_cop = run(2.0);
        let high_cop = run(4.0);
        assert_eq!(low_cop["heat_pump_available_capacity_kw"], json!(80.0));
        assert_eq!(
            low_cop["heat_pump_heating_used_kw"],
            high_cop["heat_pump_heating_used_kw"]
        );
        assert!(low_cop["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
        assert!(
            low_cop["heat_pump_electric_kw"].as_f64().unwrap()
                > high_cop["heat_pump_electric_kw"].as_f64().unwrap()
        );
    }
    #[test]
    fn g36_capacity_input_moves_physical_heat_between_heat_pump_and_boiler() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: -5.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let scenario = |capacity: f64| {
            format!(
                r#"{{"heat_pump_capacity_kw":{capacity},"boiler_capacity_kw":1000,"zone_ua_kw_per_k":1.0,"zone_capacity_kwh_per_k":50.0}}"#
            )
        };
        let mut low = Simulation::new(graph, &weather, &scenario(10.0)).unwrap();
        let mut high = Simulation::new(graph, &weather, &scenario(80.0)).unwrap();
        for simulation in [&mut low, &mut high] {
            start_present_window(simulation);
            simulation.temps = [16.0; 10];
            simulation.last_heat_request = 100.0;
        }
        let low_rows = run_weather_intervals(&mut low, 2);
        let high_rows = run_weather_intervals(&mut high, 2);
        let low_hp: f64 = (0..2)
            .map(|i| {
                low_rows["rows"][i]["heat_pump_heating_used_kw"]
                    .as_f64()
                    .unwrap()
            })
            .sum();
        let high_hp: f64 = (0..2)
            .map(|i| {
                high_rows["rows"][i]["heat_pump_heating_used_kw"]
                    .as_f64()
                    .unwrap()
            })
            .sum();
        let low_boiler: f64 = (0..2)
            .map(|i| low_rows["rows"][i]["boiler_used_kw"].as_f64().unwrap())
            .sum();
        let high_boiler: f64 = (0..2)
            .map(|i| high_rows["rows"][i]["boiler_used_kw"].as_f64().unwrap())
            .sum();
        assert!(
            high_hp > low_hp + 50.0,
            "first load-step HP output: low={low_hp}, high={high_hp}"
        );
        assert!(
            low_boiler > high_boiler + 50.0,
            "first load-step boiler output: low={low_boiler}, high={high_boiler}"
        );
    }
    #[test]
    fn g36_simultaneous_mode_drives_shared_compressor_and_closes_energy_balance() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let mut separate: Value = serde_json::from_str(graph).unwrap();
        let plant = separate["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "plant")
            .unwrap();
        plant["connections"]
            .as_array_mut()
            .unwrap()
            .retain(|edge| edge["to"] != "heat_pump_requested_mode");
        plant["connections"]
            .as_array_mut()
            .unwrap()
            .push(json!({"from":"heat_pump_heating_mode_code.y","to":"heat_pump_requested_mode"}));
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 10.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let mut recovered = Simulation::new(graph, &weather, "{}").unwrap();
        let mut independent = Simulation::new(&separate.to_string(), &weather, "{}").unwrap();
        let mode_trace = "FourPipeG36Controls|output.plant_heat_pump_requested_mode";
        for sim in [&mut recovered, &mut independent] {
            sim.set_traces(&json!([mode_trace]).to_string()).unwrap();
            start_present_window(sim);
            sim.temps = [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0];
            sim.last_heat_request = 100.0;
            sim.last_cool_request = 100.0;
        }
        let recovered_rows = run_weather_intervals(&mut recovered, 3);
        let independent_rows = run_weather_intervals(&mut independent, 3);
        for i in 0..3 {
            let recovery = &recovered_rows["rows"][i];
            let separate = &independent_rows["rows"][i];
            let heat = recovery["heat_pump_heating_used_kw"].as_f64().unwrap();
            let cool = recovery["heat_pump_cooling_used_kw"].as_f64().unwrap();
            let work = recovery["heat_pump_electric_kw"].as_f64().unwrap();
            let ambient = recovery["heat_pump_ambient_exchange_kw"].as_f64().unwrap();
            assert!(heat > 0.0 && cool > 0.0);
            assert_eq!(recovery["traces"][mode_trace], json!(3.0));
            assert_eq!(separate["traces"][mode_trace], json!(1.0));
            assert!((heat - cool - work - ambient).abs() < 1e-9);
            assert_eq!(
                recovery["heat_pump_heating_used_kw"],
                separate["heat_pump_heating_used_kw"]
            );
            assert_eq!(
                recovery["heat_pump_cooling_used_kw"],
                separate["heat_pump_cooling_used_kw"]
            );
            assert!(work + 1.0 < separate["heat_pump_electric_kw"].as_f64().unwrap());
            assert!(
                (recovery["electric_kw"].as_f64().unwrap()
                    - separate["electric_kw"].as_f64().unwrap()
                    - (work - separate["heat_pump_electric_kw"].as_f64().unwrap()))
                .abs()
                    < 1e-9
            );
        }
    }
    #[test]
    fn water_actuators_follow_cdl_outputs_even_when_enable_alias_is_false() {
        let mut graph: Value = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        let plant = graph["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "plant")
            .unwrap();
        let edges = plant["connections"].as_array_mut().unwrap();
        edges.retain(|edge| {
            !(edge["from"] == "hot_water_pump_select.y" && edge["to"] == "hot_water_pump_speed")
                && !(edge["from"] == "boiler_enable_gate.y" && edge["to"] == "boiler_kw")
        });
        edges.push(json!({"from":"one.y","to":"hot_water_pump_speed"}));
        edges.push(json!({"from":"boiler.y","to":"boiler_kw"}));
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 20.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: false,
            };
            35_040
        ])
        .unwrap();
        let mut gated = Simulation::new(
            include_str!("../../web/public/examples/four_pipe_building.json"),
            &weather,
            "{}",
        )
        .unwrap();
        gated.last_water_supply_c[0] = 40.0;
        let gated_result: Value = serde_json::from_str(&gated.step_chunk(1).unwrap()).unwrap();
        assert_eq!(gated_result["rows"][0]["hot_water_enable"], json!(0.0));
        assert_eq!(gated_result["rows"][0]["boiler_used_kw"], json!(0.0));
        let mut sim = Simulation::new(&graph.to_string(), &weather, "{}").unwrap();
        sim.last_water_supply_c[0] = 40.0;
        let result: Value = serde_json::from_str(&sim.step_chunk(1).unwrap()).unwrap();
        let row = &result["rows"][0];
        assert_eq!(row["hot_water_enable"], json!(0.0));
        assert_eq!(row["hot_water_pump_speed"], json!(1.0));
        assert!(row["hot_water_flow_kg_s"].as_f64().unwrap() > 0.0);
        assert!(row["boiler_used_kw"].as_f64().unwrap() > 0.0);
    }
    #[test]
    fn g36_editable_water_pressure_target_changes_pump_flow_and_delivery() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 0.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let scenario = r#"{"zone_ua_kw_per_k":1.0,"zone_capacity_kwh_per_k":50.0}"#;
        let target = "FourPipeG36Controls|output.plant_hot_dp_setpoint_pa";
        let mut baseline = Simulation::new(graph, &weather, scenario).unwrap();
        baseline.set_traces(&json!([target]).to_string()).unwrap();
        start_present_window(&mut baseline);
        baseline.temps = [18.0; 10];
        baseline.last_heat_request = 200.0;
        let normal = run_weather_intervals(&mut baseline, 8);
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        let plant = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "plant")
            .unwrap();
        plant["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "hot_dp_minimum")
            .unwrap()["parameters"][0]["value"] = json!(30_000.0);
        let mut raised = Simulation::new(&edited.to_string(), &weather, scenario).unwrap();
        raised.set_traces(&json!([target]).to_string()).unwrap();
        start_present_window(&mut raised);
        raised.temps = [18.0; 10];
        raised.last_heat_request = 200.0;
        let higher = run_weather_intervals(&mut raised, 8);
        let base_row = &normal["rows"][7];
        let high_row = &higher["rows"][7];
        assert!(
            high_row["traces"][target].as_f64().unwrap()
                > base_row["traces"][target].as_f64().unwrap() + 8_000.0,
            "base target={}, raised target={}, base enable={}, raised enable={}",
            base_row["traces"][target],
            high_row["traces"][target],
            base_row["hot_water_enable"],
            high_row["hot_water_enable"]
        );
        assert!(
            high_row["hot_water_pump_speed"].as_f64().unwrap()
                > base_row["hot_water_pump_speed"].as_f64().unwrap() + 0.05
        );
        assert!(
            (high_row["hot_water_differential_pressure_pa"]
                .as_f64()
                .unwrap()
                - base_row["hot_water_differential_pressure_pa"]
                    .as_f64()
                    .unwrap())
            .abs()
                > 500.0,
            "base pressure={}, raised pressure={}",
            base_row["hot_water_differential_pressure_pa"],
            high_row["hot_water_differential_pressure_pa"]
        );
        assert!(
            (high_row["hot_water_flow_kg_s"].as_f64().unwrap()
                - base_row["hot_water_flow_kg_s"].as_f64().unwrap())
            .abs()
                > 0.01,
            "base flow={}, high-target flow={}",
            base_row["hot_water_flow_kg_s"],
            high_row["hot_water_flow_kg_s"]
        );
    }
    fn run_weather_intervals(sim: &mut Simulation, intervals: usize) -> Value {
        let target = sim.cursor + intervals * (900 / sim.effective_step_seconds);
        let mut rows = Vec::new();
        while sim.cursor < target {
            let chunk: Value = serde_json::from_str(
                &sim.step_chunk(target.saturating_sub(sim.cursor).min(672))
                    .unwrap(),
            )
            .unwrap();
            rows.extend(chunk["rows"].as_array().unwrap().iter().cloned());
        }
        json!({"rows": rows})
    }
    fn assert_coil_air_balance(row: &Value, ahu: usize) {
        let start = (ahu - 1) * 5;
        let flow: f64 = row["zone_airflow"].as_array().unwrap()[start..start + 5]
            .iter()
            .map(|value| value.as_f64().unwrap())
            .sum();
        if flow <= 0.0 {
            return;
        }
        let heat = row[format!("ahu_{ahu}_heating_coil_delivered_kw")]
            .as_f64()
            .unwrap();
        let cool = row[format!("ahu_{ahu}_cooling_coil_delivered_kw")]
            .as_f64()
            .unwrap();
        let mixed = row[format!("ahu_{ahu}_mixed_air_c")].as_f64().unwrap();
        let supply = row[format!("ahu_{ahu}_supply_actual_c")].as_f64().unwrap();
        assert!((1.2 * flow * (supply - mixed) - (heat - cool)).abs() < 1e-8);
    }
    fn assert_water_heat_balance(row: &Value) {
        let hot_flow = row["hot_water_flow_kg_s"].as_f64().unwrap();
        let chilled_flow = row["chilled_water_flow_kg_s"].as_f64().unwrap();
        let hot_delta = row["hot_water_supply_temp_c"].as_f64().unwrap()
            - row["hot_water_return_temp_c"].as_f64().unwrap();
        let chilled_delta = row["chilled_water_return_temp_c"].as_f64().unwrap()
            - row["chilled_water_supply_temp_c"].as_f64().unwrap();
        assert!((row["heat_kw"].as_f64().unwrap() - hot_flow * 4.18 * hot_delta).abs() < 1e-8);
        assert!(
            (row["cool_kw"].as_f64().unwrap() - chilled_flow * 4.18 * chilled_delta).abs() < 1e-8
        );
    }
    #[test]
    fn sampled_building_step_resolves_editable_cdl_periods() {
        let g36 = include_str!("../../web/public/examples/four_pipe_g36.json");
        let conventional = include_str!("../../web/public/examples/four_pipe_building.json");
        let weather = include_str!("../../web/public/examples/weather_2025.json");
        for (requested, expected) in [(900, 60), (300, 60), (60, 60), (30, 30), (15, 15)] {
            let sim = Simulation::new(g36, weather, &format!("{{\"step_seconds\":{requested}}}"))
                .unwrap();
            assert_eq!(sim.effective_step_seconds(), expected);
            assert_eq!(sim.total(), 35_040 * 900 / expected);
        }
        let mut aligned = Simulation::new(g36, weather, "{}").unwrap();
        assert!(
            serde_json::from_str::<Value>(&aligned.step_chunk(14).unwrap()).unwrap()["rows"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let boundary: Value = serde_json::from_str(&aligned.step_chunk(1).unwrap()).unwrap();
        assert_eq!(boundary["rows"].as_array().unwrap().len(), 1);
        assert_eq!(boundary["rows"][0]["index"], json!(0));
        assert_eq!(
            Simulation::new(conventional, weather, "{}")
                .unwrap()
                .effective_step_seconds(),
            900
        );
        let mut edited: Value = serde_json::from_str(g36).unwrap();
        for ahu in edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|b| b["block_type"] == "AHUController")
        {
            for name in [
                "pressure_previous",
                "sat_previous",
                "freeze_stage_two_previous",
            ] {
                ahu["blocks"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|b| b["name"] == name)
                    .unwrap()["parameters"][0]["value"] = json!(300.0);
            }
        }
        let plant = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "plant")
            .unwrap();
        for name in [
            "hot_stage_previous",
            "chilled_stage_previous",
            "hot_stage_observed",
            "chilled_stage_observed",
            "hot_stage_ready_observed",
            "chilled_stage_ready_observed",
            "heat_pump_start_request_observed",
            "boiler_start_request_observed",
            "chiller_start_request_observed",
            "hot_reset_held_value",
            "chilled_reset_held_value",
        ] {
            plant["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == name)
                .unwrap()["parameters"][0]["value"] = json!(300.0);
        }
        for block in edited["blocks"].as_array_mut().unwrap().iter_mut() {
            if block["name"].as_str().is_some_and(|name| {
                name.ends_with("_target_previous")
                    || name.ends_with("_unoccupied_heat_previous")
                    || name.ends_with("_unoccupied_cool_previous")
            }) {
                block["parameters"][0]["value"] = json!(300.0);
            }
        }
        assert_eq!(
            Simulation::new(&edited.to_string(), weather, "{}")
                .unwrap()
                .effective_step_seconds(),
            300
        );
        edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|b| b["name"] == "ahu_1")
            .unwrap()["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|b| b["name"] == "pressure_previous")
            .unwrap()["parameters"][0]["value"] = json!(113.0);
        let invalid: Block = serde_json::from_value(edited).unwrap();
        assert!(sampled_step(&invalid, 900).is_err());
    }
    #[test]
    fn unresolved_cxf_parameter_expression_is_rejected() {
        let graph: Block = serde_json::from_value(json!({
            "name": "gain", "block_type": "Buildings.Controls.OBC.CDL.Reals.MultiplyByParameter",
            "inputs": [{"name": "u", "type": "Real"}],
            "outputs": [{"name": "y", "type": "Real"}],
            "parameters": [{"name": "k", "value": "rootGain*2"}]
        }))
        .unwrap();
        assert!(validate(&graph)
            .iter()
            .any(|error| error.contains("unsupported parameter value")));
    }
    #[test]
    fn integer_requests_match_in_compiled_and_dynamic_execution() {
        let graph: Block = serde_json::from_value(json!({
            "name":"requests","block_type":"Composite","category":"composite",
            "inputs":[{"name":"a","type":"Boolean"},{"name":"b","type":"Boolean"}],
            "outputs":[{"name":"count","type":"Integer"}],
            "blocks":[
                {"name":"a_int","block_type":"Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger","inputs":[{"name":"u","type":"Boolean"}],"outputs":[{"name":"y","type":"Integer"}]},
                {"name":"b_int","block_type":"Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger","inputs":[{"name":"u","type":"Boolean"}],"outputs":[{"name":"y","type":"Integer"}]},
                {"name":"sum","block_type":"Buildings.Controls.OBC.CDL.Integers.Add","inputs":[{"name":"u1","type":"Integer"},{"name":"u2","type":"Integer"}],"outputs":[{"name":"y","type":"Integer"}]}
            ],
            "connections":[{"from":"a","to":"a_int.u"},{"from":"b","to":"b_int.u"},{"from":"a_int.y","to":"sum.u1"},{"from":"b_int.y","to":"sum.u2"},{"from":"sum.y","to":"count"}]
        })).unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (a, b, expected) in [(false, false, 0), (true, false, 1), (true, true, 2)] {
            let inputs = HashMap::from([("a".into(), json!(a)), ("b".into(), json!(b))]);
            let reference = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
            let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(reference["count"], json!(expected));
            assert_eq!(optimized["count"], json!(expected));
        }
    }
    #[test]
    fn hysteresis_rises_above_high_and_clears_below_low_in_both_executors() {
        let graph: Block = serde_json::from_value(json!({
            "name":"valve_request", "block_type":"Buildings.Controls.OBC.CDL.Reals.Hysteresis",
            "inputs":[{"name":"u","type":"Real"}],
            "outputs":[{"name":"y","type":"Boolean"}],
            "parameters":[{"name":"uLow","value":0.4},{"name":"uHigh","value":0.8}]
        }))
        .unwrap();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (value, expected) in [
            (0.0, false),
            (0.9, true),
            (0.7, true),
            (0.2, false),
            (0.7, false),
            (0.9, true),
            (0.3, false),
        ] {
            let inputs = HashMap::from([("u".into(), json!(value))]);
            let reference = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
            let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(reference["y"], json!(expected));
            assert_eq!(optimized["y"], json!(if expected { 1.0 } else { 0.0 }));
        }
        let mut initialized = graph.clone();
        initialized.parameters.push(Parameter {
            name: "pre_y_start".into(),
            value: json!(true),
        });
        let input = HashMap::from([("u".into(), json!(0.6))]);
        assert_eq!(
            Executor::default()
                .execute(&initialized, &input, 60.0, "")
                .unwrap()["y"],
            json!(true)
        );
        assert_eq!(
            CompiledExecutor::try_new(&initialized)
                .unwrap()
                .unwrap()
                .execute(&input, 60.0, false)
                .unwrap()
                .0["y"],
            json!(1.0)
        );
    }
    #[test]
    fn unit_delay_breaks_feedback_cycle_and_samples_at_its_period() {
        let graph: Block=serde_json::from_value(json!({
            "name":"sampled_feedback","block_type":"Composite","category":"composite",
            "inputs":[{"name":"increment","type":"Real"}],"outputs":[{"name":"result","type":"Real"}],
            "blocks":[
                {"name":"previous","block_type":"Buildings.Controls.OBC.CDL.Discrete.UnitDelay",
                 "inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],
                 "parameters":[{"name":"samplePeriod","value":120.0},{"name":"y_start","value":1.0}]},
                {"name":"next","block_type":"Add","inputs":[{"name":"u1","type":"Real"},{"name":"u2","type":"Real"}],"outputs":[{"name":"y","type":"Real"}]}
            ],
            "connections":[{"from":"previous.y","to":"next.u1"},{"from":"increment","to":"next.u2"},
                {"from":"next.y","to":"previous.u"},{"from":"next.y","to":"result"}]
        })).unwrap();
        assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
        let inputs = HashMap::from([("increment".into(), json!(1.0))]);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for expected in [2.0, 3.0, 3.0, 4.0, 4.0] {
            let reference = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
            let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(reference["result"], json!(expected));
            assert_eq!(optimized["result"], json!(expected));
        }
    }
    #[test]
    fn g36_ahu_aggregates_zone_requests_and_responds_to_measured_airflow() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(0.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(20.0)),
            ("actual_supply_air_temp".into(), json!(16.0)),
            ("duct_pressure".into(), json!(0.2)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("zone_01_pressure_request".into(), json!(1)),
            ("zone_02_pressure_request".into(), json!(1)),
            ("zone_03_pressure_request".into(), json!(1)),
            ("zone_04_pressure_request".into(), json!(0)),
            ("zone_05_pressure_request".into(), json!(0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_damper_position"), json!(0.0));
        }
        let mut ex = Executor::default();
        let mut optimized = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        let mut result = HashMap::new();
        for _ in 0..20 {
            result = ex.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = optimized.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                fast["pressure_request_count"],
                result["pressure_request_count"]
            );
            assert_eq!(
                fast["duct_pressure_setpoint"],
                result["duct_pressure_setpoint"]
            );
        }
        assert_eq!(result["pressure_request_count"], json!(3));
        assert!(result["duct_pressure_setpoint"].as_f64().unwrap() > 0.35);
        assert!(result["duct_pressure_setpoint"].as_f64().unwrap() <= 0.8);
        assert!(result["minimum_outdoor_airflow"].as_f64().unwrap() > 0.0);
        assert!(result["outdoor_air_fraction"].as_f64().unwrap() > 0.2);
        let mut recovered = inputs.clone();
        recovered.insert("outdoor_airflow".into(), json!(3.0));
        let after = ex.execute(ahu, &recovered, 60.0, "").unwrap();
        let fast = optimized.execute(&recovered, 60.0, false).unwrap().0;
        assert_eq!(
            fast["duct_pressure_setpoint"],
            after["duct_pressure_setpoint"]
        );
        assert!(
            after["outdoor_air_fraction"].as_f64().unwrap()
                <= result["outdoor_air_fraction"].as_f64().unwrap()
        );
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            recovered.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
        }
        let high = after["duct_pressure_setpoint"].as_f64().unwrap();
        for _ in 0..20 {
            let reference = ex.execute(ahu, &recovered, 60.0, "").unwrap();
            let fast = optimized.execute(&recovered, 60.0, false).unwrap().0;
            assert_eq!(
                fast["duct_pressure_setpoint"],
                reference["duct_pressure_setpoint"]
            );
            result = reference;
        }
        assert!(result["duct_pressure_setpoint"].as_f64().unwrap() < high);
        recovered.insert("flow_request".into(), json!(0.0));
        assert_eq!(
            ex.execute(ahu, &recovered, 60.0, "").unwrap()["duct_pressure_setpoint"],
            json!(0.35)
        );
    }
    #[test]
    fn g36_ahu_freeze_stage_one_uses_timed_supply_feedback_and_cdl_overrides() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(18.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(20.0)),
            ("actual_supply_air_temp".into(), json!(3.5)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_damper_position"), json!(0.0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        for step in 0..6 {
            let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(result["freeze_stage_one"], json!(step == 5));
            assert_eq!(
                result["freeze_stage_one"].as_bool().unwrap(),
                fast["freeze_stage_one"].as_f64().unwrap() > 0.5
            );
            assert_eq!(
                result["freeze_hot_water_request"].as_i64().unwrap() as f64,
                fast["freeze_hot_water_request"].as_f64().unwrap()
            );
            if step == 5 {
                assert_eq!(result["freeze_hot_water_request"], json!(2));
                assert_eq!(result["outdoor_air_fraction"], json!(0.2));
                assert!(result["heating_coil_valve_position"].as_f64().unwrap() > 0.0);
            }
        }
        inputs.insert("actual_supply_air_temp".into(), json!(8.0));
        for step in 0..6 {
            let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(result["freeze_stage_one"], json!(step < 5));
            assert_eq!(
                result["freeze_stage_one"].as_bool().unwrap(),
                fast["freeze_stage_one"].as_f64().unwrap() > 0.5
            );
            assert_eq!(result["outdoor_air_fraction"], fast["outdoor_air_fraction"]);
        }
        let mut edited = ahu.clone();
        edited
            .blocks
            .iter_mut()
            .find(|block| block.name == "freeze_stage_one_on")
            .unwrap()
            .parameters[0]
            .value = json!(2.0);
        inputs.insert("actual_supply_air_temp".into(), json!(3.5));
        let mut tuned = Executor::default();
        for _ in 0..6 {
            assert_eq!(
                tuned.execute(&edited, &inputs, 60.0, "").unwrap()["freeze_stage_one"],
                json!(false)
            );
        }
    }
    #[test]
    fn g36_ahu_freeze_stage_two_interrupts_ventilation_for_one_hour() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        assert!(validate(&building).is_empty(), "{:?}", validate(&building));
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(18.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(20.0)),
            ("actual_supply_air_temp".into(), json!(3.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_damper_position"), json!(0.0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        for step in 0..6 {
            let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(result["freeze_stage_two"], json!(step == 5));
            assert_eq!(
                result["freeze_stage_two"].as_bool().unwrap(),
                fast["freeze_stage_two"].as_f64().unwrap() > 0.5
            );
            if step == 5 {
                assert_eq!(result["outdoor_air_fraction"], json!(0.0));
                assert_eq!(result["freeze_ventilation_alarm_level"], json!(3));
            }
        }
        inputs.insert("actual_supply_air_temp".into(), json!(4.0));
        let mut released_at = None;
        for step in 1..=65 {
            let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(result["outdoor_air_fraction"], fast["outdoor_air_fraction"]);
            if result["freeze_stage_two"] == json!(false) {
                released_at.get_or_insert(step);
            } else {
                assert_eq!(result["outdoor_air_fraction"], json!(0.0));
                assert_eq!(result["freeze_ventilation_alarm_level"], json!(3));
            }
        }
        assert_eq!(released_at, Some(60));
        let recovered = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        assert_eq!(recovered["freeze_stage_one"], json!(true));
        assert_eq!(recovered["outdoor_air_fraction"], json!(0.2));
        assert_eq!(recovered["freeze_ventilation_alarm_level"], json!(0));
        let mut tuned = ahu.clone();
        tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == "freeze_stage_two_on")
            .unwrap()
            .parameters[0]
            .value = json!(2.0);
        let mut executor = Executor::default();
        inputs.insert("actual_supply_air_temp".into(), json!(3.0));
        for _ in 0..6 {
            assert_eq!(
                executor.execute(&tuned, &inputs, 60.0, "").unwrap()["freeze_stage_two"],
                json!(false)
            );
        }
        let mut fine = Executor::default();
        for _ in 0..=20 {
            fine.execute(ahu, &inputs, 15.0, "").unwrap();
        }
        inputs.insert("actual_supply_air_temp".into(), json!(4.0));
        let mut fine_release = None;
        for step in 1..=244 {
            if fine.execute(ahu, &inputs, 15.0, "").unwrap()["freeze_stage_two"] == json!(false) {
                fine_release.get_or_insert(step);
            }
        }
        assert_eq!(fine_release, Some(240));
    }
    #[test]
    fn g36_ahu_freeze_stage_three_latches_shutdown_until_safe_reset() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        assert!(validate(&building).is_empty(), "{:?}", validate(&building));
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(18.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(20.0)),
            ("actual_supply_air_temp".into(), json!(8.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(true)),
            ("supply_fan_proven".into(), json!(true)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_damper_position"), json!(0.0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(result["freeze_stage_three"], json!(true));
        assert_eq!(fast["freeze_stage_three"], json!(1.0));
        assert_eq!(result["fan_speed"], json!(0.0));
        assert_eq!(result["outdoor_air_fraction"], json!(0.0));
        assert_eq!(result["cooling_coil_valve_position"], json!(1.0));
        assert!(result["heating_coil_valve_position"].as_f64().unwrap() > 0.0);
        assert_eq!(result["freeze_hot_water_request"], json!(2));
        assert_eq!(result["freeze_chilled_water_request"], json!(true));
        assert_eq!(result["freeze_alarm_level"], json!(2));
        inputs.insert("freeze_reset".into(), json!(true));
        assert_eq!(
            dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
            json!(true)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
            json!(1.0)
        );
        inputs.insert("freeze_reset".into(), json!(false));
        inputs.insert("freeze_stat_trip".into(), json!(false));
        for _ in 0..3 {
            assert_eq!(
                dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
                json!(true)
            );
            assert_eq!(
                compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
                json!(1.0)
            );
        }
        inputs.insert("freeze_reset".into(), json!(true));
        assert_eq!(
            dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
            json!(false)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
            json!(0.0)
        );
    }

    #[test]
    fn g36_ahu_freeze_stage_three_uses_both_timed_low_temperature_triggers() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        for (temperature, trigger_step) in [(3.0, 16), (0.5, 6)] {
            let mut inputs = HashMap::from([
                ("outdoor_c".into(), json!(18.0)),
                ("flow_request".into(), json!(5.0)),
                ("return_air_temp".into(), json!(24.0)),
                ("mixed_air_temp".into(), json!(20.0)),
                ("actual_supply_air_temp".into(), json!(temperature)),
                ("duct_pressure".into(), json!(0.4)),
                ("outdoor_airflow".into(), json!(0.0)),
                ("freeze_stat_trip".into(), json!(false)),
                ("supply_fan_proven".into(), json!(true)),
                ("occupied_mode".into(), json!(true)),
                ("freeze_reset".into(), json!(false)),
            ]);
            add_ahu_ventilation_inputs(&mut inputs);
            for zone in 1..=5 {
                inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
                inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(0.0));
                inputs.insert(format!("zone_{zone:02}_damper_position"), json!(0.0));
            }
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
            for step in 1..=trigger_step {
                let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
                let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
                assert_eq!(result["freeze_stage_three"], json!(step == trigger_step));
                assert_eq!(
                    fast["freeze_stage_three"],
                    json!(if step == trigger_step { 1.0 } else { 0.0 })
                );
            }
            inputs.insert("freeze_reset".into(), json!(true));
            assert_eq!(
                dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
                json!(true)
            );
            inputs.insert("actual_supply_air_temp".into(), json!(8.0));
            assert_eq!(
                dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
                json!(false)
            );
        }
    }

    #[test]
    fn g36_simulation_injects_raw_freeze_stat_and_operator_reset() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 18.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut sim = Simulation::new(graph, &weather,
            r#"{"freeze_test_ahu":1,"freeze_test_start_hour":0,"freeze_test_duration_hours":0.25,"freeze_test_reset_hour":0.5}"#).unwrap();
        let first = "FourPipeG36Controls|output.ahu_1_freeze_stage_three";
        let second = "FourPipeG36Controls|output.ahu_2_freeze_stage_three";
        sim.set_traces(&json!([first, second]).to_string()).unwrap();
        let result: Value = serde_json::from_str(&sim.step_chunk(45).unwrap()).unwrap();
        let rows = result["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["traces"][first], json!(1.0));
        assert_eq!(rows[0]["traces"][second], json!(0.0));
        assert_eq!(rows[0]["chilled_water_enable"], json!(1.0));
        assert_eq!(rows[1]["traces"][first], json!(1.0));
        assert_eq!(rows[2]["traces"][first], json!(0.0));
    }

    #[test]
    fn g36_supply_air_reset_responds_to_vav_cooling_requests_in_cdl() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(0.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(20.0)),
            ("actual_supply_air_temp".into(), json!(16.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_damper_position"), json!(0.0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        let mut low = HashMap::new();
        for _ in 0..90 {
            low = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(fast["supply_air_temp"], low["supply_air_temp"]);
        }
        assert_eq!(low["sat_request_count"], json!(0));
        let warm_target = low["supply_air_temp"].as_f64().unwrap();
        assert_eq!(warm_target, 18.0);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(1.0));
            inputs.insert(format!("zone_{zone:02}_damper_position"), json!(1.0));
        }
        let mut high = HashMap::new();
        for _ in 0..90 {
            high = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(fast["sat_request_count"], high["sat_request_count"]);
            assert_eq!(fast["supply_air_temp"], high["supply_air_temp"]);
        }
        assert_eq!(high["sat_request_count"], json!(5));
        assert!(high["supply_air_temp"].as_f64().unwrap() < warm_target - 1.0);
        let mut edited = ahu.clone();
        edited
            .blocks
            .iter_mut()
            .find(|block| block.name == "sat_response_gain")
            .unwrap()
            .parameters[0]
            .value = json!(0.0);
        let mut without_response = Executor::default();
        let mut neutral = HashMap::new();
        for _ in 0..90 {
            neutral = without_response
                .execute(&edited, &inputs, 60.0, "")
                .unwrap();
        }
        assert_eq!(neutral["supply_air_temp"], json!(18.0));
    }
    #[test]
    fn g36_supply_air_reset_edit_changes_physical_delivery() {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 20.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut graph: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let mut reset = Simulation::new(
            &graph.to_string(),
            &weather,
            "{\"zone_capacity_kwh_per_k\":50}",
        )
        .unwrap();
        reset.temps = [27.0; 10];
        let reset_rows = run_weather_intervals(&mut reset, 48);
        for ahu in graph["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|block| block["block_type"] == "AHUController")
        {
            ahu["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "sat_response_gain")
                .unwrap()["parameters"][0]["value"] = json!(0.0);
        }
        let mut neutral = Simulation::new(
            &graph.to_string(),
            &weather,
            "{\"zone_capacity_kwh_per_k\":50}",
        )
        .unwrap();
        neutral.temps = [27.0; 10];
        let neutral_rows = run_weather_intervals(&mut neutral, 48);
        let minimum = |rows: &Value, name: &str| {
            rows["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row[name].as_f64().unwrap())
                .fold(f64::INFINITY, f64::min)
        };
        assert!(
            minimum(&reset_rows, "ahu_1_supply_setpoint_c") < 15.5,
            "minimum reset target {}",
            minimum(&reset_rows, "ahu_1_supply_setpoint_c")
        );
        assert!(neutral_rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["ahu_1_supply_setpoint_c"].as_f64().unwrap() >= 16.0));
        assert!(
            minimum(&reset_rows, "ahu_1_supply_actual_c")
                < minimum(&neutral_rows, "ahu_1_supply_actual_c") - 0.01
        );
    }

    #[test]
    fn g36_water_reset_holds_for_source_stage_changes_in_both_executors() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut inputs: HashMap<String, Value> = plant
            .inputs
            .iter()
            .map(|port| {
                let value = match port.kind.as_str() {
                    "Boolean" => json!(false),
                    "Integer" => json!(0),
                    _ => json!(0.0),
                };
                (port.name.clone(), value)
            })
            .collect();
        set_equipment_ready(&mut inputs);
        for (name, value) in [
            ("cool_request_kw", 100.0),
            ("heat_pump_available_capacity_kw", 80.0),
            ("chilled_water_flow_kg_s", 2.0),
            ("chilled_water_supply_temp_c", 7.0),
            ("ahu_1_supply_setpoint_c", 16.0),
            ("ahu_2_supply_setpoint_c", 16.0),
            ("ahu_1_supply_actual_c", 16.0),
            ("ahu_2_supply_actual_c", 16.0),
        ] {
            inputs.insert(name.into(), json!(value));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        let mut last = 1.0;
        for step in 0..30 {
            let row = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                row["chilled_water_reset_fraction"],
                fast["chilled_water_reset_fraction"]
            );
            last = row["chilled_water_reset_fraction"].as_f64().unwrap();
            if step < 14 {
                assert_eq!(last, 1.0);
            }
        }
        assert!(
            last < 1.0,
            "reset should trim after the 900-second stage hold"
        );
        inputs.insert("cool_request_kw".into(), json!(10.0));
        inputs.insert("chilled_water_supply_temp_c".into(), json!(4.0));
        let mut held = None;
        for _step in 0..10 {
            let row = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                row["chilled_water_reset_fraction"],
                fast["chilled_water_reset_fraction"]
            );
            let current = row["chilled_water_reset_fraction"].as_f64().unwrap();
            if let Some(previous) = held {
                assert_eq!(
                    current, previous,
                    "source-stage change should hold the previous sampled reset"
                );
            }
            held = Some(current);
        }
        let mut tuned = plant.clone();
        tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == "chilled_stage_hold_delay")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "delayTime")
            .unwrap()
            .value = json!(120.0);
        let mut edited = Executor::default();
        let mut tuned_inputs = inputs.clone();
        tuned_inputs.insert("cool_request_kw".into(), json!(100.0));
        tuned_inputs.insert("chilled_water_supply_temp_c".into(), json!(7.0));
        let mut early = 1.0;
        for _ in 0..10 {
            early = edited.execute(&tuned, &tuned_inputs, 60.0, "").unwrap()
                ["chilled_water_reset_fraction"]
                .as_f64()
                .unwrap();
        }
        assert!(
            early < 1.0,
            "editing the CDL hold time should release reset sooner"
        );
    }
    #[test]
    fn g36_water_reset_waits_for_physical_source_running_proof() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut inputs: HashMap<String, Value> = plant
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else {
                        json!(0.0)
                    },
                )
            })
            .collect();
        set_equipment_ready(&mut inputs);
        for (name, value) in [
            ("heat_request_kw", 100.0),
            ("heat_pump_available_capacity_kw", 80.0),
            ("hot_water_flow_kg_s", 2.0),
            ("hot_water_supply_temp_c", 45.0),
        ] {
            inputs.insert(name.into(), json!(value));
        }
        inputs.insert("heat_pump_running".into(), json!(false));
        let trace = "plant|hot_stage_hold_delay.y".to_string();
        let mut dynamic = Executor::default();
        dynamic.trace_targets = vec![trace.clone()];
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        compiled.set_traces(&[trace.clone()]).unwrap();
        let active = |value: &Value| {
            value
                .as_bool()
                .unwrap_or_else(|| value.as_f64().unwrap() != 0.0)
        };
        for _ in 0..20 {
            dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let (_, fast_traces) = compiled.execute(&inputs, 60.0, true).unwrap();
            assert!(!active(&dynamic.trace_values[&trace]));
            assert_eq!(
                active(&dynamic.trace_values[&trace]),
                active(&fast_traces[&trace])
            );
        }
        inputs.insert("heat_pump_running".into(), json!(true));
        let mut released = false;
        for _ in 0..25 {
            dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let (_, fast_traces) = compiled.execute(&inputs, 60.0, true).unwrap();
            released = active(&dynamic.trace_values[&trace]);
            assert_eq!(released, active(&fast_traces[&trace]));
        }
        assert!(
            released,
            "the sampled reset should release after physical source proof and its CDL hold time"
        );
    }
    #[test]
    fn g36_chilled_water_reset_responds_to_ahu_requests_in_cdl() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut inputs = HashMap::from([
            ("heat_request_kw".into(), json!(0.0)),
            ("cool_request_kw".into(), json!(100.0)),
            ("heat_pump_available_capacity_kw".into(), json!(80.0)),
            ("hot_water_supply_temp_c".into(), json!(45.0)),
            ("hot_water_return_temp_c".into(), json!(40.0)),
            ("chilled_water_supply_temp_c".into(), json!(7.0)),
            ("chilled_water_return_temp_c".into(), json!(12.0)),
            ("hot_water_flow_kg_s".into(), json!(0.0)),
            ("chilled_water_flow_kg_s".into(), json!(2.0)),
            ("hot_water_differential_pressure_pa".into(), json!(0.0)),
            (
                "chilled_water_differential_pressure_pa".into(),
                json!(25_000.0),
            ),
            ("ahu_1_cooling_valve".into(), json!(0.0)),
            ("ahu_2_cooling_valve".into(), json!(0.0)),
            ("ahu_1_heating_valve".into(), json!(0.0)),
            ("ahu_2_heating_valve".into(), json!(0.0)),
            ("ahu_1_supply_setpoint_c".into(), json!(16.0)),
            ("ahu_2_supply_setpoint_c".into(), json!(16.0)),
            ("ahu_1_supply_actual_c".into(), json!(16.0)),
            ("ahu_2_supply_actual_c".into(), json!(16.0)),
            ("ahu_1_freeze_hot_water_request".into(), json!(0)),
            ("ahu_2_freeze_hot_water_request".into(), json!(0)),
            ("ahu_1_freeze_chilled_water_request".into(), json!(false)),
            ("ahu_2_freeze_chilled_water_request".into(), json!(false)),
        ]);
        set_equipment_ready(&mut inputs);
        for zone in 1..=10 {
            inputs.insert(format!("zone_{zone:02}_reheat_valve"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_heating_demand"), json!(0.0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        let mut low_request = HashMap::new();
        for _ in 0..90 {
            low_request = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                fast["chilled_water_supply_setpoint_c"],
                low_request["chilled_water_supply_setpoint_c"]
            );
            assert_eq!(
                fast["chilled_water_reset_fraction"],
                low_request["chilled_water_reset_fraction"]
            );
        }
        let warm_target = low_request["chilled_water_supply_setpoint_c"]
            .as_f64()
            .unwrap();
        let low_reset = low_request["chilled_water_reset_fraction"]
            .as_f64()
            .unwrap();
        let expected_low_target = 7.0 + 5.0 * (1.0 - (2.0 * (low_reset - 0.5).max(0.0)));
        assert!((warm_target - expected_low_target).abs() < 1e-9);
        assert!(
            (low_request["chilled_dp_setpoint_pa"].as_f64().unwrap()
                - (15_000.0 + 40_000.0 * low_reset.min(0.5)))
            .abs()
                < 1e-9
        );
        assert!(
            warm_target > 7.0,
            "reset should warm the water with no AHU requests"
        );
        inputs.insert("ahu_1_cooling_valve".into(), json!(1.0));
        inputs.insert("ahu_2_cooling_valve".into(), json!(1.0));
        inputs.insert("ahu_1_supply_actual_c".into(), json!(24.0));
        inputs.insert("ahu_2_supply_actual_c".into(), json!(24.0));
        let mut high_request = HashMap::new();
        for _ in 0..90 {
            high_request = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                fast["chilled_water_supply_setpoint_c"],
                high_request["chilled_water_supply_setpoint_c"]
            );
        }
        assert!(
            high_request["chilled_water_supply_setpoint_c"]
                .as_f64()
                .unwrap()
                < warm_target
        );
        let high_reset = high_request["chilled_water_reset_fraction"]
            .as_f64()
            .unwrap();
        let expected_high_target = 7.0 + 5.0 * (1.0 - (2.0 * (high_reset - 0.5).max(0.0)));
        assert!(
            (high_request["chilled_water_supply_setpoint_c"]
                .as_f64()
                .unwrap()
                - expected_high_target)
                .abs()
                < 1e-9
        );
        assert!(
            high_request["chilled_water_reset_fraction"]
                .as_f64()
                .unwrap()
                > low_request["chilled_water_reset_fraction"]
                    .as_f64()
                    .unwrap()
        );
        let mut first_half_plant = plant.clone();
        first_half_plant
            .blocks
            .iter_mut()
            .find(|block| block.name == "chilled_reset_previous")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "y_start")
            .unwrap()
            .value = json!(0.25);
        first_half_plant
            .blocks
            .iter_mut()
            .find(|block| block.name == "chilled_reset_held_value")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "y_start")
            .unwrap()
            .value = json!(0.25);
        let first_half = Executor::default()
            .execute(&first_half_plant, &inputs, 60.0, "")
            .unwrap();
        assert_eq!(first_half["chilled_water_supply_setpoint_c"], json!(12.0));
        assert_eq!(first_half["chilled_dp_setpoint_pa"], json!(25_000.0));
    }
    #[test]
    fn g36_hot_water_reset_responds_to_ahu_requests_in_cdl() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut inputs = HashMap::from([
            ("heat_request_kw".into(), json!(100.0)),
            ("cool_request_kw".into(), json!(0.0)),
            ("heat_pump_available_capacity_kw".into(), json!(80.0)),
            ("hot_water_supply_temp_c".into(), json!(45.0)),
            ("hot_water_return_temp_c".into(), json!(40.0)),
            ("chilled_water_supply_temp_c".into(), json!(7.0)),
            ("chilled_water_return_temp_c".into(), json!(12.0)),
            ("hot_water_flow_kg_s".into(), json!(2.0)),
            ("chilled_water_flow_kg_s".into(), json!(0.0)),
            ("hot_water_differential_pressure_pa".into(), json!(25_000.0)),
            ("chilled_water_differential_pressure_pa".into(), json!(0.0)),
            ("ahu_1_cooling_valve".into(), json!(0.0)),
            ("ahu_2_cooling_valve".into(), json!(0.0)),
            ("ahu_1_heating_valve".into(), json!(0.0)),
            ("ahu_2_heating_valve".into(), json!(0.0)),
            ("ahu_1_supply_setpoint_c".into(), json!(16.0)),
            ("ahu_2_supply_setpoint_c".into(), json!(16.0)),
            ("ahu_1_supply_actual_c".into(), json!(16.0)),
            ("ahu_2_supply_actual_c".into(), json!(16.0)),
            ("ahu_1_freeze_hot_water_request".into(), json!(0)),
            ("ahu_2_freeze_hot_water_request".into(), json!(0)),
            ("ahu_1_freeze_chilled_water_request".into(), json!(false)),
            ("ahu_2_freeze_chilled_water_request".into(), json!(false)),
        ]);
        set_equipment_ready(&mut inputs);
        for zone in 1..=10 {
            inputs.insert(format!("zone_{zone:02}_reheat_valve"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_heating_demand"), json!(0.0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        let mut low = HashMap::new();
        for _ in 0..90 {
            low = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                fast["hot_water_supply_setpoint_c"],
                low["hot_water_supply_setpoint_c"]
            );
        }
        let low_target = low["hot_water_supply_setpoint_c"].as_f64().unwrap();
        assert!(
            low_target < 45.0,
            "no requests should trim the hot-water target"
        );
        inputs.insert("heat_request_kw".into(), json!(0.0));
        inputs.insert("ahu_1_freeze_hot_water_request".into(), json!(2));
        let mut freeze = HashMap::new();
        for _ in 0..6 {
            freeze = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                fast["hot_water_supply_setpoint_c"],
                freeze["hot_water_supply_setpoint_c"]
            );
        }
        assert_eq!(freeze["hot_water_enable"], json!(true));
        assert!(freeze["hot_water_pump_speed"].as_f64().unwrap() >= 0.15);
        assert!(freeze["hot_water_supply_setpoint_c"].as_f64().unwrap() >= 45.0);
        inputs.insert("ahu_1_freeze_chilled_water_request".into(), json!(true));
        let chilled = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let chilled_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(chilled["chilled_water_enable"], json!(true));
        assert!(chilled["chilled_water_pump_speed"].as_f64().unwrap() >= 0.15);
        assert_eq!(chilled_fast["chilled_water_enable"], json!(1.0));
        inputs.insert("ahu_1_freeze_chilled_water_request".into(), json!(false));
        inputs.insert("heat_request_kw".into(), json!(100.0));
        inputs.insert("ahu_1_freeze_hot_water_request".into(), json!(0));
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_reheat_valve"), json!(1.0));
            inputs.insert(format!("zone_{zone:02}_heating_demand"), json!(1.0));
        }
        let mut terminal = HashMap::new();
        for step in 0..90 {
            terminal = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                fast["terminal_hot_water_request_count"],
                terminal["terminal_hot_water_request_count"],
                "step {step}"
            );
            assert_eq!(
                fast["hot_water_supply_setpoint_c"],
                terminal["hot_water_supply_setpoint_c"]
            );
        }
        assert_eq!(terminal["terminal_hot_water_request_count"], json!(5));
        assert!(terminal["hot_water_supply_setpoint_c"].as_f64().unwrap() > low_target + 1.0);
        let mut disabled_terminal = plant.clone();
        disabled_terminal
            .blocks
            .iter_mut()
            .find(|block| block.name == "terminal_heat_demand_threshold")
            .unwrap()
            .parameters[0]
            .value = json!(1.1);
        let mut edited = Executor::default();
        let mut suppressed = HashMap::new();
        for _ in 0..10 {
            suppressed = edited
                .execute(&disabled_terminal, &inputs, 60.0, "")
                .unwrap();
        }
        assert_eq!(suppressed["terminal_hot_water_request_count"], json!(0));
        inputs.insert("ahu_1_heating_valve".into(), json!(1.0));
        inputs.insert("ahu_2_heating_valve".into(), json!(1.0));
        inputs.insert("ahu_1_supply_actual_c".into(), json!(-2.0));
        inputs.insert("ahu_2_supply_actual_c".into(), json!(-2.0));
        let mut high = HashMap::new();
        for _ in 0..90 {
            high = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                fast["hot_water_supply_setpoint_c"],
                high["hot_water_supply_setpoint_c"]
            );
        }
        assert!(high["hot_water_supply_setpoint_c"].as_f64().unwrap() > low_target + 1.0);
        assert!(
            high["hot_water_reset_fraction"].as_f64().unwrap()
                > low["hot_water_reset_fraction"].as_f64().unwrap()
        );
    }
    #[test]
    fn g36_hot_water_reset_edit_reaches_physical_supply_sensor() {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 0.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut graph: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let mut reset = Simulation::new(&graph.to_string(), &weather, "{}").unwrap();
        let reset_rows = run_weather_intervals(&mut reset, 48);
        let plant = graph["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "plant")
            .unwrap();
        plant["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "hot_reset_warm_delta")
            .unwrap()["parameters"][0]["value"] = json!(0.0);
        let mut fixed = Simulation::new(&graph.to_string(), &weather, "{}").unwrap();
        let fixed_rows = run_weather_intervals(&mut fixed, 48);
        assert!(fixed_rows["rows"].as_array().unwrap().iter().all(|row| row
            ["hot_water_supply_setpoint_c"]
            .as_f64()
            .unwrap()
            == 45.0));
        let reset_target = reset_rows["rows"].as_array().unwrap().last().unwrap()
            ["hot_water_supply_setpoint_c"]
            .as_f64()
            .unwrap();
        let reset_supply = reset_rows["rows"].as_array().unwrap().last().unwrap()
            ["hot_water_supply_temp_c"]
            .as_f64()
            .unwrap();
        let fixed_supply = fixed_rows["rows"].as_array().unwrap().last().unwrap()
            ["hot_water_supply_temp_c"]
            .as_f64()
            .unwrap();
        assert!(
            reset_target < 45.0,
            "low request should trim the CDL hot-water target"
        );
        assert!(
            reset_supply < fixed_supply - 0.01,
            "graph reset should change physical supply: {reset_supply} < {fixed_supply}"
        );
    }
    #[test]
    fn g36_water_reset_edit_reaches_physical_supply_sensor() {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 26.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut graph: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        for ahu in graph["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|block| block["block_type"] == "AHUController")
        {
            ahu["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "cooling_supply")
                .unwrap()["parameters"][0]["value"] = json!(18.0);
        }
        let mut normal = Simulation::new(&graph.to_string(), &weather, "{}").unwrap();
        let normal_rows = run_weather_intervals(&mut normal, 48);
        let normal_supply = normal_rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["chilled_water_supply_temp_c"].as_f64().unwrap())
            .fold(f64::NEG_INFINITY, f64::max);
        let plant = graph["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "plant")
            .unwrap();
        plant["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "chilled_reset_warm_delta")
            .unwrap()["parameters"][0]["value"] = json!(0.0);
        let mut fixed = Simulation::new(&graph.to_string(), &weather, "{}").unwrap();
        let fixed_rows = run_weather_intervals(&mut fixed, 48);
        assert!(
            normal_supply > 7.0,
            "the sampled CDL reset should warm physical supply water"
        );
        assert!(fixed_rows["rows"].as_array().unwrap().iter().all(|row| row
            ["chilled_water_supply_setpoint_c"]
            .as_f64()
            .unwrap()
            == 7.0));
        let fixed_supply = fixed_rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["chilled_water_supply_temp_c"].as_f64().unwrap())
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            normal_supply > fixed_supply + 0.01,
            "graph reset should change physical supply: {normal_supply} > {fixed_supply}"
        );
    }
    #[test]
    fn g36_building_exercises_zone_pressure_requests() {
        let mut graph: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        for ahu in graph["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|b| b["block_type"] == "AHUController")
        {
            let pi = ahu["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|b| b["name"] == "duct_pressure_pi")
                .unwrap();
            pi["parameters"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|p| p["name"] == "yMax")
                .unwrap()["value"] = json!(0.05);
            let limit = ahu["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|b| b["name"] == "fan_limit")
                .unwrap();
            limit["parameters"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|p| p["name"] == "uMax")
                .unwrap()["value"] = json!(0.05);
        }
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 30.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let mut sim = Simulation::new(&graph.to_string(), &weather, "{}").unwrap();
        sim.temps = [30.0; 10];
        let target = "FourPipeG36Controls|output.ahu_1_pressure_request_count";
        sim.set_traces(&json!([target]).to_string()).unwrap();
        let mut highest = 0.0_f64;
        while sim.cursor < 96 * (900 / sim.effective_step_seconds) {
            let result: Value = serde_json::from_str(&sim.step_chunk(672).unwrap()).unwrap();
            for row in result["rows"].as_array().unwrap() {
                highest = highest.max(row["traces"][target].as_f64().unwrap_or(0.0));
            }
        }
        assert!(
            highest > 0.0,
            "the physical example never exercised pressure reset requests"
        );
    }
    #[test]
    fn divide_block_matches_compiled_executor() {
        let block: Block = serde_json::from_value(json!({
            "name":"quotient","block_type":"Buildings.Controls.OBC.CDL.Reals.Divide",
            "inputs":[{"name":"u1","type":"Real"},{"name":"u2","type":"Real"}],
            "outputs":[{"name":"y","type":"Real"}]
        }))
        .unwrap();
        let inputs = HashMap::from([("u1".into(), json!(3.0)), ("u2".into(), json!(4.0))]);
        let dynamic = Executor::default()
            .execute(&block, &inputs, 60.0, "")
            .unwrap();
        let mut compiled = CompiledExecutor::try_new(&block).unwrap().unwrap();
        let (fast, _) = compiled.execute(&inputs, 60.0, false).unwrap();
        assert_eq!(dynamic["y"], json!(0.75));
        assert_eq!(fast["y"], dynamic["y"]);
    }
    #[test]
    fn reference_limiter() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/p_controller_limiter.json"
        ))
        .unwrap();
        assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
        let mut ex = Executor::default();
        let result = ex
            .execute(
                &graph,
                &HashMap::from([("e".into(), json!(5.0)), ("yMax".into(), json!(20.0))]),
                900.0,
                "",
            )
            .unwrap();
        assert_eq!(result.get("y"), Some(&json!(20.0)));
    }
    #[test]
    fn all_examples_parse_and_validate() {
        for text in [
            include_str!("../../web/public/examples/four_pipe_g36.json"),
            include_str!("../../web/public/examples/east_controller.json"),
            include_str!("../../web/public/examples/west_controller.json"),
            include_str!("../../web/public/examples/north_controller.json"),
            include_str!("../../web/public/examples/south_controller.json"),
            include_str!("../../web/public/examples/corridor_controller.json"),
            include_str!("../../web/public/examples/room_control_system.json"),
            include_str!("../../web/public/examples/simple_temp_controller.json"),
            include_str!("../../web/public/examples/basic_temp_control.json"),
        ] {
            let graph: Block = serde_json::from_str(text).unwrap();
            assert!(
                validate(&graph).is_empty(),
                "{} {:?}",
                graph.name,
                validate(&graph)
            );
        }
    }
    #[test]
    fn g36_sample_uses_debounced_presence_for_setpoint_modes() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let mut weather = vec![
            Weather {
                outdoor_c: 22.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ];
        for sample in &mut weather[70..87] {
            sample.occupied = false;
        }
        let weather = serde_json::to_string(&weather).unwrap();
        let mut sim = Simulation::new(graph, &weather, r#"{"step_seconds":15}"#).unwrap();
        let trace = "FourPipeG36Controls.zone_01|cool_setpoint_active.y";
        sim.set_traces(&serde_json::to_string(&vec![trace]).unwrap())
            .unwrap();
        while sim.cursor < 31 * 60 {
            sim.step_chunk((31 * 60 - sim.cursor).min(672)).unwrap();
        }
        sim.step_chunk(3).unwrap();
        assert!(
            !sim.zone_occupied[0],
            "presence must persist for one minute"
        );
        sim.step_chunk(2).unwrap();
        assert!(sim.zone_occupied[0]);
        let mut values = HashMap::new();
        while sim.cursor < 71 * 60 {
            let chunk: Value = serde_json::from_str(&sim.step_chunk(672).unwrap()).unwrap();
            for row in chunk["rows"].as_array().unwrap() {
                let index = row["index"].as_u64().unwrap();
                if [31, 48, 70].contains(&index) {
                    values.insert(index, row.clone());
                }
            }
        }
        assert_eq!(values[&31]["zone_occupied"][0], 1.0);
        assert_eq!(values[&31]["traces"][trace], 24.0);
        assert_eq!(values[&48]["zone_occupied"][0], 0.0);
        assert_eq!(values[&48]["traces"][trace], 24.5);
        assert_eq!(values[&70]["schedule_occupied"], 0.0);
        assert_eq!(values[&70]["preconditioning_mode"], 0.0);
        assert_eq!(values[&70]["traces"][trace], 80.0);
    }
    #[test]
    fn g36_vacancy_requires_five_continuous_minutes() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 22.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut sim = Simulation::new(graph, &weather, r#"{"step_seconds":15}"#).unwrap();
        while sim.cursor < 48 * 60 {
            sim.step_chunk((48 * 60 - sim.cursor).min(672)).unwrap();
        }
        assert!(sim.zone_occupied[0]);
        sim.step_chunk(1).unwrap();
        assert!(!sim.raw_occupancy[0]);
        sim.step_chunk(18).unwrap();
        assert!(sim.zone_occupied[0]);
        sim.step_chunk(2).unwrap();
        assert!(!sim.zone_occupied[0]);
    }
    #[test]
    fn g36_timing_decisions_change_when_cdl_parameters_change() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let mut weather = vec![
            Weather {
                outdoor_c: 22.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ];
        for sample in &mut weather[70..87] {
            sample.occupied = false;
        }
        let weather = serde_json::to_string(&weather).unwrap();
        let mut reference = Simulation::new(graph, &weather, "{}").unwrap();
        reference.cursor = 80 * (900 / reference.effective_step_seconds);
        reference.temps = [19.0; 10];
        let reference_start = run_weather_intervals(&mut reference, 1);
        let mut reference = Simulation::new(graph, &weather, "{}").unwrap();
        let reference = run_weather_intervals(&mut reference, 34);
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        let horizon = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "preconditioning_horizon")
            .unwrap();
        horizon["parameters"][0]["value"] = json!(1.0);
        let zone = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "zone_01")
            .unwrap();
        let arrival = zone["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "presence_delay")
            .unwrap();
        arrival["parameters"][0]["value"] = json!(1800.0);
        let mut changed = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        changed.cursor = 80 * (900 / changed.effective_step_seconds);
        changed.temps = [19.0; 10];
        let changed_start = run_weather_intervals(&mut changed, 1);
        let mut changed = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        let changed = run_weather_intervals(&mut changed, 34);
        assert_eq!(reference["rows"][31]["zone_occupied"][0], 1.0);
        assert_eq!(changed["rows"][31]["zone_occupied"][0], 0.0);
        assert_eq!(changed["rows"][33]["zone_occupied"][0], 1.0);
        assert_eq!(reference_start["rows"][0]["preconditioning_mode"], 1.0);
        assert_eq!(changed_start["rows"][0]["preconditioning_mode"], 0.0);
    }
    #[test]
    fn compiled_g36_timing_matches_dynamic_executor() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = include_str!("../../web/public/examples/weather_2025.json");
        let mut fast = Simulation::new(graph, weather, "{}").unwrap();
        assert!(fast.compiled.is_some());
        let mut dynamic = Simulation::new(graph, weather, "{}").unwrap();
        dynamic.compiled = None;
        let fast = run_weather_intervals(&mut fast, 96);
        let dynamic = run_weather_intervals(&mut dynamic, 96);
        for (a, b) in fast["rows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(dynamic["rows"].as_array().unwrap())
        {
            assert_eq!(a["preconditioning_mode"], b["preconditioning_mode"]);
            assert_eq!(a["zone_occupied"], b["zone_occupied"]);
            for point in [
                "hot_water_pump_speed",
                "chilled_water_pump_speed",
                "hot_water_differential_pressure_pa",
                "chilled_water_differential_pressure_pa",
            ] {
                assert!(
                    (a[point].as_f64().unwrap() - b[point].as_f64().unwrap()).abs() < 1e-9,
                    "{point}: compiled={}, dynamic={}",
                    a[point],
                    b[point]
                );
            }
            assert!(
                (a["electric_kw"].as_f64().unwrap() - b["electric_kw"].as_f64().unwrap()).abs()
                    < 1e-9
            );
        }
    }
    #[test]
    fn conventional_outdoor_air_command_is_graph_owned() {
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 30.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        let base: Value = serde_json::from_str(&normal.step_chunk(3).unwrap()).unwrap();
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        for controller in edited["blocks"].as_array_mut().unwrap() {
            if controller["block_type"] != "AHUController" {
                continue;
            }
            let constant = controller["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "minimum_outdoor_air")
                .unwrap();
            constant["parameters"][0]["value"] = json!(0.8);
        }
        let mut changed = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        let high: Value = serde_json::from_str(&changed.step_chunk(3).unwrap()).unwrap();
        assert_eq!(base["rows"][2]["ahu_1_outdoor_air_fraction"], 0.2);
        assert_eq!(high["rows"][2]["ahu_1_outdoor_air_fraction"], 0.8);
        assert!(
            high["rows"][2]["ahu_1_mixed_air_c"].as_f64().unwrap()
                > base["rows"][2]["ahu_1_mixed_air_c"].as_f64().unwrap()
        );
    }
    #[test]
    fn g36_terminal_alarms_require_persistent_measured_faults() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut executor = Executor::default();
        let mut inputs = HashMap::from([
            ("room_temp".into(), json!(27.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("zone_presence_raw".into(), json!(true)),
            ("measured_airflow".into(), json!(0.0)),
            ("discharge_temp".into(), json!(40.0)),
        ]);
        for step in 0..6 {
            let outputs = executor.execute(zone, &inputs, 60.0, "").unwrap();
            assert_eq!(outputs["low_airflow_alarm"], json!(step >= 5));
            assert_eq!(outputs["high_discharge_alarm"], json!(step >= 5));
        }
        inputs.insert("measured_airflow".into(), json!(1.0));
        inputs.insert("discharge_temp".into(), json!(30.0));
        let recovered = executor.execute(zone, &inputs, 60.0, "").unwrap();
        assert_eq!(recovered["low_airflow_alarm"], false);
        assert_eq!(recovered["high_discharge_alarm"], false);
    }
    #[test]
    fn g36_zone_conditioning_modes_are_cdl_decisions_and_drive_setpoints() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let heat_trace = "zone_01|heat_setpoint_active.y".to_string();
        let cool_trace = "zone_01|cool_setpoint_active.y".to_string();
        for (
            room,
            schedule,
            warmup,
            cooldown,
            setback,
            setup,
            freeze,
            presence,
            mode,
            heat,
            cool,
        ) in [
            (
                22.0, false, false, false, false, false, false, false, 0.0, -50.0, 80.0,
            ),
            (
                15.0, false, false, false, true, false, false, false, 6.0, 18.0, 80.0,
            ),
            (
                33.0, false, false, false, false, true, false, false, 5.0, -50.0, 32.0,
            ),
            (
                3.0, false, false, false, false, false, true, false, 7.0, 18.0, 80.0,
            ),
            (
                19.0, false, true, false, false, false, false, false, 3.0, 21.0, 24.0,
            ),
            (
                27.0, false, false, true, false, false, false, false, 4.0, 21.0, 24.0,
            ),
            (
                22.0, true, false, false, false, false, false, false, 2.0, 20.5, 24.5,
            ),
            (
                22.0, true, false, false, false, false, false, true, 1.0, 21.0, 24.0,
            ),
        ] {
            let inputs = HashMap::from([
                ("room_temp".into(), json!(room)),
                ("supply_air_temp".into(), json!(16.0)),
                ("supply_pressure".into(), json!(1.0)),
                ("scheduled_occupied".into(), json!(schedule)),
                ("group_warmup".into(), json!(warmup)),
                ("group_cooldown".into(), json!(cooldown)),
                ("group_setback".into(), json!(setback)),
                ("group_setup".into(), json!(setup)),
                ("group_freeze_setback".into(), json!(freeze)),
                ("window_closed".into(), json!(true)),
                ("local_override".into(), json!(false)),
                ("zone_presence_raw".into(), json!(presence)),
                ("measured_airflow".into(), json!(0.0)),
                ("discharge_temp".into(), json!(16.0)),
            ]);
            let mut dynamic = Executor::default();
            dynamic.trace_targets = vec![heat_trace.clone(), cool_trace.clone()];
            let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
            let mut actual = HashMap::new();
            for _ in 0..7 {
                actual = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
                let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
                assert_eq!(
                    actual["conditioning_state"].as_f64(),
                    fast["conditioning_state"].as_f64()
                );
            }
            assert_eq!(
                actual["conditioning_state"].as_f64(),
                Some(mode),
                "room={room}, schedule={schedule}, warmup={warmup}, cooldown={cooldown}"
            );
            assert_eq!(dynamic.trace_values[&heat_trace].as_f64(), Some(heat));
            assert_eq!(dynamic.trace_values[&cool_trace].as_f64(), Some(cool));
        }
        let mut tuned = zone.clone();
        tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == "heat_setpoint")
            .unwrap()
            .parameters[0]
            .value = json!(23.0);
        let inputs = HashMap::from([
            ("room_temp".into(), json!(22.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(false)),
            ("group_warmup".into(), json!(true)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("zone_presence_raw".into(), json!(false)),
            ("measured_airflow".into(), json!(0.0)),
            ("discharge_temp".into(), json!(16.0)),
        ]);
        let standard = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        let changed = Executor::default()
            .execute(&tuned, &inputs, 60.0, "")
            .unwrap();
        assert_eq!(standard["conditioning_state"].as_f64(), Some(0.0));
        assert_eq!(changed["conditioning_state"].as_f64(), Some(3.0));
    }
    #[test]
    fn g36_zone_group_optimal_start_is_an_editable_cdl_decision() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("seconds_until_occupied".into(), json!(3600.0)),
            ("lookahead_allowed".into(), json!(true)),
            ("scheduled_occupied".into(), json!(false)),
        ]);
        for zone in 1..=5 {
            inputs.insert(format!("room_{zone}"), json!(19.0));
            inputs.insert(format!("heat_target_{zone}"), json!(21.0));
            inputs.insert(format!("cool_target_{zone}"), json!(24.0));
            inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
            inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
            inputs.insert(format!("window_closed_{zone}"), json!(true));
            inputs.insert(format!("override_{zone}"), json!(false));
        }
        let check = |block: &Block, values: &HashMap<String, Value>| {
            let dynamic = Executor::default()
                .execute(block, values, 60.0, "")
                .unwrap();
            let compiled = CompiledExecutor::try_new(block)
                .unwrap()
                .unwrap()
                .execute(values, 60.0, false)
                .unwrap()
                .0;
            for name in ["warmup", "cooldown", "preconditioning"] {
                assert_eq!(
                    dynamic[name].as_bool(),
                    compiled[name].as_f64().map(|value| value != 0.0)
                );
            }
            dynamic
        };
        let warm = check(group, &inputs);
        assert_eq!(warm["warmup"], true);
        assert_eq!(warm["cooldown"], false);
        inputs.insert("seconds_until_occupied".into(), json!(7200.0));
        assert_eq!(check(group, &inputs)["preconditioning"], false);
        inputs.insert("seconds_until_occupied".into(), json!(3600.0));
        for zone in 1..=5 {
            inputs.insert(format!("room_{zone}"), json!(29.0));
        }
        let cool = check(group, &inputs);
        assert_eq!(cool["cooldown"], true);
        assert_eq!(cool["warmup"], false);
        inputs.insert("seconds_until_occupied".into(), json!(10801.0));
        assert_eq!(check(group, &inputs)["preconditioning"], false);
        inputs.insert("lookahead_allowed".into(), json!(false));
        inputs.insert("seconds_until_occupied".into(), json!(3600.0));
        assert_eq!(check(group, &inputs)["preconditioning"], false);
        let mut tuned = group.clone();
        tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == "cooldown_seconds_per_k")
            .unwrap()
            .parameters[0]
            .value = json!(600.0);
        inputs.insert("lookahead_allowed".into(), json!(true));
        assert_eq!(check(&tuned, &inputs)["preconditioning"], false);
    }
    #[test]
    fn g36_group_setback_setup_and_freeze_latch_in_cdl() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("seconds_until_occupied".into(), json!(86400.0)),
            ("lookahead_allowed".into(), json!(false)),
            ("scheduled_occupied".into(), json!(false)),
        ]);
        for zone in 1..=5 {
            inputs.insert(format!("room_{zone}"), json!(19.0));
            inputs.insert(format!("heat_target_{zone}"), json!(21.0));
            inputs.insert(format!("cool_target_{zone}"), json!(24.0));
            inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
            inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
            inputs.insert(format!("window_closed_{zone}"), json!(true));
            inputs.insert(format!("override_{zone}"), json!(false));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
        for (room, setback, setup, freeze, alarm) in [
            (19.0, false, false, false, 0.0),
            (17.0, true, false, false, 0.0),
            (18.5, true, false, false, 0.0),
            (19.2, false, false, false, 0.0),
            (33.0, false, true, false, 0.0),
            (31.5, false, true, false, 0.0),
            (30.8, false, false, false, 0.0),
            (3.0, false, false, true, 3.0),
            (6.0, false, false, true, 3.0),
            (7.2, true, false, false, 0.0),
        ] {
            for zone in 1..=5 {
                inputs.insert(format!("room_{zone}"), json!(room));
            }
            let result = dynamic.execute(group, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            for (name, expected) in [
                ("setback", setback),
                ("setup", setup),
                ("freeze_setback", freeze),
            ] {
                assert_eq!(result[name], json!(expected), "{name} at {room} °C");
                assert_eq!(fast[name].as_f64(), Some(if expected { 1.0 } else { 0.0 }));
            }
            assert_eq!(result["freeze_alarm_level"].as_f64(), Some(alarm));
            assert_eq!(fast["freeze_alarm_level"].as_f64(), Some(alarm));
        }
    }
    #[test]
    fn g36_window_contact_changes_group_and_terminal_cdl_decisions() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let mut group_inputs = HashMap::from([
            ("seconds_until_occupied".into(), json!(3600.0)),
            ("lookahead_allowed".into(), json!(true)),
            ("scheduled_occupied".into(), json!(false)),
        ]);
        for zone in 1..=5 {
            group_inputs.insert(
                format!("room_{zone}"),
                json!(if zone == 1 { 15.0 } else { 21.0 }),
            );
            group_inputs.insert(format!("heat_target_{zone}"), json!(21.0));
            group_inputs.insert(format!("cool_target_{zone}"), json!(24.0));
            group_inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
            group_inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
            group_inputs.insert(format!("window_closed_{zone}"), json!(true));
            group_inputs.insert(format!("override_{zone}"), json!(false));
        }
        assert_eq!(
            Executor::default()
                .execute(group, &group_inputs, 60.0, "")
                .unwrap()["warmup"],
            true
        );
        group_inputs.insert("window_closed_1".into(), json!(false));
        let opened = Executor::default()
            .execute(group, &group_inputs, 60.0, "")
            .unwrap();
        assert_eq!(opened["warmup"], false);
        let compiled = CompiledExecutor::try_new(group)
            .unwrap()
            .unwrap()
            .execute(&group_inputs, 60.0, false)
            .unwrap()
            .0;
        assert_eq!(compiled["warmup"], 0.0);
        group_inputs.insert("lookahead_allowed".into(), json!(false));
        group_inputs.insert("room_1".into(), json!(5.0));
        assert_eq!(
            Executor::default()
                .execute(group, &group_inputs, 60.0, "")
                .unwrap()["setback"],
            false
        );
        group_inputs.insert("window_closed_1".into(), json!(true));
        assert_eq!(
            Executor::default()
                .execute(group, &group_inputs, 60.0, "")
                .unwrap()["setback"],
            true
        );
        group_inputs.insert("window_closed_1".into(), json!(false));
        group_inputs.insert("room_1".into(), json!(3.0));
        assert_eq!(
            Executor::default()
                .execute(group, &group_inputs, 60.0, "")
                .unwrap()["freeze_setback"],
            true
        );

        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut inputs = HashMap::from([
            ("room_temp".into(), json!(29.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("zone_presence_raw".into(), json!(true)),
            ("measured_airflow".into(), json!(0.0)),
            ("discharge_temp".into(), json!(16.0)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
        ]);
        let closed = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        inputs.insert("window_closed".into(), json!(false));
        let open = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        assert!(closed["cooling_demand"].as_f64().unwrap() > 0.0);
        assert_eq!(open["cooling_demand"], 0.0);
        inputs.insert("room_temp".into(), json!(3.0));
        inputs.insert("group_freeze_setback".into(), json!(true));
        inputs.insert("scheduled_occupied".into(), json!(false));
        let freeze = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        assert!(freeze["heating_demand"].as_f64().unwrap() > 0.0);
    }
    #[test]
    fn scheduled_raw_window_opening_reaches_physical_zone() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 32.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let mut closed = Simulation::new(graph, &weather, "{}").unwrap();
        let mut opened = Simulation::new(
            graph,
            &weather,
            r#"{"window_openings":[{"zone":1,"start_hour":0,"duration_hours":2}]}"#,
        )
        .unwrap();
        closed.temps = [29.0; 10];
        opened.temps = [29.0; 10];
        let normal = run_weather_intervals(&mut closed, 2);
        let with_open_window = run_weather_intervals(&mut opened, 2);
        assert!(
            with_open_window["rows"][1]["zone_airflow"][0]
                .as_f64()
                .unwrap()
                < normal["rows"][1]["zone_airflow"][0].as_f64().unwrap()
        );
        assert!(
            with_open_window["rows"][1]["zone_c"][0].as_f64().unwrap()
                > normal["rows"][1]["zone_c"][0].as_f64().unwrap()
        );
    }
    #[test]
    fn g36_local_override_and_window_alarm_are_cdl_decisions() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("seconds_until_occupied".into(), json!(3600.0)),
            ("lookahead_allowed".into(), json!(true)),
            ("scheduled_occupied".into(), json!(false)),
        ]);
        for zone in 1..=5 {
            inputs.insert(
                format!("room_{zone}"),
                json!(if zone == 1 { 15.0 } else { 21.0 }),
            );
            inputs.insert(format!("heat_target_{zone}"), json!(21.0));
            inputs.insert(format!("cool_target_{zone}"), json!(24.0));
            inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
            inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
            inputs.insert(format!("window_closed_{zone}"), json!(true));
            inputs.insert(format!("override_{zone}"), json!(false));
        }
        let closed = Executor::default()
            .execute(group, &inputs, 60.0, "")
            .unwrap();
        assert_eq!(closed["warmup"], true);
        assert_eq!(closed["occupied"], false);
        assert_eq!(closed["window_open_alarm_level"], 0.0);
        inputs.insert("override_1".into(), json!(true));
        inputs.insert("window_closed_1".into(), json!(false));
        let overridden = Executor::default()
            .execute(group, &inputs, 60.0, "")
            .unwrap();
        assert_eq!(overridden["occupied"], true);
        assert_eq!(overridden["warmup"], false);
        assert_eq!(overridden["window_open_alarm_level"], 0.0);
        let compiled = CompiledExecutor::try_new(group)
            .unwrap()
            .unwrap()
            .execute(&inputs, 60.0, false)
            .unwrap()
            .0;
        assert_eq!(compiled["occupied"], 1.0);
        assert_eq!(compiled["window_open_alarm_level"], 0.0);
        inputs.insert("override_1".into(), json!(false));
        let alarm = Executor::default()
            .execute(group, &inputs, 60.0, "")
            .unwrap();
        assert_eq!(alarm["occupied"], false);
        assert_eq!(alarm["window_open_alarm_level"], 4.0);

        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut zone_inputs = HashMap::from([
            ("room_temp".into(), json!(29.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(false)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("zone_presence_raw".into(), json!(false)),
            ("measured_airflow".into(), json!(0.0)),
            ("discharge_temp".into(), json!(16.0)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
        ]);
        let idle = Executor::default()
            .execute(zone, &zone_inputs, 60.0, "")
            .unwrap();
        zone_inputs.insert("local_override".into(), json!(true));
        let active = Executor::default()
            .execute(zone, &zone_inputs, 60.0, "")
            .unwrap();
        assert_eq!(idle["cooling_demand"], 0.0);
        assert!(active["cooling_demand"].as_f64().unwrap() > 0.0);
    }
    #[test]
    fn raw_local_override_reaches_ahu_and_zone_physics() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 32.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: false
            };
            35_040
        ])
        .unwrap();
        let mut baseline = Simulation::new(graph, &weather, "{}").unwrap();
        let mut overridden = Simulation::new(
            graph,
            &weather,
            r#"{"occupancy_overrides":[{"zone":1,"start_hour":0,"duration_hours":2}]}"#,
        )
        .unwrap();
        baseline.temps = [29.0; 10];
        overridden.temps = [29.0; 10];
        let idle = run_weather_intervals(&mut baseline, 3);
        let active = run_weather_intervals(&mut overridden, 3);
        assert!(
            active["rows"][2]["zone_airflow"][0].as_f64().unwrap()
                > idle["rows"][2]["zone_airflow"][0].as_f64().unwrap()
        );
        assert!(
            active["rows"][2]["zone_c"][0].as_f64().unwrap()
                < idle["rows"][2]["zone_c"][0].as_f64().unwrap()
        );
    }
    #[test]
    fn cdl_group_setback_changes_physical_heating_and_zone_temperature() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 0.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: false
            };
            35_040
        ])
        .unwrap();
        let mut default = Simulation::new(graph, &weather, "{}").unwrap();
        default.temps = [19.0; 10];
        let normal = run_weather_intervals(&mut default, 4);
        let mut changed: Value = serde_json::from_str(graph).unwrap();
        for zone in changed["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|block| block["block_type"] == "VAVBoxController")
        {
            zone["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "unoccupied_heat")
                .unwrap()["parameters"][0]["value"] = json!(20.0);
        }
        let mut tuned = Simulation::new(&changed.to_string(), &weather, "{}").unwrap();
        tuned.temps = [19.0; 10];
        let setback = run_weather_intervals(&mut tuned, 4);
        assert!(
            setback["rows"][3]["heat_request_kw"].as_f64().unwrap()
                > normal["rows"][3]["heat_request_kw"].as_f64().unwrap() + 1.0
        );
        assert!(
            setback["rows"][3]["avg_zone_c"].as_f64().unwrap()
                > normal["rows"][3]["avg_zone_c"].as_f64().unwrap() + 0.1
        );
    }
    #[test]
    fn cdl_warmup_mode_changes_physical_zone_temperature() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let mut weather = vec![
            Weather {
                outdoor_c: -5.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: false
            };
            35_040
        ];
        for sample in &mut weather[87..] {
            sample.occupied = true;
        }
        let weather = serde_json::to_string(&weather).unwrap();
        let mut without: Value = serde_json::from_str(graph).unwrap();
        without["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "preconditioning_horizon")
            .unwrap()["parameters"][0]["value"] = json!(1.0);
        let mut active = Simulation::new(graph, &weather, r#"{"step_seconds":60}"#).unwrap();
        let mut inactive =
            Simulation::new(&without.to_string(), &weather, r#"{"step_seconds":60}"#).unwrap();
        for sim in [&mut active, &mut inactive] {
            sim.cursor = 80 * (900 / sim.effective_step_seconds);
            sim.temps = [19.0; 10];
            sim.last_heat_request = 100.0;
            sim.last_water_flow_kg_s = [2.0, 2.0];
        }
        let warm = run_weather_intervals(&mut active, 8);
        let coast = run_weather_intervals(&mut inactive, 8);
        assert_eq!(warm["rows"][0]["preconditioning_mode"], json!(1.0));
        assert_eq!(coast["rows"][0]["preconditioning_mode"], json!(0.0));
        let warm_temp = warm["rows"][7]["avg_zone_c"].as_f64().unwrap();
        let coast_temp = coast["rows"][7]["avg_zone_c"].as_f64().unwrap();
        assert!(
            warm_temp > coast_temp + 0.2,
            "warm-up {warm_temp} °C, coasting {coast_temp} °C"
        );
    }
    #[test]
    fn g36_damper_tracks_measured_flow_in_both_executors() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut inputs = HashMap::from([
            ("room_temp".into(), json!(27.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("zone_presence_raw".into(), json!(true)),
            ("measured_airflow".into(), json!(0.1)),
            ("discharge_temp".into(), json!(16.0)),
        ]);
        let low_flow = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        inputs.insert("measured_airflow".into(), json!(0.8));
        let high_flow = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        assert_eq!(low_flow["airflow"], json!(0.1));
        assert_eq!(high_flow["airflow"], json!(0.8));
        assert_eq!(low_flow["airflow_request"], high_flow["airflow_request"]);
        assert!(
            low_flow["damper_position"].as_f64().unwrap()
                > high_flow["damper_position"].as_f64().unwrap()
        );
        let compiled = CompiledExecutor::try_new(zone)
            .unwrap()
            .unwrap()
            .execute(&inputs, 60.0, false)
            .unwrap()
            .0;
        assert_eq!(high_flow["damper_position"], compiled["damper_position"]);
        assert_eq!(high_flow["airflow"], compiled["airflow"]);
    }
    #[test]
    fn g36_heating_damper_gain_changes_delivered_airflow() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 0.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        normal.temps = [15.0; 10];
        let open = run_weather_intervals(&mut normal, 3);
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        for zone in edited["blocks"].as_array_mut().unwrap() {
            if zone["block_type"] != "VAVBoxController" {
                continue;
            }
            zone["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "heating_damper_gain")
                .unwrap()["parameters"][0]["value"] = json!(0.0);
        }
        let mut minimum = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        minimum.temps = [15.0; 10];
        let held = run_weather_intervals(&mut minimum, 3);
        assert!(
            open["rows"][2]["delivered_airflow"].as_f64().unwrap()
                > held["rows"][2]["delivered_airflow"].as_f64().unwrap()
        );
    }
    #[test]
    fn g36_heating_airflow_waits_for_warm_discharge() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut inputs = HashMap::from([
            ("room_temp".into(), json!(15.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("zone_presence_raw".into(), json!(true)),
            ("measured_airflow".into(), json!(0.3)),
            ("discharge_temp".into(), json!(17.0)),
        ]);
        let cold = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        inputs.insert("discharge_temp".into(), json!(19.0));
        let warm = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        assert!(
            warm["damper_position"].as_f64().unwrap()
                > cold["damper_position"].as_f64().unwrap() + 0.3,
            "warm damper {}, cold damper {}, warm setpoint {}, cold setpoint {}",
            warm["damper_position"],
            cold["damper_position"],
            warm["airflow_request"],
            cold["airflow_request"]
        );
        let compiled = CompiledExecutor::try_new(zone)
            .unwrap()
            .unwrap()
            .execute(&inputs, 60.0, false)
            .unwrap()
            .0;
        assert_eq!(warm["damper_position"], compiled["damper_position"]);
        let mut edited = zone.clone();
        edited
            .blocks
            .iter_mut()
            .find(|block| block.name == "heating_airflow_margin")
            .unwrap()
            .parameters[0]
            .value = json!(5.0);
        let held = Executor::default()
            .execute(&edited, &inputs, 60.0, "")
            .unwrap();
        assert_eq!(held["damper_position"], cold["damper_position"]);
        inputs.insert("discharge_temp".into(), json!(17.0));
        inputs.insert("scheduled_occupied".into(), json!(false));
        inputs.insert("group_warmup".into(), json!(true));
        let warmup = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        assert!(
            warmup["damper_position"].as_f64().unwrap()
                > cold["damper_position"].as_f64().unwrap() + 0.3
        );
    }
    #[test]
    fn g36_discharge_limit_is_graph_owned_and_sensors_follow_delivery() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 0.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let scenario = r#"{"heat_pump_capacity_kw":1000,"boiler_capacity_kw":1000,"chiller_capacity_kw":1000}"#;
        let mut normal = Simulation::new(graph, &weather, scenario).unwrap();
        normal.temps = [15.0; 10];
        normal.last_zone_airflow = [0.4; 10];
        normal.last_heat_request = 200.0;
        let base = run_weather_intervals(&mut normal, 10);
        let mut dynamic = Simulation::new(graph, &weather, scenario).unwrap();
        dynamic.compiled = None;
        dynamic.temps = [15.0; 10];
        dynamic.last_zone_airflow = [0.4; 10];
        dynamic.last_heat_request = 200.0;
        let dynamic_rows = run_weather_intervals(&mut dynamic, 10);
        assert_eq!(
            base["rows"][9]["zone_discharge_c"],
            dynamic_rows["rows"][9]["zone_discharge_c"]
        );
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        for zone in edited["blocks"].as_array_mut().unwrap() {
            if zone["block_type"] != "VAVBoxController" {
                continue;
            }
            let limit = zone["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "maximum_discharge_temp")
                .unwrap();
            limit["parameters"][0]["value"] = json!(18.0);
        }
        let mut restricted = Simulation::new(&edited.to_string(), &weather, scenario).unwrap();
        restricted.temps = [15.0; 10];
        restricted.last_zone_airflow = [0.4; 10];
        restricted.last_heat_request = 200.0;
        let low = run_weather_intervals(&mut restricted, 10);
        let base_row = &base["rows"][9];
        let low_row = &low["rows"][9];
        assert!(
            base_row["zone_discharge_c"][0].as_f64().unwrap()
            > low_row["zone_discharge_c"][0].as_f64().unwrap() + 3.0,
            "base discharge={}, limited discharge={}, hot flow={}, hot pressure={}, speed={}, enabled={}",
            base_row["zone_discharge_c"][0],
            low_row["zone_discharge_c"][0],
            base_row["hot_water_flow_kg_s"],
            base_row["hot_water_differential_pressure_pa"],
            base_row["hot_water_pump_speed"],
            base_row["hot_water_enable"]
        );
        assert!(
            low_row["zone_discharge_c"][0].as_f64().unwrap()
                <= low_row["ahu_1_supply_actual_c"].as_f64().unwrap() + 2.5,
            "discharge={}, supply={}",
            low_row["zone_discharge_c"][0],
            low_row["ahu_1_supply_actual_c"]
        );
        assert!(
            low_row["zone_discharge_c"][0].as_f64().unwrap()
                >= low_row["ahu_1_supply_actual_c"].as_f64().unwrap() - 0.1
        );
        for row in [base_row, low_row] {
            let total: f64 = row["zone_airflow"]
                .as_array()
                .unwrap()
                .iter()
                .map(|flow| flow.as_f64().unwrap())
                .sum();
            assert!((total - row["delivered_airflow"].as_f64().unwrap()).abs() < 1e-9);
        }
    }
    #[test]
    fn g36_discharge_feedback_gain_changes_physical_reheat() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 0.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let scenario = r#"{"heat_pump_capacity_kw":1000,"boiler_capacity_kw":1000}"#;
        let mut normal = Simulation::new(graph, &weather, scenario).unwrap();
        normal.temps = [15.0; 10];
        normal.last_zone_airflow = [0.4; 10];
        normal.last_heat_request = 200.0;
        let base = run_weather_intervals(&mut normal, 10);
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        for zone in edited["blocks"].as_array_mut().unwrap() {
            if zone["block_type"] != "VAVBoxController" {
                continue;
            }
            zone["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "discharge_valve_pi")
                .unwrap()["parameters"][0]["value"] = json!(0.0);
        }
        let mut disabled = Simulation::new(&edited.to_string(), &weather, scenario).unwrap();
        disabled.temps = [15.0; 10];
        disabled.last_zone_airflow = [0.4; 10];
        disabled.last_heat_request = 200.0;
        let no_feedback = run_weather_intervals(&mut disabled, 10);
        assert!(
            base["rows"][9]["zone_discharge_c"][0].as_f64().unwrap()
                > no_feedback["rows"][9]["zone_discharge_c"][0]
                    .as_f64()
                    .unwrap()
                    + 1.0
        );
        assert!(
            base["rows"][9]["zone_c"][0].as_f64().unwrap()
                > no_feedback["rows"][9]["zone_c"][0].as_f64().unwrap()
        );
    }
    #[test]
    fn g36_outdoor_air_command_changes_mixing_and_coil_load() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 18.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let mut free_cooling = Simulation::new(graph, &weather, "{}").unwrap();
        start_present_window(&mut free_cooling);
        let free = run_weather_intervals(&mut free_cooling, 3);
        let mut fixed: Value = serde_json::from_str(graph).unwrap();
        for ahu in 1..=2 {
            let controller = fixed["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == format!("ahu_{ahu}"))
                .unwrap();
            let outdoor = controller["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "economizer_mixed_air_pi")
                .unwrap();
            outdoor["parameters"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|parameter| parameter["name"] == "yMax")
                .unwrap()["value"] = json!(0.2);
        }
        let mut minimum_only = Simulation::new(&fixed.to_string(), &weather, "{}").unwrap();
        start_present_window(&mut minimum_only);
        let minimum = run_weather_intervals(&mut minimum_only, 3);
        let free_row = &free["rows"][2];
        let minimum_row = &minimum["rows"][2];
        assert!(free_row["ahu_1_outdoor_air_fraction"].as_f64().unwrap() > 0.2);
        assert!(minimum_row["ahu_1_outdoor_air_fraction"].as_f64().unwrap() <= 0.2 + 1e-9);
        assert!(
            free_row["ahu_1_mixed_air_c"].as_f64().unwrap()
                < minimum_row["ahu_1_mixed_air_c"].as_f64().unwrap()
        );
        assert!(
            free_row["cool_request_kw"].as_f64().unwrap()
                < minimum_row["cool_request_kw"].as_f64().unwrap()
        );
    }
    #[test]
    fn g36_economizer_modulates_from_measured_mixed_air() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(18.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.3)),
            ("mixed_air_temp".into(), json!(20.0)),
            ("actual_supply_air_temp".into(), json!(16.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_damper_position"), json!(0.0));
        }
        let high = Executor::default().execute(ahu, &inputs, 60.0, "").unwrap()
            ["outdoor_air_fraction"]
            .as_f64()
            .unwrap();
        inputs.insert("mixed_air_temp".into(), json!(15.0));
        let low = Executor::default().execute(ahu, &inputs, 60.0, "").unwrap()
            ["outdoor_air_fraction"]
            .as_f64()
            .unwrap();
        assert!(
            high > low + 0.1,
            "mixed-air PI should modulate: {high} > {low}"
        );
        let optimized = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        let mut fast = optimized;
        let fast_low = fast.execute(&inputs, 60.0, false).unwrap().0;
        assert!((fast_low["outdoor_air_fraction"].as_f64().unwrap() - low).abs() < 1e-9);
    }
    #[test]
    fn g36_economizer_high_limit_requires_fan_proof_and_no_freeze() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        assert!(validate(&building).is_empty(), "{:?}", validate(&building));
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(22.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(20.0)),
            ("actual_supply_air_temp".into(), json!(16.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.3)),
            ("freeze_stat_trip".into(), json!(false)),
            ("freeze_reset".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("occupied_mode".into(), json!(true)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_damper_position"), json!(0.0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        for (outdoor, fan, stat, expected) in [
            (22.0, true, false, true),
            (23.5, true, false, true),
            (24.1, true, false, false),
            (23.5, true, false, false),
            (22.9, true, false, true),
            (22.9, false, false, false),
            (22.9, true, false, true),
            (22.9, true, true, false),
            (22.9, true, false, false),
        ] {
            inputs.insert("outdoor_c".into(), json!(outdoor));
            inputs.insert("supply_fan_proven".into(), json!(fan));
            inputs.insert("freeze_stat_trip".into(), json!(stat));
            let actual = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(actual["economizer_mode_enabled"], json!(expected));
            assert_eq!(
                fast["economizer_mode_enabled"],
                json!(if expected { 1.0 } else { 0.0 })
            );
        }
        let mut tuned = ahu.clone();
        tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == "economizer_high_limit_hysteresis")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "uLow")
            .unwrap()
            .value = json!(-2.0);
        let mut executor = Executor::default();
        inputs.insert("freeze_stat_trip".into(), json!(false));
        inputs.insert("outdoor_c".into(), json!(22.9));
        assert_eq!(
            executor.execute(&tuned, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
            json!(true)
        );
        inputs.insert("outdoor_c".into(), json!(24.1));
        assert_eq!(
            executor.execute(&tuned, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
            json!(false)
        );
        inputs.insert("outdoor_c".into(), json!(22.9));
        assert_eq!(
            executor.execute(&tuned, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
            json!(false)
        );
    }
    #[test]
    fn g36_minimum_outdoor_air_requires_occupied_mode_and_fan_proof() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        assert!(validate(&building).is_empty(), "{:?}", validate(&building));
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(18.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(20.0)),
            ("actual_supply_air_temp".into(), json!(16.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("freeze_reset".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("occupied_mode".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_damper_position"), json!(0.0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        let off = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast_off = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(off["minimum_outdoor_air_enabled"], json!(false));
        assert_eq!(off["minimum_outdoor_airflow"], json!(0.0));
        assert!(off["outdoor_air_fraction"].as_f64().unwrap() >= 0.2);
        assert_eq!(
            off["outdoor_air_fraction"],
            fast_off["outdoor_air_fraction"]
        );
        inputs.insert("flow_request".into(), json!(0.0));
        assert_eq!(
            Executor::default().execute(ahu, &inputs, 60.0, "").unwrap()["outdoor_air_fraction"],
            json!(0.0)
        );
        inputs.insert("flow_request".into(), json!(5.0));
        inputs.insert("occupied_mode".into(), json!(true));
        let on = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast_on = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(on["minimum_outdoor_air_enabled"], json!(true));
        assert!((on["minimum_outdoor_airflow"].as_f64().unwrap() - 0.45).abs() < 1e-9);
        assert!(on["outdoor_air_fraction"].as_f64().unwrap() >= 0.2);
        assert_eq!(on["outdoor_air_fraction"], fast_on["outdoor_air_fraction"]);
        inputs.insert("supply_fan_proven".into(), json!(false));
        let unproven = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        assert_eq!(unproven["minimum_outdoor_air_enabled"], json!(false));
        assert_eq!(unproven["minimum_outdoor_airflow"], json!(0.0));
        let mut limited = ahu.clone();
        limited
            .blocks
            .iter_mut()
            .find(|block| block.name == "outdoor_damper_physical_max")
            .unwrap()
            .parameters[0]
            .value = json!(0.1);
        inputs.insert("supply_fan_proven".into(), json!(true));
        let capped = Executor::default()
            .execute(&limited, &inputs, 60.0, "")
            .unwrap();
        assert_eq!(capped["outdoor_air_fraction"], json!(0.1));
        let mut return_limited = ahu.clone();
        return_limited
            .blocks
            .iter_mut()
            .find(|block| block.name == "outdoor_damper_physical_max")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(0.2);
        return_limited
            .blocks
            .iter_mut()
            .find(|block| block.name == "minimum_oa_pi")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(2.0);
        inputs.insert("occupied_mode".into(), json!(false));
        let return_off = Executor::default()
            .execute(&return_limited, &inputs, 60.0, "")
            .unwrap();
        inputs.insert("occupied_mode".into(), json!(true));
        inputs.insert("outdoor_airflow".into(), json!(0.0));
        let return_on = Executor::default()
            .execute(&return_limited, &inputs, 60.0, "")
            .unwrap();
        assert_eq!(return_off["outdoor_air_damper_position"], json!(0.2));
        assert_eq!(return_on["outdoor_air_damper_position"], json!(0.2));
        assert!(
            return_on["return_air_damper_position"].as_f64().unwrap()
                < return_off["return_air_damper_position"].as_f64().unwrap()
        );
        inputs.insert("zone_01_measured_airflow".into(), json!(0.1));
        let constrained = Executor::default().execute(ahu, &inputs, 60.0, "").unwrap();
        let expected_efficiency = 1.0 + 0.45 / 1.7 - 0.9;
        assert!(
            (constrained["ventilation_efficiency"].as_f64().unwrap() - expected_efficiency).abs()
                < 1e-9
        );
        assert!((constrained["minimum_outdoor_airflow"].as_f64().unwrap() - 0.75).abs() < 1e-9);
        let mut raised_design = ahu.clone();
        raised_design
            .blocks
            .iter_mut()
            .find(|block| block.name == "vent_total_design")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(1.5);
        let raised = Executor::default()
            .execute(&raised_design, &inputs, 60.0, "")
            .unwrap();
        assert!(
            (raised["minimum_outdoor_airflow"].as_f64().unwrap() - 0.45 / expected_efficiency)
                .abs()
                < 1e-9
        );
    }
    #[test]
    fn g36_zone_ventilation_uses_confirmed_presence_and_editable_design_air() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 18.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let area = "FourPipeG36Controls|output.zone_01_ventilation_area";
        let population = "FourPipeG36Controls|output.zone_01_ventilation_population";
        let minimum = "FourPipeG36Controls|output.zone_01_ventilation_minimum_oa_flow";
        let targets = json!([area, population, minimum]).to_string();
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        normal.set_traces(&targets).unwrap();
        let rows = run_weather_intervals(&mut normal, 35);
        assert_eq!(rows["rows"][30]["traces"][area], json!(0.0));
        assert_eq!(rows["rows"][33]["traces"][area], json!(0.03));
        assert_eq!(rows["rows"][33]["traces"][population], json!(0.06));
        let baseline_minimum = rows["rows"][33]["traces"][minimum].as_f64().unwrap();
        assert!((0.09..=0.1125 + 1e-9).contains(&baseline_minimum));
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        let zone = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "zone_01")
            .unwrap();
        zone["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "ventilation_area_design")
            .unwrap()["parameters"][0]["value"] = json!(0.08);
        let mut tuned = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        tuned.set_traces(&targets).unwrap();
        let tuned_rows = run_weather_intervals(&mut tuned, 35);
        assert_eq!(tuned_rows["rows"][33]["traces"][area], json!(0.08));
        assert!(tuned_rows["rows"][33]["traces"][minimum].as_f64().unwrap() > baseline_minimum);
    }
    #[test]
    fn g36_common_damper_maps_pi_halves_to_separate_actuators() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(30.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(24.0)),
            ("mixed_air_temp".into(), json!(20.0)),
            ("actual_supply_air_temp".into(), json!(16.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("freeze_reset".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("occupied_mode".into(), json!(true)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_cooling_demand"), json!(0.0));
            inputs.insert(format!("zone_{zone:02}_damper_position"), json!(0.0));
        }
        let mut low = ahu.clone();
        for block_name in ["vent_uncorrected_design", "vent_total_design"] {
            low.blocks
                .iter_mut()
                .find(|block| block.name == block_name)
                .unwrap()
                .parameters
                .iter_mut()
                .find(|parameter| parameter.name == "k")
                .unwrap()
                .value = json!(1.0);
        }
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_ventilation_area"), json!(0.2));
            inputs.insert(format!("zone_{zone:02}_ventilation_population"), json!(0.0));
            inputs.insert(
                format!("zone_{zone:02}_ventilation_minimum_oa_flow"),
                json!(0.2),
            );
        }
        low.blocks
            .iter_mut()
            .find(|block| block.name == "minimum_oa_pi")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(0.2);
        let lower = Executor::default()
            .execute(&low, &inputs, 60.0, "")
            .unwrap();
        assert!((0.4..0.42).contains(&lower["outdoor_air_damper_position"].as_f64().unwrap()));
        assert!((lower["return_air_damper_position"].as_f64().unwrap() - 0.8).abs() < 1e-9);
        let mut high = low.clone();
        high.blocks
            .iter_mut()
            .find(|block| block.name == "minimum_oa_pi")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(0.8);
        let upper = Executor::default()
            .execute(&high, &inputs, 60.0, "")
            .unwrap();
        assert!((upper["outdoor_air_damper_position"].as_f64().unwrap() - 1.0).abs() < 1e-9);
        assert!((0.35..0.4).contains(&upper["return_air_damper_position"].as_f64().unwrap()));
        let mut compiled = CompiledExecutor::try_new(&high).unwrap().unwrap();
        let (fast, _) = compiled.execute(&inputs, 60.0, false).unwrap();
        assert_eq!(
            upper["outdoor_air_damper_position"],
            fast["outdoor_air_damper_position"]
        );
        assert_eq!(
            upper["return_air_damper_position"],
            fast["return_air_damper_position"]
        );
    }
    #[test]
    fn g36_outdoor_damper_cap_changes_physical_air_mixing() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 18.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        start_present_window(&mut normal);
        let normal_rows = run_weather_intervals(&mut normal, 4);
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        let ahu = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "ahu_1")
            .unwrap();
        ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "outdoor_damper_physical_max")
            .unwrap()["parameters"][0]["value"] = json!(0.05);
        let mut limited = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        start_present_window(&mut limited);
        let limited_rows = run_weather_intervals(&mut limited, 4);
        let baseline = &normal_rows["rows"][3];
        let capped = &limited_rows["rows"][3];
        assert!(baseline["ahu_1_outdoor_air_fraction"].as_f64().unwrap() > 0.2);
        assert!(
            capped["ahu_1_intake_opening"].as_f64().unwrap()
                < baseline["ahu_1_intake_opening"].as_f64().unwrap()
        );
        assert!(
            capped["ahu_1_outdoor_airflow"].as_f64().unwrap()
                < baseline["ahu_1_outdoor_airflow"].as_f64().unwrap()
        );
    }
    #[test]
    fn g36_return_damper_cap_changes_physical_air_mixing() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 30.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        let ahu = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "ahu_1")
            .unwrap();
        ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "outdoor_damper_physical_max")
            .unwrap()["parameters"][0]["value"] = json!(0.2);
        let mut normal = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        start_present_window(&mut normal);
        let normal_rows = run_weather_intervals(&mut normal, 4);
        let ahu = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "ahu_1")
            .unwrap();
        ahu["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "return_damper_physical_max")
            .unwrap()["parameters"][0]["value"] = json!(0.05);
        let mut limited = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        start_present_window(&mut limited);
        let limited_rows = run_weather_intervals(&mut limited, 4);
        let baseline = &normal_rows["rows"][3];
        let capped = &limited_rows["rows"][3];
        assert!(
            capped["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
                > baseline["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
        );
        assert!(
            capped["ahu_1_intake_opening"].as_f64().unwrap()
                < baseline["ahu_1_intake_opening"].as_f64().unwrap()
        );
        assert!(
            capped["delivered_airflow"].as_f64().unwrap()
                < baseline["delivered_airflow"].as_f64().unwrap()
        );
        assert!(
            capped["ahu_1_mixed_air_c"].as_f64().unwrap()
                > baseline["ahu_1_mixed_air_c"].as_f64().unwrap()
        );
    }
    #[test]
    fn closed_common_intake_stops_ahu_airflow() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 18.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        let ahu = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "ahu_1")
            .unwrap();
        for name in ["outdoor_damper_physical_max", "return_damper_physical_max"] {
            ahu["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == name)
                .unwrap()["parameters"][0]["value"] = json!(0.0);
        }
        let mut simulation = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        let rows = run_weather_intervals(&mut simulation, 4);
        let row = &rows["rows"][3];
        assert_eq!(row["ahu_1_intake_opening"], json!(0.0));
        assert_eq!(row["ahu_1_duct_pressure"], json!(0.0));
        assert_eq!(row["ahu_1_outdoor_airflow"], json!(0.0));
        assert!(row["zone_airflow"].as_array().unwrap()[..5]
            .iter()
            .all(|flow| flow.as_f64() == Some(0.0)));
    }
    #[test]
    fn g36_economizer_uses_delivered_fan_airflow_as_proof() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 18.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let trace = "FourPipeG36Controls|output.ahu_1_economizer_mode_enabled";
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        start_present_window(&mut normal);
        normal.set_traces(&json!([trace]).to_string()).unwrap();
        let normal_rows = run_weather_intervals(&mut normal, 4);
        assert_eq!(normal_rows["rows"][3]["traces"][trace], json!(1.0));
        let mut disabled: Value = serde_json::from_str(graph).unwrap();
        let ahu = disabled["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "ahu_1")
            .unwrap();
        let fan_edge = ahu["connections"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|edge| edge["to"] == "freeze_stage_three_fan.u2")
            .unwrap();
        fan_edge["from"] = json!("fan_off.y");
        let mut stopped = Simulation::new(&disabled.to_string(), &weather, "{}").unwrap();
        start_present_window(&mut stopped);
        stopped.set_traces(&json!([trace]).to_string()).unwrap();
        let stopped_rows = run_weather_intervals(&mut stopped, 4);
        assert_eq!(stopped_rows["rows"][3]["traces"][trace], json!(0.0));
        assert_eq!(stopped_rows["rows"][3]["ahu_1_outdoor_airflow"], json!(0.0));
    }
    #[test]
    fn ahu_coil_heat_depends_on_editable_valve_command() {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: -10.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        let normal_rows = run_weather_intervals(&mut normal, 4);
        let mut closed: Value = serde_json::from_str(graph).unwrap();
        for ahu in 1..=2 {
            let controller = closed["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == format!("ahu_{ahu}"))
                .unwrap();
            let edge = controller["connections"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|edge| edge["to"] == "heating_coil_valve_position")
                .unwrap();
            edge["from"] = json!("coil_zero.y");
        }
        let mut no_heat = Simulation::new(&closed.to_string(), &weather, "{}").unwrap();
        let closed_rows = run_weather_intervals(&mut no_heat, 4);
        let normal_row = &normal_rows["rows"][3];
        let closed_row = &closed_rows["rows"][3];
        assert!(
            normal_row["ahu_1_heating_coil_valve_position"]
                .as_f64()
                .unwrap()
                > 0.0
        );
        assert_eq!(closed_row["ahu_1_heating_coil_valve_position"], 0.0);
        assert!(
            normal_row["ahu_1_supply_actual_c"].as_f64().unwrap()
                > closed_row["ahu_1_supply_actual_c"].as_f64().unwrap()
        );
        assert!(
            normal_row["ahu_1_heating_coil_request_kw"]
                .as_f64()
                .unwrap()
                > 0.0
        );
        assert_eq!(closed_row["ahu_1_heating_coil_request_kw"], 0.0);
        assert_coil_air_balance(normal_row, 1);
        assert_coil_air_balance(closed_row, 1);
    }
    #[test]
    fn ahu_coil_cooling_depends_on_editable_valve_command() {
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 32.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        let normal_rows = run_weather_intervals(&mut normal, 4);
        let mut closed: Value = serde_json::from_str(graph).unwrap();
        for ahu in 1..=2 {
            let controller = closed["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == format!("ahu_{ahu}"))
                .unwrap();
            let edge = controller["connections"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|edge| edge["to"] == "cooling_coil_valve_position")
                .unwrap();
            edge["from"] = json!("coil_zero.y");
        }
        let mut no_cooling = Simulation::new(&closed.to_string(), &weather, "{}").unwrap();
        let closed_rows = run_weather_intervals(&mut no_cooling, 4);
        let normal_row = &normal_rows["rows"][3];
        let closed_row = &closed_rows["rows"][3];
        assert!(
            normal_row["ahu_1_cooling_coil_valve_position"]
                .as_f64()
                .unwrap()
                > 0.0
        );
        assert_eq!(closed_row["ahu_1_cooling_coil_valve_position"], 0.0);
        assert!(
            normal_row["ahu_1_supply_actual_c"].as_f64().unwrap()
                < closed_row["ahu_1_supply_actual_c"].as_f64().unwrap()
        );
        assert!(
            normal_row["ahu_1_cooling_coil_request_kw"]
                .as_f64()
                .unwrap()
                > 0.0
        );
        assert_eq!(closed_row["ahu_1_cooling_coil_request_kw"], 0.0);
        assert_coil_air_balance(normal_row, 1);
        assert_coil_air_balance(closed_row, 1);
    }
    #[test]
    fn water_pump_commands_gate_delivered_heat_and_cooling() {
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        for (outdoor, output_name, delivered_name, flow_name) in [
            (
                -10.0,
                "hot_water_pump_speed",
                "heat_kw",
                "hot_water_flow_kg_s",
            ),
            (
                32.0,
                "chilled_water_pump_speed",
                "cool_kw",
                "chilled_water_flow_kg_s",
            ),
        ] {
            let weather = serde_json::to_string(&vec![
                Weather {
                    outdoor_c: outdoor,
                    solar_kw: 0.0,
                    internal_kw: 0.0,
                    occupied: true,
                };
                35_040
            ])
            .unwrap();
            let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
            let normal_rows = run_weather_intervals(&mut normal, 4);
            let mut disabled: Value = serde_json::from_str(graph).unwrap();
            let plant = disabled["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "plant")
                .unwrap();
            let edge = plant["connections"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|edge| edge["to"] == output_name)
                .unwrap();
            edge["from"] = json!("pump_zero.y");
            let mut no_flow = Simulation::new(&disabled.to_string(), &weather, "{}").unwrap();
            let disabled_rows = run_weather_intervals(&mut no_flow, 4);
            let normal_row = &normal_rows["rows"][3];
            let disabled_row = &disabled_rows["rows"][3];
            assert!(normal_row[flow_name].as_f64().unwrap() > 0.0, "{flow_name}");
            assert!(
                normal_row[delivered_name].as_f64().unwrap() > 0.0,
                "{delivered_name}"
            );
            assert_eq!(disabled_row[flow_name], 0.0);
            assert_eq!(disabled_row[delivered_name], 0.0);
            assert_water_heat_balance(normal_row);
            assert_water_heat_balance(disabled_row);
        }
    }
    #[test]
    fn editable_water_supply_setpoints_limit_coil_delivery() {
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        for (outdoor, constant_name, changed_setpoint, delivered_name, sensor_name) in [
            (
                -10.0,
                "hot_water_supply_setpoint",
                35.0,
                "heat_kw",
                "hot_water_supply_temp_c",
            ),
            (
                32.0,
                "chilled_water_supply_setpoint",
                17.0,
                "cool_kw",
                "chilled_water_supply_temp_c",
            ),
        ] {
            let weather = serde_json::to_string(&vec![
                Weather {
                    outdoor_c: outdoor,
                    solar_kw: 0.0,
                    internal_kw: 0.0,
                    occupied: true,
                };
                35_040
            ])
            .unwrap();
            let mut default = Simulation::new(graph, &weather, "{}").unwrap();
            let baseline = run_weather_intervals(&mut default, 48);
            let mut changed: Value = serde_json::from_str(graph).unwrap();
            let plant = changed["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "plant")
                .unwrap();
            let setpoint = plant["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == constant_name)
                .unwrap();
            setpoint["parameters"][0]["value"] = json!(changed_setpoint);
            let mut limited = Simulation::new(&changed.to_string(), &weather, "{}").unwrap();
            let altered = run_weather_intervals(&mut limited, 48);
            let baseline_row = &baseline["rows"][47];
            let altered_row = &altered["rows"][47];
            assert!(baseline_row[delivered_name].as_f64().unwrap() > 0.0);
            assert_eq!(
                altered_row[if sensor_name == "hot_water_supply_temp_c" {
                    "hot_water_supply_setpoint_c"
                } else {
                    "chilled_water_supply_setpoint_c"
                }],
                changed_setpoint
            );
            let actual = altered_row[sensor_name].as_f64().unwrap();
            let baseline_actual = baseline_row[sensor_name].as_f64().unwrap();
            if sensor_name == "hot_water_supply_temp_c" {
                assert!(actual < baseline_actual - 1.0);
            } else {
                assert!(actual > baseline_actual + 1.0);
            }
            let altered_zone = altered_row["avg_zone_c"].as_f64().unwrap();
            let baseline_zone = baseline_row["avg_zone_c"].as_f64().unwrap();
            if delivered_name == "heat_kw" {
                assert!(altered_zone < baseline_zone);
            } else {
                assert!(altered_zone > baseline_zone);
            }
            assert_water_heat_balance(altered_row);
        }
    }
    #[test]
    fn vav_request_starts_fan_when_duct_pressure_is_zero() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 30.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: false,
            };
            35_040
        ])
        .unwrap();
        let mut sim = Simulation::new(graph, &weather, "{}").unwrap();
        sim.temps = [34.0; 10];
        sim.last_duct_pressure = [0.0; 2];
        let result = run_weather_intervals(&mut sim, 3);
        assert!(
            result["rows"][0]["requested_airflow"].as_f64().unwrap() > 0.0,
            "row0 request={} avg={} supply={} fan={}",
            result["rows"][0]["requested_airflow"],
            result["rows"][0]["avg_zone_c"],
            result["rows"][0]["ahu_1_supply_actual_c"],
            result["rows"][0]["ahu_1_duct_pressure"]
        );
        assert!(result["rows"][1]["delivered_airflow"].as_f64().unwrap() > 0.0);
    }
    #[test]
    fn plant_nameplate_capacity_limits_heating_delivery() {
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: -8.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut sized = Simulation::new(graph, &weather, "{}").unwrap();
        let mut undersized = Simulation::new(
            graph,
            &weather,
            r#"{"heat_pump_capacity_kw":1,"boiler_capacity_kw":1}"#,
        )
        .unwrap();
        let normal: Value = serde_json::from_str(&sized.step_chunk(12).unwrap()).unwrap();
        let limited: Value = serde_json::from_str(&undersized.step_chunk(12).unwrap()).unwrap();
        let normal = &normal["rows"][11];
        let limited = &limited["rows"][11];
        assert!(normal["heat_kw"].as_f64().unwrap() > limited["heat_kw"].as_f64().unwrap());
        let source_heat = limited["heat_pump_heating_used_kw"].as_f64().unwrap()
            + limited["boiler_used_kw"].as_f64().unwrap();
        assert!(source_heat <= 2.0 + 1e-9);
        assert!(
            limited["heat_kw"].as_f64().unwrap() > source_heat,
            "stored hot water should briefly cover a source shortfall"
        );
        let stored_heat = (undersized.last_water_supply_c[0]
            - limited["hot_water_supply_temp_c"].as_f64().unwrap())
            * (5_000.0 * 4.18 / 3_600.0)
            / 0.25;
        assert!((source_heat - limited["heat_kw"].as_f64().unwrap() - stored_heat).abs() < 1e-8);
        assert!(normal["avg_zone_c"].as_f64().unwrap() > limited["avg_zone_c"].as_f64().unwrap());
    }
    #[test]
    fn examples_produce_declared_outputs() {
        for text in [
            include_str!("../../web/public/examples/east_controller.json"),
            include_str!("../../web/public/examples/west_controller.json"),
            include_str!("../../web/public/examples/north_controller.json"),
            include_str!("../../web/public/examples/south_controller.json"),
            include_str!("../../web/public/examples/corridor_controller.json"),
            include_str!("../../web/public/examples/room_control_system.json"),
            include_str!("../../web/public/examples/simple_temp_controller.json"),
            include_str!("../../web/public/examples/basic_temp_control.json"),
        ] {
            let graph: Block = serde_json::from_str(text).unwrap();
            let inputs = graph
                .inputs
                .iter()
                .map(|p| {
                    (
                        p.name.clone(),
                        json!(if p.name.contains("pressure") {
                            1.0
                        } else if p.name.contains("supply_air") {
                            13.0
                        } else {
                            22.0
                        }),
                    )
                })
                .collect();
            let output = Executor::default()
                .execute(&graph, &inputs, 900.0, "")
                .unwrap();
            assert!(!output.is_empty(), "{} returned no outputs", graph.name);
        }
    }
    #[test]
    fn smaller_timestep_advances_weather_every_fifteen_minutes() {
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        let mut samples = vec![
            Weather {
                outdoor_c: 12.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: false,
            };
            35_040
        ];
        samples[1].outdoor_c = 30.0;
        let weather = serde_json::to_string(&samples).unwrap();
        let mut sim = Simulation::new(graph, &weather, r#"{"step_seconds":15}"#).unwrap();
        let first: Value = serde_json::from_str(&sim.step_chunk(59).unwrap()).unwrap();
        assert_eq!(first["progress"], 59);
        assert_eq!(first["total"], 2_102_400);
        assert_eq!(first["rows"].as_array().unwrap().len(), 0);
        let boundary: Value = serde_json::from_str(&sim.step_chunk(1).unwrap()).unwrap();
        assert_eq!(boundary["rows"][0]["index"], 0);
        assert_eq!(boundary["rows"][0]["outdoor_c"], 12.0);
        let next: Value = serde_json::from_str(&sim.step_chunk(60).unwrap()).unwrap();
        assert_eq!(next["rows"][0]["index"], 1);
        assert_eq!(next["rows"][0]["outdoor_c"], 30.0);
        assert!(next["summary"]["electricity_kwh"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn arrow_batch_preserves_measurements_and_selected_trace() {
        use arrow_ipc::reader::StreamReader;
        use std::io::Cursor;

        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 12.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: false
            };
            35_040
        ])
        .unwrap();
        let mut sim = Simulation::new(graph, &weather, r#"{"step_seconds":15}"#).unwrap();
        let trace = "FourPipeBuildingControls.plant|backup_fraction.y";
        sim.set_traces(&serde_json::to_string(&vec![trace]).unwrap())
            .unwrap();
        assert!(sim.step_chunk_arrow(59).unwrap().is_empty());
        let bytes = sim.step_chunk_arrow(1).unwrap();
        let batch = StreamReader::try_new(Cursor::new(bytes), None)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(batch.num_rows(), 1);
        assert_eq!(sim.progress(), 60);
        let outdoor = batch
            .column_by_name("outdoor_c")
            .unwrap()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap();
        assert_eq!(outdoor.value(0), 12.0);
        for name in [
            "zone_01_airflow",
            "zone_01_discharge_c",
            "ahu_1_heating_coil_valve_position",
            "ahu_1_cooling_coil_valve_position",
            "ahu_1_heating_coil_delivered_kw",
            "ahu_1_cooling_coil_delivered_kw",
            "hot_water_supply_temp_c",
            "hot_water_return_temp_c",
            "hot_water_flow_kg_s",
            "chilled_water_supply_temp_c",
            "chilled_water_return_temp_c",
            "chilled_water_flow_kg_s",
            "water_pump_kw",
            "heat_pump_electric_kw",
            "heat_pump_ambient_exchange_kw",
        ] {
            let sensor = batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap();
            assert!(sensor.value(0).is_finite(), "{name}");
        }
        for name in [
            "schedule_occupied",
            "preconditioning_mode",
            "zone_01_occupied",
        ] {
            let occupancy = batch
                .column_by_name(name)
                .unwrap()
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap();
            assert_eq!(occupancy.value(0), 0.0, "{name}");
        }
        let traced = batch
            .column_by_name(&format!("trace:{trace}"))
            .unwrap()
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap();
        assert!((traced.value(0) - 0.45).abs() < 1e-12);
    }

    #[test]
    fn compiled_simulation_matches_dynamic_executor() {
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        let weather = include_str!("../../web/public/examples/weather_2025.json");
        let traces = serde_json::to_string(&vec![
            "FourPipeBuildingControls.plant|backup_fraction.y",
            "FourPipeBuildingControls.ahu_1|is_cold.y",
            "FourPipeBuildingControls|edge.0",
            "FourPipeBuildingControls|outdoor_c",
            "FourPipeBuildingControls|output.ahu_1_supply_air_temp",
            "FourPipeBuildingControls.zone_01|edge.0",
            "FourPipeBuildingControls.zone_01|cool_error.y",
        ])
        .unwrap();
        for step_seconds in [900, 15] {
            let scenario = format!("{{\"step_seconds\":{step_seconds}}}");
            let mut fast = Simulation::new(graph, weather, &scenario).unwrap();
            assert!(fast.compiled.is_some());
            fast.set_traces(&traces).unwrap();
            let mut dynamic = Simulation::new(graph, weather, &scenario).unwrap();
            dynamic.compiled = None;
            dynamic.set_traces(&traces).unwrap();
            let count = if step_seconds == 900 { 96 } else { 120 };
            let fast_rows: Value = serde_json::from_str(&fast.step_chunk(count).unwrap()).unwrap();
            let dynamic_rows: Value =
                serde_json::from_str(&dynamic.step_chunk(count).unwrap()).unwrap();
            assert_eq!(fast_rows["progress"], dynamic_rows["progress"]);
            let fast_rows = fast_rows["rows"].as_array().unwrap();
            let dynamic_rows = dynamic_rows["rows"].as_array().unwrap();
            assert_eq!(fast_rows.len(), dynamic_rows.len());
            for (a, b) in fast_rows.iter().zip(dynamic_rows) {
                for name in [
                    "avg_zone_c",
                    "electric_kw",
                    "gas_kw",
                    "heat_kw",
                    "cool_kw",
                    "requested_airflow",
                ] {
                    assert!(
                        (a[name].as_f64().unwrap() - b[name].as_f64().unwrap()).abs() < 1e-9,
                        "{name} differs at step {step_seconds}"
                    );
                }
                for target in [
                    "FourPipeBuildingControls.plant|backup_fraction.y",
                    "FourPipeBuildingControls.ahu_1|is_cold.y",
                    "FourPipeBuildingControls|edge.0",
                    "FourPipeBuildingControls|outdoor_c",
                    "FourPipeBuildingControls|output.ahu_1_supply_air_temp",
                    "FourPipeBuildingControls.zone_01|edge.0",
                    "FourPipeBuildingControls.zone_01|cool_error.y",
                ] {
                    assert!(
                        (a["traces"][target].as_f64().unwrap()
                            - b["traces"][target].as_f64().unwrap())
                        .abs()
                            < 1e-9,
                        "{target} differs"
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "Manual performance comparison; run with --ignored --nocapture"]
    fn benchmark_compiled_vs_dynamic() {
        use std::time::Instant;
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        let weather = include_str!("../../web/public/examples/weather_2025.json");
        let mut fast = Simulation::new(graph, weather, r#"{"step_seconds":15}"#).unwrap();
        let mut dynamic = Simulation::new(graph, weather, r#"{"step_seconds":15}"#).unwrap();
        dynamic.compiled = None;
        for (label, sim) in [("compiled", &mut fast), ("dynamic", &mut dynamic)] {
            let started = Instant::now();
            while sim.cursor < 12_000 {
                sim.step_chunk(672).unwrap();
            }
            let seconds = started.elapsed().as_secs_f64();
            println!(
                "{label}: {} steps in {seconds:.3}s ({:.0} steps/s)",
                sim.cursor,
                sim.cursor as f64 / seconds
            );
        }
    }

    #[test]
    fn closed_loop_changes_after_setpoint_edit() {
        let mut graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 20.0,
                solar_kw: 1.0,
                internal_kw: 1.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let original = serde_json::to_string(&graph).unwrap();
        let mut a = Simulation::new(&original, &weather, "{}").unwrap();
        let baseline: Value = serde_json::from_str(&a.step_chunk(16).unwrap()).unwrap();
        for b in &mut graph.blocks {
            if let Some(source) = b
                .blocks
                .iter_mut()
                .find(|child| child.name == "cool_setpoint")
            {
                if let Some(p) = source.parameters.iter_mut().find(|p| p.name == "k") {
                    p.value = json!(20.0)
                }
            }
        }
        let mut b =
            Simulation::new(&serde_json::to_string(&graph).unwrap(), &weather, "{}").unwrap();
        let changed: Value = serde_json::from_str(&b.step_chunk(16).unwrap()).unwrap();
        assert_ne!(
            baseline["rows"][15]["avg_zone_c"],
            changed["rows"][15]["avg_zone_c"]
        );
    }
    #[test]
    fn ahu_and_plant_are_executed_from_the_building_graph() {
        let mut graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(20.0)),
            ("heat_request_kw".into(), json!(10.0)),
            ("cool_request_kw".into(), json!(5.0)),
            ("ahu_1_flow_request".into(), json!(2.0)),
            ("ahu_2_flow_request".into(), json!(2.0)),
            ("ahu_1_actual_supply_air_temp".into(), json!(16.0)),
            ("ahu_2_actual_supply_air_temp".into(), json!(16.0)),
            ("ahu_1_mixed_air_temp".into(), json!(20.0)),
            ("ahu_2_mixed_air_temp".into(), json!(20.0)),
            ("hot_water_supply_temp_c".into(), json!(45.0)),
            ("hot_water_return_temp_c".into(), json!(40.0)),
            ("chilled_water_supply_temp_c".into(), json!(7.0)),
            ("chilled_water_return_temp_c".into(), json!(12.0)),
            ("hot_water_flow_kg_s".into(), json!(1.0)),
            ("chilled_water_flow_kg_s".into(), json!(1.0)),
        ]);
        for zone in 1..=10 {
            inputs.insert(format!("zone_{zone:02}_room_temp"), json!(22.0));
            inputs.insert(format!("zone_{zone:02}_supply_pressure"), json!(1.0));
        }
        graph
            .blocks
            .iter_mut()
            .find(|block| block.name == "plant")
            .unwrap()
            .blocks
            .iter_mut()
            .find(|block| block.name == "heat_pump_dispatch_fraction")
            .unwrap()
            .parameters[0]
            .value = json!(0.5);
        let baseline = Executor::default()
            .execute(&graph, &inputs, 900.0, "")
            .unwrap();
        assert_eq!(baseline["ahu_1_supply_air_temp"], json!(16.0));
        assert_eq!(baseline["plant_boiler_kw"], json!(5.0));
        let mut traced = Executor::default();
        traced.trace_targets = vec![
            "FourPipeBuildingControls.plant|backup_fraction.y".into(),
            "FourPipeBuildingControls.plant|edge.2".into(),
            "FourPipeBuildingControls.ahu_1|is_cold.y".into(),
        ];
        traced.execute(&graph, &inputs, 900.0, "").unwrap();
        assert_eq!(
            traced
                .trace_values
                .get("FourPipeBuildingControls.plant|backup_fraction.y"),
            Some(&json!(0.5))
        );
        assert_eq!(
            traced
                .trace_values
                .get("FourPipeBuildingControls.plant|edge.2"),
            Some(&json!(0.5))
        );
        assert_eq!(
            traced
                .trace_values
                .get("FourPipeBuildingControls.ahu_1|is_cold.y"),
            Some(&json!(false))
        );
        let ahu = graph.blocks.iter_mut().find(|b| b.name == "ahu_1").unwrap();
        ahu.blocks
            .iter_mut()
            .find(|b| b.name == "mild_supply")
            .unwrap()
            .parameters[0]
            .value = json!(19.0);
        let plant = graph.blocks.iter_mut().find(|b| b.name == "plant").unwrap();
        plant
            .blocks
            .iter_mut()
            .find(|b| b.name == "one")
            .unwrap()
            .parameters[0]
            .value = json!(0.75);
        let changed = Executor::default()
            .execute(&graph, &inputs, 900.0, "")
            .unwrap();
        assert_eq!(changed["ahu_1_supply_air_temp"], json!(19.0));
        assert_eq!(changed["plant_boiler_kw"], json!(2.5));
    }

    #[test]
    fn fan_gain_changes_delivered_air_and_zone_temperature() {
        let mut graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 30.0,
                solar_kw: 12.0,
                internal_kw: 16.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut normal =
            Simulation::new(&serde_json::to_string(&graph).unwrap(), &weather, "{}").unwrap();
        let normal_rows: Value = serde_json::from_str(&normal.step_chunk(192).unwrap()).unwrap();
        let ahu = graph.blocks.iter_mut().find(|b| b.name == "ahu_1").unwrap();
        ahu.blocks
            .iter_mut()
            .find(|b| b.name == "fan_gain")
            .unwrap()
            .parameters[0]
            .value = json!(0.1);
        let mut slow =
            Simulation::new(&serde_json::to_string(&graph).unwrap(), &weather, "{}").unwrap();
        let slow_rows: Value = serde_json::from_str(&slow.step_chunk(192).unwrap()).unwrap();
        let normal_last = &normal_rows["rows"][191];
        let slow_last = &slow_rows["rows"][191];
        let ahu_1_flow = |row: &Value| {
            row["zone_airflow"].as_array().unwrap()[..5]
                .iter()
                .map(|value| value.as_f64().unwrap())
                .sum::<f64>()
        };
        let flow_loss = normal_rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(slow_rows["rows"].as_array().unwrap())
            .map(|(normal, slow)| ahu_1_flow(normal) - ahu_1_flow(slow))
            .fold(0.0_f64, f64::max);
        let zone_warming = normal_rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(slow_rows["rows"].as_array().unwrap())
            .map(|(normal, slow)| {
                slow["zone_c"][0].as_f64().unwrap() - normal["zone_c"][0].as_f64().unwrap()
            })
            .fold(0.0_f64, f64::max);
        assert!(
            flow_loss > 0.1,
            "maximum AHU 1 airflow loss {flow_loss}; final flows {} and {}",
            ahu_1_flow(normal_last),
            ahu_1_flow(slow_last)
        );
        assert!(zone_warming > 0.1, "maximum zone warming {zone_warming}");
    }

    #[test]
    fn closing_cdl_damper_stops_flow_despite_airflow_request() {
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 30.0,
                solar_kw: 12.0,
                internal_kw: 16.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut open = Simulation::new(graph, &weather, "{}").unwrap();
        let reference: Value = serde_json::from_str(&open.step_chunk(3).unwrap()).unwrap();
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        let zone = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "zone_01")
            .unwrap();
        let edges = zone["connections"].as_array_mut().unwrap();
        edges.retain(|edge| edge["to"] != "damper_position");
        edges.push(json!({"from":"zero.y","to":"damper_position"}));
        let mut closed = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        let result: Value = serde_json::from_str(&closed.step_chunk(3).unwrap()).unwrap();
        assert!(result["rows"][2]["requested_airflow"].as_f64().unwrap() > 0.0);
        assert!(reference["rows"][2]["zone_airflow"][0].as_f64().unwrap() > 0.0);
        assert_eq!(result["rows"][2]["zone_airflow"][0], json!(0.0));
    }

    #[test]
    fn plant_output_limits_supply_air_and_energy_use() {
        let mut graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 32.0,
                solar_kw: 12.0,
                internal_kw: 16.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        graph
            .blocks
            .iter_mut()
            .find(|block| block.name == "plant")
            .unwrap()
            .blocks
            .iter_mut()
            .find(|block| block.name == "heat_pump_dispatch_fraction")
            .unwrap()
            .parameters[0]
            .value = json!(0.0);
        let scenario = "{}";
        let mut normal =
            Simulation::new(&serde_json::to_string(&graph).unwrap(), &weather, scenario).unwrap();
        let normal_rows: Value = serde_json::from_str(&normal.step_chunk(192).unwrap()).unwrap();
        let plant = graph.blocks.iter_mut().find(|b| b.name == "plant").unwrap();
        plant
            .blocks
            .iter_mut()
            .find(|b| b.name == "one")
            .unwrap()
            .parameters[0]
            .value = json!(0.0);
        let mut unavailable =
            Simulation::new(&serde_json::to_string(&graph).unwrap(), &weather, scenario).unwrap();
        let unavailable_rows: Value =
            serde_json::from_str(&unavailable.step_chunk(192).unwrap()).unwrap();
        let normal_last = &normal_rows["rows"][191];
        let unavailable_last = &unavailable_rows["rows"][191];
        assert!(normal_last["cool_kw"].as_f64().unwrap() > 0.0);
        assert_eq!(unavailable_last["cool_kw"], json!(0.0));
        assert_eq!(unavailable_last["chiller_used_kw"], json!(0.0));
        assert!(
            unavailable_last["ahu_1_supply_actual_c"].as_f64().unwrap()
                > unavailable_last["ahu_1_supply_setpoint_c"]
                    .as_f64()
                    .unwrap()
        );
        assert!(
            unavailable_last["zone_c"][0].as_f64().unwrap()
                > normal_last["zone_c"][0].as_f64().unwrap()
        );
        let used_cooling = normal_last["heat_pump_cooling_used_kw"].as_f64().unwrap()
            + normal_last["chiller_used_kw"].as_f64().unwrap();
        let stored_delta = (normal_last["chilled_water_supply_temp_c"].as_f64().unwrap()
            - normal.last_water_supply_c[1])
            * (5_000.0 * 4.18 / 3_600.0)
            / 0.25;
        assert!(
            (normal_last["cool_kw"].as_f64().unwrap() + stored_delta - used_cooling).abs() < 1e-8
        );
        assert!(
            normal_last["cool_kw"].as_f64().unwrap()
                <= normal_last["cool_request_kw"].as_f64().unwrap() + 1e-8
        );
    }

    #[test]
    fn supply_reset_changes_coil_load_and_zone_temperature() {
        let mut graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 32.0,
                solar_kw: 12.0,
                internal_kw: 16.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let mut normal =
            Simulation::new(&serde_json::to_string(&graph).unwrap(), &weather, "{}").unwrap();
        let normal_rows: Value = serde_json::from_str(&normal.step_chunk(192).unwrap()).unwrap();
        graph
            .blocks
            .iter_mut()
            .find(|b| b.name == "ahu_1")
            .unwrap()
            .blocks
            .iter_mut()
            .find(|b| b.name == "cooling_supply")
            .unwrap()
            .parameters[0]
            .value = json!(18.0);
        let mut warm =
            Simulation::new(&serde_json::to_string(&graph).unwrap(), &weather, "{}").unwrap();
        let warm_rows: Value = serde_json::from_str(&warm.step_chunk(192).unwrap()).unwrap();
        let a = &normal_rows["rows"][191];
        let b = &warm_rows["rows"][191];
        assert!(
            b["ahu_1_supply_actual_c"].as_f64().unwrap()
                > a["ahu_1_supply_actual_c"].as_f64().unwrap()
        );
        assert!((b["zone_c"][0].as_f64().unwrap() - a["zone_c"][0].as_f64().unwrap()).abs() > 0.05,
            "normal zone {}, warm zone {}, normal supply {}, warm supply {}, normal airflow {}, warm airflow {}",
            a["zone_c"][0], b["zone_c"][0], a["ahu_1_supply_actual_c"], b["ahu_1_supply_actual_c"],
            a["zone_airflow"][0], b["zone_airflow"][0]);
        assert!(
            (b["cool_request_kw"].as_f64().unwrap() - a["cool_request_kw"].as_f64().unwrap()).abs()
                > 0.1
        );
    }

    #[test]
    fn plant_dispatch_moves_energy_between_electricity_and_fuel() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: -5.0,
                solar_kw: 0.0,
                internal_kw: 1.2,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let dispatch_graph = |fraction: f64| {
            let mut edited = graph.clone();
            edited
                .blocks
                .iter_mut()
                .find(|block| block.name == "plant")
                .unwrap()
                .blocks
                .iter_mut()
                .find(|block| block.name == "heat_pump_dispatch_fraction")
                .unwrap()
                .parameters[0]
                .value = json!(fraction);
            serde_json::to_string(&edited).unwrap()
        };
        let mut boiler = Simulation::new(
            &dispatch_graph(0.0),
            &weather,
            r#"{"boiler_capacity_kw":1000}"#,
        )
        .unwrap();
        let mut heat_pump = Simulation::new(
            &dispatch_graph(1.0),
            &weather,
            r#"{"heat_pump_capacity_kw":1000}"#,
        )
        .unwrap();
        let mut ignored_legacy_scenario = Simulation::new(
            &dispatch_graph(0.0),
            &weather,
            r#"{"heat_pump_fraction":1.0,"boiler_capacity_kw":1000}"#,
        )
        .unwrap();
        let boiler_result: Value = serde_json::from_str(&boiler.step_chunk(192).unwrap()).unwrap();
        let hp_result: Value = serde_json::from_str(&heat_pump.step_chunk(192).unwrap()).unwrap();
        let legacy_result: Value =
            serde_json::from_str(&ignored_legacy_scenario.step_chunk(192).unwrap()).unwrap();
        assert_eq!(
            legacy_result["rows"][191]["gas_kw"],
            boiler_result["rows"][191]["gas_kw"]
        );
        let boiler_last = &boiler_result["rows"][191];
        let hp_last = &hp_result["rows"][191];
        assert!(boiler_last["gas_kw"].as_f64().unwrap() > 0.0);
        assert_eq!(hp_last["gas_kw"], json!(0.0));
        assert!(
            hp_last["electric_kw"].as_f64().unwrap() > boiler_last["electric_kw"].as_f64().unwrap()
        );
        assert!(
            (hp_last["heat_kw"].as_f64().unwrap() - boiler_last["heat_kw"].as_f64().unwrap()).abs()
                < 1e-6
        );
    }
}
