use arrow_array::{ArrayRef, Float64Array, RecordBatch, UInt32Array};
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{DataType, Field, Schema};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

mod compiled;
use compiled::CompiledExecutor;

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
        "And",
        "Or",
        "Not",
        "Delay",
        "TrueDelay",
        "Latch",
        "UnitDelay",
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
    "boiler_used_kw",
    "chiller_used_kw",
    "hot_water_supply_temp_c",
    "hot_water_return_temp_c",
    "chilled_water_supply_temp_c",
    "chilled_water_return_temp_c",
    "hot_water_flow_kg_s",
    "chilled_water_flow_kg_s",
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
pub struct Scenario {
    #[serde(default = "default_ua")]
    pub zone_ua_kw_per_k: f64,
    #[serde(default = "default_cap")]
    pub zone_capacity_kwh_per_k: f64,
    #[serde(default = "default_hp")]
    pub heat_pump_fraction: f64,
    #[serde(default = "default_heat_pump_capacity")]
    pub heat_pump_capacity_kw: f64,
    #[serde(default = "default_boiler_capacity")]
    pub boiler_capacity_kw: f64,
    #[serde(default = "default_chiller_capacity")]
    pub chiller_capacity_kw: f64,
    #[serde(default = "default_step_seconds")]
    pub step_seconds: usize,
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
fn default_hp() -> f64 {
    0.55
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
        let errors = validate(&graph);
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
            || !(0.0..=1.0).contains(&scenario.heat_pump_fraction)
            || !scenario.heat_pump_capacity_kw.is_finite()
            || scenario.heat_pump_capacity_kw <= 0.0
            || !scenario.boiler_capacity_kw.is_finite()
            || scenario.boiler_capacity_kw <= 0.0
            || !scenario.chiller_capacity_kw.is_finite()
            || scenario.chiller_capacity_kw <= 0.0
            || !matches!(scenario.step_seconds, 15 | 30 | 60 | 300 | 900)
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
            let mut cool_request = 0.0;
            let mut heat_request = 0.0;
            let mut requested_flow = [0.0_f64; 10];
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
                "heat_pump_fraction".into(),
                json!(self.scenario.heat_pump_fraction),
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
            for ahu in 0..2 {
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
            let preconditioning = control_flag("preconditioning_mode").unwrap_or(false);
            for zone in 0..10 {
                self.zone_occupied[zone] =
                    control_flag(&format!("zone_{:02}_occupancy_confirmed", zone + 1))
                        .unwrap_or(self.raw_occupancy[zone]);
            }
            let hot_water_enabled = control_flag("plant_hot_water_enable")
                .ok_or_else(|| JsValue::from_str("Missing plant_hot_water_enable output"))?;
            let chilled_water_enabled = control_flag("plant_chilled_water_enable")
                .ok_or_else(|| JsValue::from_str("Missing plant_chilled_water_enable output"))?;
            let hot_water_pump_speed = if hot_water_enabled {
                clamp(output("plant_hot_water_pump_speed")?, 0.0, 1.0)
            } else {
                0.0
            };
            let chilled_water_pump_speed = if chilled_water_enabled {
                clamp(output("plant_chilled_water_pump_speed")?, 0.0, 1.0)
            } else {
                0.0
            };
            let water_supply_c = [
                clamp(output("plant_hot_water_supply_setpoint_c")?, 30.0, 60.0),
                clamp(output("plant_chilled_water_supply_setpoint_c")?, 4.0, 18.0),
            ];
            let water_flow_kg_s = [hot_water_pump_speed * 4.31, chilled_water_pump_speed * 4.31];
            let hp_heat_command = if hot_water_enabled {
                output("plant_heat_pump_heating_kw")?.max(0.0)
            } else {
                0.0
            };
            let hp_cool_command = if chilled_water_enabled {
                output("plant_heat_pump_cooling_kw")?.max(0.0)
            } else {
                0.0
            };
            let hp_capacity_fraction = if hp_heat_command + hp_cool_command > 0.0 {
                (self.scenario.heat_pump_capacity_kw / (hp_heat_command + hp_cool_command)).min(1.0)
            } else {
                1.0
            };
            let hp_heat = hp_heat_command * hp_capacity_fraction;
            let hp_cool = hp_cool_command * hp_capacity_fraction;
            let boiler = if hot_water_enabled {
                output("plant_boiler_kw")?
                    .max(0.0)
                    .min(self.scenario.boiler_capacity_kw)
            } else {
                0.0
            };
            let chiller = if chilled_water_enabled {
                output("plant_chiller_kw")?
                    .max(0.0)
                    .min(self.scenario.chiller_capacity_kw)
            } else {
                0.0
            };
            let plant_heat = hp_heat + boiler;
            let plant_cool = hp_cool + chiller;
            let supply_setpoint = [
                output("ahu_1_supply_air_temp")?,
                output("ahu_2_supply_air_temp")?,
            ];
            let fan_speed = [output("ahu_1_fan_speed")?, output("ahu_2_fan_speed")?];
            let outdoor_air_fraction = [
                clamp(output("ahu_1_outdoor_air_fraction")?, 0.0, 1.0),
                clamp(output("ahu_2_outdoor_air_fraction")?, 0.0, 1.0),
            ];
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
                reheat_command[zone] =
                    clamp(output(&format!("{name}_reheat_valve_position"))?, 0.0, 1.0);
                requested_total[ahu] += requested_flow[zone];
            }
            // Fan speed limits how much of the VAV request can physically reach the zones.
            // At full speed each AHU can supply five normalized airflow units.
            let delivery_fraction = [0, 1].map(|ahu| {
                if requested_total[ahu] > 0.0 {
                    clamp(
                        5.0 * clamp(fan_speed[ahu], 0.0, 1.0) / requested_total[ahu],
                        0.0,
                        1.0,
                    )
                } else {
                    1.0
                }
            });
            // A normalized duct pressure measurement follows the available fan
            // head relative to the VAV demand. The controller sees it next step.
            let duct_pressure = [0, 1].map(|ahu| {
                if requested_total[ahu] > 0.0 {
                    clamp(
                        (5.0 * clamp(fan_speed[ahu], 0.0, 1.0) / requested_total[ahu]).powi(2),
                        0.0,
                        1.0,
                    )
                } else {
                    0.0
                }
            });
            let mut delivered_flow = [0.0_f64; 10];
            let mut delivered_by_ahu = [0.0_f64; 2];
            let mut reheat_request = [0.0_f64; 10];
            for zone in 0..10 {
                let ahu = zone / 5;
                delivered_flow[zone] = requested_flow[zone] * delivery_fraction[ahu];
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
                clamp(
                    plant_heat.min(water_heat_capacity_kw) / heat_request,
                    0.0,
                    1.0,
                )
            } else {
                1.0
            };
            let cool_availability = if cool_request > 0.0 {
                clamp(
                    plant_cool.min(water_cool_capacity_kw) / cool_request,
                    0.0,
                    1.0,
                )
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
            self.last_water_supply_c = water_supply_c;
            self.last_water_return_c = water_return_c;
            self.last_water_flow_kg_s = water_flow_kg_s;
            let hp_heat_used = if plant_heat > 0.0 {
                heat_kw * hp_heat / plant_heat
            } else {
                0.0
            };
            let boiler_used = heat_kw - hp_heat_used;
            let hp_cool_used = if plant_cool > 0.0 {
                cool_kw * hp_cool / plant_cool
            } else {
                0.0
            };
            let chiller_used = cool_kw - hp_cool_used;
            let fan_kw = fan_speed
                .iter()
                .map(|s| 2.0 * clamp(*s, 0.0, 1.0).powi(3))
                .sum::<f64>();
            let water_pump_kw =
                3.0 * (hot_water_pump_speed.powi(3) + chilled_water_pump_speed.powi(3));
            let electric_kw = hp_heat_used / 3.2
                + hp_cool_used / 3.8
                + chiller_used / 3.5
                + fan_kw
                + water_pump_kw;
            let gas_kw = boiler_used / 0.9;
            self.electricity_kwh += electric_kw * dt_hours;
            self.gas_kwh += gas_kw * dt_hours;
            if (self.cursor + 1) % steps_per_weather == 0 {
                let mut row = json!({"index":index,"schedule_occupied":if w.occupied {1.0} else {0.0},"preconditioning_mode":if preconditioning {1.0} else {0.0},"zone_occupied":self.zone_occupied.map(|occupied|if occupied {1.0} else {0.0}),"zone_presence_raw":self.raw_occupancy.map(|occupied|if occupied {1.0} else {0.0}),"zone_airflow":delivered_flow,"zone_discharge_c":discharge_c,"outdoor_c":w.outdoor_c,"avg_zone_c":self.temps.iter().sum::<f64>()/10.0,"zone_c":self.temps,"cool_kw":cool_kw,"heat_kw":heat_kw,"cool_request_kw":cool_request,"heat_request_kw":heat_request,"electric_kw":electric_kw,"gas_kw":gas_kw,"requested_airflow":requested_total.iter().sum::<f64>(),"delivered_airflow":delivered_by_ahu.iter().sum::<f64>(),"ahu_1_supply_actual_c":actual_supply_c[0],"ahu_2_supply_actual_c":actual_supply_c[1],"ahu_1_supply_setpoint_c":supply_setpoint[0],"ahu_2_supply_setpoint_c":supply_setpoint[1],"ahu_1_duct_pressure":duct_pressure[0],"ahu_2_duct_pressure":duct_pressure[1],"ahu_1_outdoor_air_fraction":outdoor_air_fraction[0],"ahu_2_outdoor_air_fraction":outdoor_air_fraction[1],"ahu_1_heating_coil_valve_position":heating_coil_valve[0],"ahu_2_heating_coil_valve_position":heating_coil_valve[1],"ahu_1_cooling_coil_valve_position":cooling_coil_valve[0],"ahu_2_cooling_coil_valve_position":cooling_coil_valve[1],"ahu_1_outdoor_airflow":outdoor_airflow[0],"ahu_2_outdoor_airflow":outdoor_airflow[1],"ahu_1_mixed_air_c":mixed_air_c[0],"ahu_2_mixed_air_c":mixed_air_c[1],"heat_pump_heating_used_kw":hp_heat_used,"heat_pump_cooling_used_kw":hp_cool_used,"boiler_used_kw":boiler_used,"chiller_used_kw":chiller_used});
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
                    ("hot_water_return_temp_c", water_return_c[0]),
                    ("chilled_water_supply_temp_c", water_supply_c[1]),
                    ("chilled_water_return_temp_c", water_return_c[1]),
                    ("hot_water_flow_kg_s", water_flow_kg_s[0]),
                    ("chilled_water_flow_kg_s", water_flow_kg_s[1]),
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
            ahu["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|b| b["name"] == "pressure_previous")
                .unwrap()["parameters"][0]["value"] = json!(300.0);
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
        let inputs = HashMap::from([
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
        ]);
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
        for zone in 1..=3 {
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
        assert_eq!(values[&70]["preconditioning_mode"], 1.0);
        assert_eq!(values[&70]["traces"][trace], 24.5);
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
        let reference = run_weather_intervals(&mut reference, 71);
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
        let changed = run_weather_intervals(&mut changed, 71);
        assert_eq!(reference["rows"][31]["zone_occupied"][0], 1.0);
        assert_eq!(changed["rows"][31]["zone_occupied"][0], 0.0);
        assert_eq!(changed["rows"][33]["zone_occupied"][0], 1.0);
        assert_eq!(reference["rows"][70]["preconditioning_mode"], 1.0);
        assert_eq!(changed["rows"][70]["preconditioning_mode"], 0.0);
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
            ("preconditioning_mode".into(), json!(false)),
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
        let scenario = r#"{"heat_pump_fraction":1,"heat_pump_capacity_kw":1000,"boiler_capacity_kw":1000,"chiller_capacity_kw":1000}"#;
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
                > low_row["zone_discharge_c"][0].as_f64().unwrap() + 5.0
        );
        assert!(
            low_row["zone_discharge_c"][0].as_f64().unwrap() <= 18.5,
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
        ]);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
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
            let baseline = run_weather_intervals(&mut default, 4);
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
            let altered = run_weather_intervals(&mut limited, 4);
            let baseline_row = &baseline["rows"][3];
            let altered_row = &altered["rows"][3];
            assert!(baseline_row[delivered_name].as_f64().unwrap() > 0.0);
            assert_eq!(altered_row[sensor_name], changed_setpoint);
            assert_eq!(altered_row[delivered_name], 0.0);
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
        assert!(result["rows"][0]["requested_airflow"].as_f64().unwrap() > 0.0);
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
        assert!(limited["heat_kw"].as_f64().unwrap() <= 2.0 + 1e-9);
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
            ("heat_pump_fraction".into(), json!(0.5)),
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
        let baseline = Executor::default()
            .execute(&graph, &inputs, 900.0, "")
            .unwrap();
        assert_eq!(baseline["ahu_1_supply_air_temp"], json!(16.0));
        assert_eq!(baseline["plant_boiler_kw"], json!(5.0));
        let mut traced = Executor::default();
        traced.trace_targets = vec![
            "FourPipeBuildingControls.plant|backup_fraction.y".into(),
            "FourPipeBuildingControls.plant|edge.1".into(),
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
                .get("FourPipeBuildingControls.plant|edge.1"),
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
        assert!(
            slow_last["delivered_airflow"].as_f64().unwrap()
                < normal_last["delivered_airflow"].as_f64().unwrap()
        );
        assert!(
            slow_last["zone_c"][0].as_f64().unwrap()
                > normal_last["zone_c"][0].as_f64().unwrap() + 0.1
        );
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
        let scenario = r#"{"heat_pump_fraction":0.0}"#;
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
        assert!((normal_last["cool_kw"].as_f64().unwrap() - used_cooling).abs() < 1e-8);
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
        assert!(b["zone_c"][0].as_f64().unwrap() > a["zone_c"][0].as_f64().unwrap());
        assert!(
            (b["cool_request_kw"].as_f64().unwrap() - a["cool_request_kw"].as_f64().unwrap()).abs()
                > 0.1
        );
    }

    #[test]
    fn plant_dispatch_moves_energy_between_electricity_and_fuel() {
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
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
        let mut boiler = Simulation::new(
            graph,
            &weather,
            r#"{"heat_pump_fraction":0.0,"boiler_capacity_kw":1000}"#,
        )
        .unwrap();
        let mut heat_pump = Simulation::new(
            graph,
            &weather,
            r#"{"heat_pump_fraction":1.0,"heat_pump_capacity_kw":1000}"#,
        )
        .unwrap();
        let boiler_result: Value = serde_json::from_str(&boiler.step_chunk(192).unwrap()).unwrap();
        let hp_result: Value = serde_json::from_str(&heat_pump.step_chunk(192).unwrap()).unwrap();
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
