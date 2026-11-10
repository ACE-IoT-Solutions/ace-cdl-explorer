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
        "AddParameter",
        "Min",
        "Max",
        "Line",
        "BooleanToInteger",
        "RealToInteger",
        "BooleanToReal",
        "MultiplyByParameter",
        "Limiter",
        "LimPID",
        "PIDWithReset",
        "Filter",
        "LimRateLimiter",
        "Hysteresis",
        "Switch",
        "Gain",
        "Greater",
        "GreaterThreshold",
        "LessThreshold",
        "Equal",
        "Less",
        "Subtract",
        "Multiply",
        "Divide",
        "And",
        "Or",
        "Not",
        "Delay",
        "TrueDelay",
        "TrueFalseHold",
        "Latch",
        "UnitDelay",
        "Sampler",
        "Pre",
        "TriggeredSampler",
        "TriggeredMovingMean",
        "SampleTrigger",
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
    if block.category.as_deref() == Some("composite") && block.blocks.is_empty() {
        errors.push(format!("{at}: composite must contain CDL blocks"));
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
                let integer_parameter = match block.block_type.rsplit('.').next() {
                    Some("BooleanToInteger") => {
                        matches!(parameter.name.as_str(), "integerTrue" | "integerFalse")
                    }
                    Some("GreaterThreshold") => {
                        parameter.name == "t" && block.block_type.contains("Integers.")
                    }
                    _ => false,
                };
                if integer_parameter
                    && !parameter
                        .value
                        .as_f64()
                        .is_some_and(|value| value.is_finite() && value.fract() == 0.0)
                {
                    errors.push(format!("{at}: {} must be an integer", parameter.name));
                }
            }
            if matches!(
                block.block_type.rsplit('.').next(),
                Some("UnitDelay" | "Sampler")
            ) && !block.parameters.iter().any(|parameter| {
                parameter.name == "samplePeriod"
                    && parameter
                        .value
                        .as_f64()
                        .is_some_and(|value| value.is_finite() && value > 0.0)
            }) {
                errors.push(format!(
                    "{at}: sampled block requires a positive samplePeriod"
                ));
            }
            if (block.block_type.ends_with("Reals.GreaterThreshold")
                || block.block_type.ends_with("Reals.LessThreshold")
                || block.block_type.ends_with("Reals.Greater")
                || block.block_type.ends_with("Reals.Less")
                || matches!(block.block_type.as_str(), "Greater" | "Less"))
                && block.parameters.iter().any(|parameter| {
                    parameter.name == "h"
                        && !parameter
                            .value
                            .as_f64()
                            .is_some_and(|value| value.is_finite() && value >= 0.0)
                })
            {
                errors.push(format!("{at}: comparator hysteresis must be nonnegative"));
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
            if block.block_type.rsplit('.').next() == Some("PIDWithReset") {
                for parameter in ["k", "Ti", "r", "Ni"] {
                    if block.parameters.iter().any(|item| {
                        item.name == parameter
                            && !item
                                .value
                                .as_f64()
                                .is_some_and(|value| value.is_finite() && value > 0.0)
                    }) {
                        errors.push(format!("{at}: {parameter} must be positive"));
                    }
                }
                if param(block, "yMin", 0.0) >= param(block, "yMax", 1.0) {
                    errors.push(format!("{at}: yMin must be less than yMax"));
                }
            }
            if block.block_type.rsplit('.').next() == Some("SampleTrigger")
                && (!block.parameters.iter().any(|parameter| {
                    parameter.name == "period"
                        && parameter
                            .value
                            .as_f64()
                            .is_some_and(|value| value.is_finite() && value > 0.0)
                }) || block.parameters.iter().any(|parameter| {
                    parameter.name == "shift"
                        && !parameter.value.as_f64().is_some_and(f64::is_finite)
                }))
            {
                errors.push(format!(
                    "{at}: SampleTrigger requires a positive period and finite shift"
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
            if block.block_type.rsplit('.').next() == Some("TrueFalseHold") {
                let duration = block
                    .parameters
                    .iter()
                    .find(|parameter| parameter.name == "trueHoldDuration")
                    .and_then(|parameter| parameter.value.as_f64());
                if !duration.is_some_and(|value| value.is_finite() && value >= 0.0)
                    || block.parameters.iter().any(|parameter| {
                        parameter.name == "falseHoldDuration"
                            && !parameter
                                .value
                                .as_f64()
                                .is_some_and(|value| value.is_finite() && value >= 0.0)
                    })
                {
                    errors.push(format!(
                        "{at}: TrueFalseHold requires nonnegative finite hold durations"
                    ));
                }
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
                        || matches!(
                            child.block_type.rsplit('.').next(),
                            Some("UnitDelay" | "Pre")
                        )
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

#[derive(Clone, Default)]
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
fn bool_param(block: &Block, key: &str, default: bool) -> bool {
    block
        .parameters
        .iter()
        .find(|p| p.name == key)
        .and_then(|p| p.value.as_bool())
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
                let low = param(block, "yMin", 0.0);
                let high = param(block, "yMax", 1.0);
                let proportional = gain * e;
                let previous_trigger = *self.flags.get(&key).unwrap_or(&false);
                self.flags.insert(key.clone(), trigger);
                let integral = if trigger && !previous_trigger {
                    param(block, "y_reset", param(block, "xi_start", 0.0)) - proportional
                } else {
                    let old = *self
                        .state
                        .get(&key)
                        .unwrap_or(&param(block, "xi_start", 0.0));
                    let unsaturated = proportional + old;
                    let saturated = clamp(unsaturated, low, high);
                    let antiwindup =
                        (unsaturated - saturated) / (gain * param(block, "Ni", 0.9)).max(1e-12);
                    old + gain * dt / param(block, "Ti", 0.5).max(1e-12) * (e - antiwindup)
                };
                self.state.insert(key.clone(), integral);
                json!(clamp(proportional + integral, low, high))
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
    "ahu_1_actual_fan_speed",
    "ahu_2_actual_fan_speed",
    "ahu_1_actual_outdoor_damper_position",
    "ahu_2_actual_outdoor_damper_position",
    "ahu_1_actual_return_damper_position",
    "ahu_2_actual_return_damper_position",
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
    "hot_water_plant_proven",
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
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_co2_ppm")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_occupied")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_presence_raw")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_airflow")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_discharge_c")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_actual_damper_position")))
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_actual_reheat_valve_position")))
        .chain(targets.iter().map(|target| format!("trace:{target}")))
    {
        fields.push(Field::new(&name, DataType::Float64, false));
        let values = rows
            .iter()
            .map(|row| {
                let value = if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_actual_damper_position"))
                {
                    row["zone_actual_damper_position"]
                        [zone.parse::<usize>().expect("Known zone column") - 1]
                        .as_f64()
                } else if let Some(zone) = name
                    .strip_prefix("zone_")
                    .and_then(|s| s.strip_suffix("_actual_reheat_valve_position"))
                {
                    row["zone_actual_reheat_valve_position"]
                        [zone.parse::<usize>().expect("Known zone column") - 1]
                        .as_f64()
                } else if let Some(zone) = name
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
                    .and_then(|s| s.strip_suffix("_co2_ppm"))
                {
                    row["zone_co2_ppm"][zone.parse::<usize>().expect("Known zone column") - 1]
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
pub struct TerminalFault {
    pub zone: usize,
    pub kind: String,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AhuDamperFault {
    pub ahu: usize,
    pub damper: String,
    pub stuck_position: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OutdoorTemperatureEvent {
    pub delta_c: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}

fn ahu_damper_position(
    faults: &[AhuDamperFault],
    ahu: usize,
    damper: &str,
    hour: f64,
    command: f64,
) -> f64 {
    faults
        .iter()
        .rev()
        .find(|fault| {
            fault.ahu == ahu
                && fault.damper == damper
                && hour >= fault.start_hour
                && hour < fault.start_hour + fault.duration_hours
        })
        .map_or(command, |fault| fault.stuck_position)
}

fn terminal_fault_active(faults: &[TerminalFault], zone: usize, kind: &str, hour: f64) -> bool {
    faults.iter().any(|fault| {
        fault.zone == zone
            && fault.kind == kind
            && hour >= fault.start_hour
            && hour < fault.start_hour + fault.duration_hours
    })
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
    #[serde(default = "default_terminal_design_flow")]
    pub terminal_design_flow: [f64; 10],
    #[serde(default = "default_ahu_fan_design_airflow")]
    pub ahu_fan_design_airflow: [f64; 2],
    #[serde(default = "default_water_pump_design_flow")]
    pub water_pump_design_flow_kg_s: [f64; 2],
    #[serde(default = "default_water_pump_shutoff_pressure")]
    pub water_pump_shutoff_pressure_pa: [f64; 2],
    #[serde(default = "default_zone_air_volume")]
    pub zone_air_volume_m3: f64,
    #[serde(default = "default_zone_infiltration")]
    pub zone_infiltration_m3_s: f64,
    #[serde(default = "default_open_window_infiltration")]
    pub open_window_additional_infiltration_m3_s: f64,
    #[serde(default = "default_zone_occupants")]
    pub design_occupants_per_zone: f64,
    #[serde(default = "default_co2_generation")]
    pub co2_generation_lps_per_person: f64,
    #[serde(default = "default_outdoor_co2")]
    pub outdoor_co2_ppm: f64,
    #[serde(default = "default_heat_pump_capacity")]
    pub heat_pump_capacity_kw: f64,
    #[serde(default = "default_heat_pump_performance")]
    pub heat_pump_performance: Vec<HeatPumpPerformancePoint>,
    #[serde(default)]
    pub equipment_outages: Vec<EquipmentOutage>,
    #[serde(default)]
    pub equipment_start_failures: Vec<EquipmentOutage>,
    #[serde(default)]
    pub fan_faults: Vec<EquipmentOutage>,
    #[serde(default)]
    pub terminal_faults: Vec<TerminalFault>,
    #[serde(default)]
    pub ahu_damper_faults: Vec<AhuDamperFault>,
    #[serde(default)]
    pub outdoor_temperature_events: Vec<OutdoorTemperatureEvent>,
    #[serde(default)]
    pub window_openings: Vec<WindowOpening>,
    #[serde(default)]
    pub occupancy_overrides: Vec<ZoneOverride>,
    #[serde(default)]
    pub alarm_acknowledgements: Vec<ZoneOverride>,
    #[serde(default)]
    pub ahu_alarm_acknowledgements: Vec<AhuOverride>,
    #[serde(default)]
    pub plant_alarm_acknowledgements: Vec<OperatorPress>,
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
#[derive(Clone, Debug, Deserialize)]
pub struct AhuOverride {
    pub ahu: usize,
    pub start_hour: f64,
    pub duration_hours: f64,
}
#[derive(Clone, Debug, Deserialize)]
pub struct OperatorPress {
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
fn default_zone_air_volume() -> f64 {
    250.0
}
fn default_zone_infiltration() -> f64 {
    0.01
}
fn default_terminal_design_flow() -> [f64; 10] {
    [0.95; 10]
}
fn default_ahu_fan_design_airflow() -> [f64; 2] {
    [5.0; 2]
}
fn default_water_pump_design_flow() -> [f64; 2] {
    [4.31; 2]
}
fn default_water_pump_shutoff_pressure() -> [f64; 2] {
    [80_000.0; 2]
}
fn default_open_window_infiltration() -> f64 {
    0.2
}
fn default_zone_occupants() -> f64 {
    8.0
}
fn default_co2_generation() -> f64 {
    0.004
}
fn default_outdoor_co2() -> f64 {
    420.0
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
fn water_loop_hydraulics(
    pump_speed: f64,
    valve_area: f64,
    design_flow_kg_s: f64,
    shutoff_pressure_pa: f64,
) -> (f64, f64) {
    let speed = clamp(pump_speed, 0.0, 1.0);
    let area = clamp(valve_area, 0.0, 1.0);
    let pressure_pa = shutoff_pressure_pa * speed.powi(2) / (1.0 + area.powi(2));
    let flow_kg_s = design_flow_kg_s * area * (pressure_pa / 40_000.0).sqrt();
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
    co2_ppm: [f64; 10],
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
    last_fan_running: [bool; 2],
    last_actual_fan_speed: [f64; 2],
    last_outdoor_damper_position: [f64; 2],
    last_return_damper_position: [f64; 2],
    last_zone_discharge_c: [f64; 10],
    last_zone_damper_position: [f64; 10],
    last_zone_reheat_valve_position: [f64; 10],
    last_water_supply_c: [f64; 2],
    last_water_return_c: [f64; 2],
    last_water_flow_kg_s: [f64; 2],
    last_water_dp_pa: [f64; 2],
    source_start_elapsed_s: [f64; 3],
    source_running: [bool; 3],
    last_hot_water_plant_proven: bool,
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
    fn visit(block: &Block, result: &mut Vec<(&'static str, f64)>) {
        match block.block_type.rsplit('.').next() {
            Some("UnitDelay" | "Sampler") => {
                if let Some(period) = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "samplePeriod")
                    .and_then(|p| p.value.as_f64())
                {
                    result.push(("samplePeriod", period));
                }
            }
            Some("SampleTrigger") => {
                if let Some(period) = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "period")
                    .and_then(|p| p.value.as_f64())
                {
                    result.push(("SampleTrigger period", period));
                }
                if let Some(shift) = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "shift")
                    .and_then(|p| p.value.as_f64())
                {
                    if shift != 0.0 {
                        result.push(("SampleTrigger shift", shift.abs()));
                    }
                }
            }
            _ => {}
        }
        for child in &block.blocks {
            visit(child, result);
        }
    }
    let mut periods = Vec::new();
    visit(graph, &mut periods);
    let mut step = requested;
    for (name, seconds) in periods {
        let integer = seconds.round();
        if !seconds.is_finite()
            || seconds < 15.0
            || (integer - seconds).abs() > 1e-9
            || integer > usize::MAX as f64
        {
            return Err(format!("Building simulation requires {name} to be a whole number of seconds at least 15; got {seconds}"));
        }
        step = gcd(step, integer as usize);
    }
    if step < 15 {
        return Err(
            "CDL sampling events require a simulation step below the supported 15-second minimum"
                .into(),
        );
    }
    Ok(step)
}

fn is_g36_graph(graph: &Block) -> bool {
    graph.name == "FourPipeG36Controls"
        || graph
            .blocks
            .iter()
            .any(|block| block.block_type == "ZoneGroupModeController")
}

fn cdl_output_has_block_driver(block: &Block, port: &str) -> bool {
    if !block.outputs.iter().any(|output| output.name == port) {
        return false;
    }
    if block.blocks.is_empty() {
        return block.category.as_deref() != Some("composite")
            && block.block_type.rsplit('.').next() != Some("VAVBoxController")
            && known_type(&block.block_type);
    }
    block.connections.iter().any(|edge| {
        let (target, target_port) = edge.target();
        if target != port || !target_port.is_empty() {
            return false;
        }
        let (source, source_port) = edge.source();
        block
            .blocks
            .iter()
            .any(|child| child.name == source && cdl_output_has_block_driver(child, &source_port))
    })
}

fn undriven_building_outputs(graph: &Block) -> Vec<String> {
    if !graph
        .blocks
        .iter()
        .any(|block| block.block_type == "AHUController")
    {
        return Vec::new();
    }
    graph
        .outputs
        .iter()
        .filter(|output| {
            !graph.connections.iter().any(|edge| {
                let (target, target_port) = edge.target();
                if target != output.name || !target_port.is_empty() {
                    return false;
                }
                let (source, source_port) = edge.source();
                graph.blocks.iter().any(|block| {
                    block.name == source && cdl_output_has_block_driver(block, &source_port)
                })
            })
        })
        .map(|output| {
            format!(
                "Building output {} must have a CDL block driver",
                output.name
            )
        })
        .collect()
}

fn g36_control_contract(graph: &Block) -> Vec<String> {
    if !is_g36_graph(graph) {
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
        let driven = graph.connections.iter().any(|edge| {
            if edge.target().0 != name {
                return false;
            }
            let (source, port) = edge.source();
            graph
                .blocks
                .iter()
                .any(|block| block.name == source && cdl_output_has_block_driver(block, &port))
        });
        if !declared || !driven {
            errors.push(format!(
                "G36 simulation requires a CDL-driven {name} output"
            ));
        }
    }
    errors
}

fn conventional_control_contract(graph: &Block) -> Vec<String> {
    if is_g36_graph(graph)
        || !graph
            .blocks
            .iter()
            .any(|block| block.block_type == "AHUController")
    {
        return Vec::new();
    }
    let mut errors = Vec::new();
    let plant_outputs = [
        "hot_water_enable",
        "chilled_water_enable",
        "hot_water_pump_speed",
        "chilled_water_pump_speed",
        "hot_water_supply_setpoint_c",
        "chilled_water_supply_setpoint_c",
        "heat_pump_heating_kw",
        "heat_pump_cooling_kw",
        "boiler_kw",
        "chiller_kw",
        "heat_pump_requested_mode",
        "heat_pump_start_fault",
        "boiler_start_fault",
        "chiller_start_fault",
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
        "airflow_request",
        "damper_position",
        "reheat_valve_position",
    ];
    let names = std::iter::once("preconditioning_mode".to_string())
        .chain((1..=10).map(|zone| format!("zone_{zone:02}_occupancy_confirmed")))
        .chain(
            plant_outputs
                .into_iter()
                .map(|port| format!("plant_{port}"))
                .chain(
                    (1..=2)
                        .flat_map(|ahu| ahu_outputs.map(move |port| format!("ahu_{ahu}_{port}"))),
                )
                .chain((1..=10).flat_map(|zone| {
                    zone_outputs.map(move |port| format!("zone_{zone:02}_{port}"))
                })),
        );
    for name in names {
        let driven = graph.connections.iter().any(|edge| {
            if edge.target().0 != name {
                return false;
            }
            let (source, port) = edge.source();
            graph
                .blocks
                .iter()
                .any(|block| block.name == source && cdl_output_has_block_driver(block, &port))
        });
        if !graph.outputs.iter().any(|output| output.name == name) || !driven {
            errors.push(format!(
                "Building simulation requires a CDL-driven {name} output"
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
        errors.extend(conventional_control_contract(&graph));
        errors.extend(undriven_building_outputs(&graph));
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
            || scenario
                .terminal_design_flow
                .iter()
                .any(|flow| !flow.is_finite() || *flow <= 0.0 || *flow > 5.0)
            || scenario
                .ahu_fan_design_airflow
                .iter()
                .any(|flow| !flow.is_finite() || *flow <= 0.0 || *flow > 20.0)
            || scenario
                .water_pump_design_flow_kg_s
                .iter()
                .any(|flow| !flow.is_finite() || *flow <= 0.0 || *flow > 20.0)
            || scenario
                .water_pump_shutoff_pressure_pa
                .iter()
                .any(|pressure| {
                    !pressure.is_finite() || *pressure < 10_000.0 || *pressure > 500_000.0
                })
            || !scenario.zone_air_volume_m3.is_finite()
            || scenario.zone_air_volume_m3 <= 0.0
            || scenario.zone_air_volume_m3 > 10_000.0
            || !scenario.zone_infiltration_m3_s.is_finite()
            || scenario.zone_infiltration_m3_s < 0.0
            || scenario.zone_infiltration_m3_s > 5.0
            || !scenario
                .open_window_additional_infiltration_m3_s
                .is_finite()
            || scenario.open_window_additional_infiltration_m3_s < 0.0
            || scenario.open_window_additional_infiltration_m3_s > 5.0
            || !scenario.design_occupants_per_zone.is_finite()
            || scenario.design_occupants_per_zone < 0.0
            || scenario.design_occupants_per_zone > 1_000.0
            || !scenario.co2_generation_lps_per_person.is_finite()
            || scenario.co2_generation_lps_per_person < 0.0
            || scenario.co2_generation_lps_per_person > 0.1
            || !scenario.outdoor_co2_ppm.is_finite()
            || !(250.0..=2000.0).contains(&scenario.outdoor_co2_ppm)
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
            || scenario.fan_faults.iter().any(|fault| {
                !matches!(
                    fault.equipment.as_str(),
                    "ahu_1_stuck_off"
                        | "ahu_1_stuck_on"
                        | "ahu_2_stuck_off"
                        | "ahu_2_stuck_on"
                        | "ahu_1_speed_capped"
                        | "ahu_2_speed_capped"
                ) || !fault.start_hour.is_finite()
                    || fault.start_hour < 0.0
                    || !fault.duration_hours.is_finite()
                    || fault.duration_hours <= 0.0
            })
            || scenario.terminal_faults.iter().any(|fault| {
                !(1..=10).contains(&fault.zone)
                    || !matches!(
                        fault.kind.as_str(),
                        "damper_stuck_open" | "reheat_valve_leak"
                    )
                    || !fault.start_hour.is_finite()
                    || fault.start_hour < 0.0
                    || !fault.duration_hours.is_finite()
                    || fault.duration_hours <= 0.0
            })
            || scenario.ahu_damper_faults.iter().any(|fault| {
                !(1..=2).contains(&fault.ahu)
                    || !matches!(fault.damper.as_str(), "outdoor" | "return")
                    || !fault.stuck_position.is_finite()
                    || !(0.0..=1.0).contains(&fault.stuck_position)
                    || !fault.start_hour.is_finite()
                    || fault.start_hour < 0.0
                    || !fault.duration_hours.is_finite()
                    || fault.duration_hours <= 0.0
            })
            || scenario.outdoor_temperature_events.iter().any(|event| {
                !event.delta_c.is_finite()
                    || event.delta_c.abs() > 25.0
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
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
            || scenario.alarm_acknowledgements.iter().any(|event| {
                !(1..=10).contains(&event.zone)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.ahu_alarm_acknowledgements.iter().any(|event| {
                !(1..=2).contains(&event.ahu)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.plant_alarm_acknowledgements.iter().any(|event| {
                !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
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
        let initial_co2_ppm = scenario.outdoor_co2_ppm;
        Ok(Self {
            graph,
            weather,
            seconds_until_occupied,
            scenario,
            effective_step_seconds,
            executor: Executor::default(),
            compiled,
            temps: [22.0; 10],
            co2_ppm: [initial_co2_ppm; 10],
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
            last_fan_running: [false; 2],
            last_actual_fan_speed: [0.0; 2],
            last_outdoor_damper_position: [0.0; 2],
            last_return_damper_position: [1.0; 2],
            last_zone_discharge_c: [16.0; 10],
            last_zone_damper_position: [0.0; 10],
            last_zone_reheat_valve_position: [0.0; 10],
            last_water_supply_c: [45.0, 7.0],
            last_water_return_c: [45.0, 7.0],
            last_water_flow_kg_s: [0.0; 2],
            last_water_dp_pa: [0.0; 2],
            source_start_elapsed_s: [0.0; 3],
            source_running: [false; 3],
            last_hot_water_plant_proven: false,
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
            let outdoor_c = w.outdoor_c
                + self
                    .scenario
                    .outdoor_temperature_events
                    .iter()
                    .filter(|event| {
                        elapsed_hours >= event.start_hour
                            && elapsed_hours < event.start_hour + event.duration_hours
                    })
                    .map(|event| event.delta_c)
                    .sum::<f64>();
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
            control_inputs.insert("outdoor_c".into(), json!(outdoor_c));
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
            control_inputs.insert(
                "hot_water_plant_proven".into(),
                json!(self.last_hot_water_plant_proven),
            );
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
            let plant_alarm_acknowledge =
                self.scenario
                    .plant_alarm_acknowledgements
                    .iter()
                    .any(|event| {
                        elapsed_hours >= event.start_hour
                            && elapsed_hours < event.start_hour + event.duration_hours
                    });
            control_inputs.insert(
                "plant_alarm_acknowledge".into(),
                json!(plant_alarm_acknowledge),
            );
            let (hp_capacity_fraction, heating_cop, cooling_cop) =
                heat_pump_performance(&self.scenario.heat_pump_performance, outdoor_c);
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
                let alarm_acknowledge =
                    self.scenario
                        .ahu_alarm_acknowledgements
                        .iter()
                        .any(|event| {
                            event.ahu == ahu + 1
                                && elapsed_hours >= event.start_hour
                                && elapsed_hours < event.start_hour + event.duration_hours
                        });
                control_inputs.insert(
                    format!("ahu_{}_alarm_acknowledge", ahu + 1),
                    json!(alarm_acknowledge),
                );
                control_inputs.insert(
                    format!("ahu_{}_supply_fan_proven", ahu + 1),
                    json!(self.last_fan_running[ahu]),
                );
                control_inputs.insert(
                    format!("ahu_{}_actual_fan_speed", ahu + 1),
                    json!(self.last_actual_fan_speed[ahu]),
                );
                control_inputs.insert(
                    format!("ahu_{}_actual_outdoor_damper_position", ahu + 1),
                    json!(self.last_outdoor_damper_position[ahu]),
                );
                control_inputs.insert(
                    format!("ahu_{}_actual_return_damper_position", ahu + 1),
                    json!(self.last_return_damper_position[ahu]),
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
            let mut window_closed = [true; 10];
            for zone in 0..10 {
                let name = format!("zone_{:02}", zone + 1);
                let raw = raw_zone_occupancy(index, zone, w.occupied);
                self.raw_occupancy[zone] = raw;
                control_inputs.insert(format!("{name}_occupied"), json!(raw));
                control_inputs.insert(
                    format!("{name}_occupancy_count"),
                    json!(if raw {
                        self.scenario.design_occupants_per_zone
                    } else {
                        0.0
                    }),
                );
                window_closed[zone] = !self.scenario.window_openings.iter().any(|opening| {
                    opening.zone == zone + 1
                        && elapsed_hours >= opening.start_hour
                        && elapsed_hours < opening.start_hour + opening.duration_hours
                });
                control_inputs.insert(format!("{name}_window_closed"), json!(window_closed[zone]));
                let local_override = self.scenario.occupancy_overrides.iter().any(|event| {
                    event.zone == zone + 1
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                });
                control_inputs.insert(format!("{name}_override"), json!(local_override));
                let alarm_acknowledge = self.scenario.alarm_acknowledgements.iter().any(|event| {
                    event.zone == zone + 1
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                });
                control_inputs.insert(
                    format!("{name}_alarm_acknowledge"),
                    json!(alarm_acknowledge),
                );
                // Older saved projects may still use the former low-flow-only
                // raw contact name in custom wiring.
                control_inputs.insert(
                    format!("{name}_low_airflow_acknowledge"),
                    json!(alarm_acknowledge),
                );
                control_inputs.insert(format!("{name}_room_temp"), json!(self.temps[zone]));
                control_inputs.insert(format!("{name}_co2_ppm"), json!(self.co2_ppm[zone]));
                control_inputs.insert(
                    format!("{name}_measured_airflow"),
                    json!(self.last_zone_airflow[zone]),
                );
                control_inputs.insert(
                    format!("{name}_discharge_temp"),
                    json!(self.last_zone_discharge_c[zone]),
                );
                control_inputs.insert(
                    format!("{name}_actual_damper_position"),
                    json!(self.last_zone_damper_position[zone]),
                );
                control_inputs.insert(
                    format!("{name}_actual_reheat_valve_position"),
                    json!(self.last_zone_reheat_valve_position[zone]),
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
            let preconditioning = control_flag("preconditioning_mode")
                .ok_or_else(|| JsValue::from_str("Missing CDL preconditioning_mode output"))?;
            for zone in 0..10 {
                let name = format!("zone_{:02}_occupancy_confirmed", zone + 1);
                self.zone_occupied[zone] = control_flag(&name)
                    .ok_or_else(|| JsValue::from_str(&format!("Missing CDL {name} output")))?;
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
            // The physical machine accepts only the services selected by the
            // editable CDL mode output; the harness does not choose a mode.
            let heat_pump_mode = output("plant_heat_pump_requested_mode")?;
            if ![0.0, 1.0, 2.0, 3.0].contains(&heat_pump_mode) {
                return Err(JsValue::from_str(
                    "CDL plant_heat_pump_requested_mode must be 0 off, 1 heating, 2 cooling, or 3 simultaneous",
                ));
            }
            let hp_heat_command = if heat_pump_mode == 1.0 || heat_pump_mode == 3.0 {
                output("plant_heat_pump_heating_kw")?.max(0.0)
            } else {
                0.0
            };
            let hp_cool_command = if heat_pump_mode == 2.0 || heat_pump_mode == 3.0 {
                output("plant_heat_pump_cooling_kw")?.max(0.0)
            } else {
                0.0
            };
            let boiler_command = output("plant_boiler_kw")?.max(0.0);
            let chiller_command = output("plant_chiller_kw")?.max(0.0);
            let source_start_faults = ["heat_pump", "boiler", "chiller"].map(|equipment| {
                let name = format!("plant_{equipment}_start_fault");
                control_flag(&name)
                    .ok_or_else(|| JsValue::from_str(&format!("Missing CDL {name} output")))
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
            // Motor status is physical feedback, distinct from the CDL command
            // and from delivered terminal airflow. Fault intervals affect the
            // motor and air/energy physics; only CDL decides the alarm response.
            let actual_fan_speed = [0, 1].map(|ahu| {
                let stopped = !equipment_available(
                    &self.scenario.fan_faults,
                    &format!("ahu_{}_stuck_off", ahu + 1),
                    elapsed_hours,
                );
                let stuck_on = !equipment_available(
                    &self.scenario.fan_faults,
                    &format!("ahu_{}_stuck_on", ahu + 1),
                    elapsed_hours,
                );
                let speed_capped = !equipment_available(
                    &self.scenario.fan_faults,
                    &format!("ahu_{}_speed_capped", ahu + 1),
                    elapsed_hours,
                );
                let speed = if stopped {
                    0.0
                } else if stuck_on {
                    clamp(fan_speed[ahu], 0.2, 1.0)
                } else if speed_capped {
                    clamp(fan_speed[ahu], 0.0, 0.3)
                } else {
                    clamp(fan_speed[ahu], 0.0, 1.0)
                };
                self.last_fan_running[ahu] = speed > 0.01;
                speed
            });
            self.last_actual_fan_speed = actual_fan_speed;
            let mut outdoor_air_fraction = [0.0; 2];
            let mut intake_opening = [1.0; 2];
            let mut actual_outdoor_damper_position = [0.0; 2];
            let mut actual_return_damper_position = [1.0; 2];
            for (ahu, fraction) in outdoor_air_fraction.iter_mut().enumerate() {
                let prefix = format!("ahu_{}", ahu + 1);
                let outdoor_name = format!("{prefix}_outdoor_air_damper_position");
                let return_name = format!("{prefix}_return_air_damper_position");
                let outdoor = ahu_damper_position(
                    &self.scenario.ahu_damper_faults,
                    ahu + 1,
                    "outdoor",
                    elapsed_hours,
                    clamp(output(&outdoor_name)?, 0.0, 1.0),
                );
                let return_air = ahu_damper_position(
                    &self.scenario.ahu_damper_faults,
                    ahu + 1,
                    "return",
                    elapsed_hours,
                    clamp(output(&return_name)?, 0.0, 1.0),
                );
                actual_outdoor_damper_position[ahu] = outdoor;
                actual_return_damper_position[ahu] = return_air;
                let open_area = outdoor + return_air;
                intake_opening[ahu] = clamp(open_area, 0.0, 1.0);
                *fraction = if open_area > 0.0 {
                    outdoor / open_area
                } else {
                    0.0
                };
            }
            self.last_outdoor_damper_position = actual_outdoor_damper_position;
            self.last_return_damper_position = actual_return_damper_position;
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
            // The harness applies physical actuator faults after reading CDL
            // commands. A stuck damper changes actual opening and airflow. A
            // leaking valve passes water with its actuator still closed.
            let actual_damper_position = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map(|zone| {
                if terminal_fault_active(
                    &self.scenario.terminal_faults,
                    zone + 1,
                    "damper_stuck_open",
                    elapsed_hours,
                ) {
                    1.0
                } else {
                    damper_command[zone]
                }
            });
            let actual_reheat_valve_position = reheat_command;
            let effective_reheat_opening = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map(|zone| {
                if terminal_fault_active(
                    &self.scenario.terminal_faults,
                    zone + 1,
                    "reheat_valve_leak",
                    elapsed_hours,
                ) {
                    actual_reheat_valve_position[zone].max(0.3)
                } else {
                    actual_reheat_valve_position[zone]
                }
            });
            // Two parallel valve groups and a small bypass form a simplified
            // hydraulic network. The remote differential pressure follows a
            // quadratic pump curve and resistance law; opening valves raises
            // flow and lowers pressure at a fixed pump speed.
            let hot_valve_area = 0.2
                + 0.4 * heating_coil_valve.iter().sum::<f64>() / 2.0
                + 0.4 * effective_reheat_opening.iter().sum::<f64>() / 10.0;
            let chilled_valve_area = 0.2 + 0.8 * cooling_coil_valve.iter().sum::<f64>() / 2.0;
            let valve_area = [hot_valve_area, chilled_valve_area];
            let pump_speed = [hot_water_pump_speed, chilled_water_pump_speed];
            let hydraulics = [0, 1].map(|loop_index| {
                water_loop_hydraulics(
                    pump_speed[loop_index],
                    valve_area[loop_index],
                    self.scenario.water_pump_design_flow_kg_s[loop_index],
                    self.scenario.water_pump_shutoff_pressure_pa[loop_index],
                )
            });
            let water_dp_pa = hydraulics.map(|(pressure, _)| pressure);
            let water_flow_kg_s = hydraulics.map(|(_, flow)| flow);
            // The harness resolves a simple fan/terminal air network from CDL
            // actuator commands. Airflow setpoints do not directly deliver air.
            // At pressure 1 each fully open terminal passes its configured
            // physical design airflow; each AHU has its own full-speed fan
            // capacity. CDL independently chooses airflow requests.
            let damper_area = [0, 1].map(|ahu| {
                (ahu * 5..ahu * 5 + 5)
                    .map(|zone| {
                        self.scenario.terminal_design_flow[zone] * actual_damper_position[zone]
                    })
                    .sum::<f64>()
            });
            let duct_pressure = [0, 1].map(|ahu| {
                if damper_area[ahu] > 0.0 {
                    clamp(
                        (self.scenario.ahu_fan_design_airflow[ahu]
                            * actual_fan_speed[ahu]
                            * intake_opening[ahu]
                            / damper_area[ahu])
                            .powi(2),
                        0.0,
                        1.0,
                    )
                } else if actual_fan_speed[ahu] > 0.0 && intake_opening[ahu] > 0.0 {
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
                delivered_flow[zone] = self.scenario.terminal_design_flow[zone]
                    * actual_damper_position[zone]
                    * duct_pressure[ahu].sqrt();
                delivered_by_ahu[ahu] += delivered_flow[zone];
                let discharge_limit_kw =
                    delivered_flow[zone] * 1.2 * (40.0 - supply_setpoint[ahu]).max(0.0);
                reheat_request[zone] =
                    (effective_reheat_opening[zone] * 8.0).min(discharge_limit_kw);
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
                    + outdoor_air_fraction[ahu] * outdoor_c;
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
            // Each zone is a mixed air volume. The prior zone concentrations
            // determine AHU return concentration; actual outdoor-damper and
            // terminal flows determine the next measured concentrations.
            // This transport model never decides a ventilation command.
            let return_co2_ppm = [0, 1].map(|ahu| {
                let start = ahu * 5;
                if delivered_by_ahu[ahu] > 0.0 {
                    (start..start + 5)
                        .map(|zone| delivered_flow[zone] * self.co2_ppm[zone])
                        .sum::<f64>()
                        / delivered_by_ahu[ahu]
                } else {
                    self.co2_ppm[start..start + 5].iter().sum::<f64>() / 5.0
                }
            });
            let supply_co2_ppm = [0, 1].map(|ahu| {
                outdoor_air_fraction[ahu] * self.scenario.outdoor_co2_ppm
                    + (1.0 - outdoor_air_fraction[ahu]) * return_co2_ppm[ahu]
            });
            for zone in 0..10 {
                // Teaching conversion: one normalized airflow unit is 1 m³/s.
                let airflow = delivered_flow[zone];
                let infiltration = self.scenario.zone_infiltration_m3_s
                    + if window_closed[zone] {
                        0.0
                    } else {
                        self.scenario.open_window_additional_infiltration_m3_s
                    };
                let total_flow = airflow + infiltration;
                let source_ppm_m3_s = if self.raw_occupancy[zone] {
                    self.scenario.design_occupants_per_zone
                        * self.scenario.co2_generation_lps_per_person
                        * 1000.0
                } else {
                    0.0
                };
                let outside_source = airflow * supply_co2_ppm[zone / 5]
                    + infiltration * self.scenario.outdoor_co2_ppm
                    + source_ppm_m3_s;
                self.co2_ppm[zone] = if total_flow > 0.0 {
                    let equilibrium = outside_source / total_flow;
                    let decay = (-total_flow * dt_seconds / self.scenario.zone_air_volume_m3).exp();
                    equilibrium + (self.co2_ppm[zone] - equilibrium) * decay
                } else {
                    self.co2_ppm[zone]
                        + source_ppm_m3_s * dt_seconds / self.scenario.zone_air_volume_m3
                };
            }
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
                // Closed-window leakage is already included in the effective envelope UA.
                // Only the additional open-window exchange adds sensible heat here.
                let window_exchange_kw = if window_closed[zone] {
                    0.0
                } else {
                    1.2 * self.scenario.open_window_additional_infiltration_m3_s
                        * (outdoor_c - self.temps[zone])
                };
                let load = self.scenario.zone_ua_kw_per_k * (outdoor_c - self.temps[zone])
                    + solar_share
                    + w.internal_kw / 10.0
                    + air_exchange_kw
                    + window_exchange_kw;
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
            self.last_zone_damper_position = actual_damper_position;
            self.last_zone_reheat_valve_position = actual_reheat_valve_position;
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
            // This is physical heating production from the prior step, not a
            // command or a control decision. Cooling-only heat-pump operation
            // cannot prove the hot-water plant for terminal alarm gating.
            self.last_hot_water_plant_proven = source_heat_kw > 0.01;
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
            let fan_kw = actual_fan_speed
                .iter()
                .map(|s| 2.0 * clamp(*s, 0.0, 1.0).powi(3))
                .sum::<f64>();
            let water_pump_kw =
                3.0 * (hot_water_pump_speed.powi(3) + chilled_water_pump_speed.powi(3));
            // A CDL request for simultaneous service shares one compressor.
            // The remaining heat exchange goes to or from ambient air.
            let heat_pump_electric_kw = if heat_pump_mode == 3.0 {
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
                let mut row = json!({"index":index,"schedule_occupied":if w.occupied {1.0} else {0.0},"preconditioning_mode":if preconditioning {1.0} else {0.0},"zone_occupied":self.zone_occupied.map(|occupied|if occupied {1.0} else {0.0}),"zone_presence_raw":self.raw_occupancy.map(|occupied|if occupied {1.0} else {0.0}),"zone_airflow":delivered_flow,"zone_discharge_c":discharge_c,"outdoor_c":outdoor_c,"avg_zone_c":self.temps.iter().sum::<f64>()/10.0,"zone_c":self.temps,"cool_kw":cool_kw,"heat_kw":heat_kw,"cool_request_kw":cool_request,"heat_request_kw":heat_request,"electric_kw":electric_kw,"gas_kw":gas_kw,"requested_airflow":requested_total.iter().sum::<f64>(),"delivered_airflow":delivered_by_ahu.iter().sum::<f64>(),"ahu_1_supply_actual_c":actual_supply_c[0],"ahu_2_supply_actual_c":actual_supply_c[1],"ahu_1_supply_setpoint_c":supply_setpoint[0],"ahu_2_supply_setpoint_c":supply_setpoint[1],"ahu_1_duct_pressure":duct_pressure[0],"ahu_2_duct_pressure":duct_pressure[1],"ahu_1_outdoor_air_fraction":outdoor_air_fraction[0],"ahu_2_outdoor_air_fraction":outdoor_air_fraction[1],"ahu_1_intake_opening":intake_opening[0],"ahu_2_intake_opening":intake_opening[1],"ahu_1_heating_coil_valve_position":heating_coil_valve[0],"ahu_2_heating_coil_valve_position":heating_coil_valve[1],"ahu_1_cooling_coil_valve_position":cooling_coil_valve[0],"ahu_2_cooling_coil_valve_position":cooling_coil_valve[1],"ahu_1_outdoor_airflow":outdoor_airflow[0],"ahu_2_outdoor_airflow":outdoor_airflow[1],"ahu_1_mixed_air_c":mixed_air_c[0],"ahu_2_mixed_air_c":mixed_air_c[1],"heat_pump_heating_used_kw":hp_heat_used,"heat_pump_cooling_used_kw":hp_cool_used,"boiler_used_kw":boiler_used,"chiller_used_kw":chiller_used});
                row.as_object_mut()
                    .unwrap()
                    .insert("zone_co2_ppm".into(), json!(self.co2_ppm));
                row.as_object_mut()
                    .unwrap()
                    .insert("ahu_1_actual_fan_speed".into(), json!(actual_fan_speed[0]));
                row.as_object_mut()
                    .unwrap()
                    .insert("ahu_2_actual_fan_speed".into(), json!(actual_fan_speed[1]));
                for ahu in 0..2 {
                    row.as_object_mut().unwrap().insert(
                        format!("ahu_{}_actual_outdoor_damper_position", ahu + 1),
                        json!(actual_outdoor_damper_position[ahu]),
                    );
                    row.as_object_mut().unwrap().insert(
                        format!("ahu_{}_actual_return_damper_position", ahu + 1),
                        json!(actual_return_damper_position[ahu]),
                    );
                }
                row.as_object_mut().unwrap().insert(
                    "zone_actual_damper_position".into(),
                    json!(actual_damper_position),
                );
                row.as_object_mut().unwrap().insert(
                    "zone_actual_reheat_valve_position".into(),
                    json!(actual_reheat_valve_position),
                );
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
                row.as_object_mut().unwrap().insert(
                    "hot_water_plant_proven".into(),
                    json!(if self.last_hot_water_plant_proven {
                        1.0
                    } else {
                        0.0
                    }),
                );
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
    fn pid_with_reset_uses_rising_fan_proof_in_both_executors() {
        let graph: Block = serde_json::from_value(json!({
            "name":"DamperLoop","block_type":"Composite","category":"composite",
            "inputs":[{"name":"u_s","type":"Real"},{"name":"u_m","type":"Real"},{"name":"fan","type":"Boolean"}],
            "outputs":[{"name":"y","type":"Real"}],
            "blocks":[{"name":"pi","block_type":"Buildings.Controls.OBC.CDL.Reals.PIDWithReset","category":"elementary",
                "inputs":[{"name":"u_s","type":"Real"},{"name":"u_m","type":"Real"},{"name":"trigger","type":"Boolean"}],
                "outputs":[{"name":"y","type":"Real"}],
                "parameters":[{"name":"k","value":0.5},{"name":"Ti","value":300.0},{"name":"r","value":1.0},
                    {"name":"Ni","value":0.9},{"name":"yMin","value":0.0},{"name":"yMax","value":1.0},
                    {"name":"y_reset","value":0.01},{"name":"reverseActing","value":true,"type":"Boolean"}]}],
            "connections":[{"from":"u_s","to":"pi.u_s"},{"from":"u_m","to":"pi.u_m"},
                {"from":"fan","to":"pi.trigger"},{"from":"pi.y","to":"y"}]
        })).unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        let samples = [
            (false, 0.0, 0.0),
            (true, 0.0, 0.01),
            (true, 0.2, 0.13),
            (true, 0.2, 0.15),
            (false, 0.2, 0.17),
            (true, 0.2, 0.01),
        ];
        for (fan, setpoint, expected) in samples {
            let inputs = HashMap::from([
                ("u_s".into(), json!(setpoint)),
                ("u_m".into(), json!(0.0)),
                ("fan".into(), json!(fan)),
            ]);
            let actual = dynamic.execute(&graph, &inputs, 60.0, "").unwrap()["y"]
                .as_f64()
                .unwrap();
            let faster = compiled.execute(&inputs, 60.0, false).unwrap().0["y"]
                .as_f64()
                .unwrap();
            assert!((actual - faster).abs() < 1e-12);
            assert!(
                (actual - expected).abs() < 1e-12,
                "fan={fan} setpoint={setpoint}: {actual}"
            );
        }
    }
    #[test]
    fn pid_with_reset_pi_matches_pinned_obc_trace() {
        let source = include_str!("../../verification/g36_pid_with_reset_dymola.txt");
        let series = |name: &str| -> Vec<f64> {
            let line = source.lines().find(|line| line.starts_with(name)).unwrap();
            line.split_once('=')
                .unwrap()
                .1
                .trim_matches(['[', ']'])
                .split(',')
                .map(|field| field.trim().parse::<f64>().unwrap())
                .collect()
        };
        for (name, reset) in [("limPIPar", 0.5), ("limPIInp", 0.75)] {
            let measured = series(&format!("{name}.u_m="));
            let reference = series(&format!("{name}.y="));
            assert_eq!((measured.len(), reference.len()), (101, 101));
            let block: Block = serde_json::from_value(json!({
                "name": "pi", "block_type": "Buildings.Controls.OBC.CDL.Reals.PIDWithReset",
                "inputs": [{"name":"u_s","type":"Real"},{"name":"u_m","type":"Real"},
                    {"name":"trigger","type":"Boolean"}],
                "outputs": [{"name":"y","type":"Real"}],
                "parameters": [{"name":"k","value":1.0},{"name":"Ti","value":1.0},
                    {"name":"Ni","value":0.9},{"name":"yMin","value":-1.0},
                    {"name":"yMax","value":1.0},{"name":"y_reset","value":reset},
                    {"name":"reverseActing","type":"Boolean","value":true}]
            }))
            .unwrap();
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(&block).unwrap().unwrap();
            let mut max_error = 0.0_f64;
            for step in 0..=10_000 {
                let sample = (step / 100).min(100);
                let next = (sample + 1).min(100);
                let fraction = (step % 100) as f64 / 100.0;
                let measurement = measured[sample] * (1.0 - fraction) + measured[next] * fraction;
                let triggered = step >= 1_000;
                let inputs = HashMap::from([
                    ("u_s".into(), json!(if triggered { 1.0 } else { 0.0 })),
                    ("u_m".into(), json!(measurement)),
                    ("trigger".into(), json!(triggered)),
                ]);
                let regular = dynamic.execute(&block, &inputs, 0.001, "").unwrap()["y"]
                    .as_f64()
                    .unwrap();
                let faster = compiled.execute(&inputs, 0.001, false).unwrap().0["y"]
                    .as_f64()
                    .unwrap();
                assert!((regular - faster).abs() < 1e-12, "{name} step {step}");
                // Dymola records t=1 immediately before the rising-edge reset.
                if step % 100 == 0 && step != 1_000 {
                    max_error = max_error.max((regular - reference[step / 100]).abs());
                }
            }
            assert!(max_error < 5e-4, "{name}: max error {max_error}");
        }
    }
    #[test]
    fn g36_vav_damper_loops_reset_from_physical_fan_proof() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        assert!(validate(&graph).is_empty());
        for zone in graph
            .blocks
            .iter()
            .filter(|block| block.block_type == "VAVBoxController")
        {
            let pi = zone
                .blocks
                .iter()
                .find(|block| block.name == "damper_flow_pi")
                .unwrap();
            assert_eq!(
                pi.block_type,
                "Buildings.Controls.OBC.CDL.Reals.PIDWithReset"
            );
            assert_eq!(param(pi, "y_reset", -1.0), 0.01);
            assert!(zone.connections.iter().any(|edge| {
                edge.source() == ("supply_fan_proven".into(), String::new())
                    && edge.target() == ("damper_flow_pi".into(), "trigger".into())
            }));
            assert!(CompiledExecutor::try_new(zone).unwrap().is_some());
        }
    }
    #[test]
    fn boolean_to_real_matches_obc_parameters_in_both_executors() {
        let graph: Block = serde_json::from_value(json!({
            "name": "bool_to_real",
            "block_type": "Buildings.Controls.OBC.CDL.Conversions.BooleanToReal",
            "inputs": [{"name": "u", "type": "Boolean"}],
            "outputs": [{"name": "y", "type": "Real"}],
            "parameters": [{"name": "realTrue", "value": 2.5}, {"name": "realFalse", "value": -0.5}]
        }))
        .unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (input, expected) in [(false, -0.5), (true, 2.5), (false, -0.5)] {
            let values = HashMap::from([("u".into(), json!(input))]);
            assert_eq!(
                dynamic.execute(&graph, &values, 60.0, "").unwrap()["y"],
                json!(expected)
            );
            assert_eq!(
                compiled.execute(&values, 60.0, false).unwrap().0["y"],
                json!(expected)
            );
        }
    }
    #[test]
    fn vav_shortcut_requires_expansion_into_cdl_blocks() {
        let legacy: Block = serde_json::from_value(json!({
            "name": "legacy_vav",
            "block_type": "VAVBoxController",
            "inputs": [{"name": "room_temp", "type": "Real"}],
            "outputs": [{"name": "damper_position", "type": "Real"}]
        }))
        .unwrap();
        assert!(validate(&legacy)
            .iter()
            .any(|error| error.contains("unsupported block type VAVBoxController")));
        assert!(Executor::default()
            .execute(&legacy, &HashMap::new(), 900.0, "")
            .is_err());
    }
    #[test]
    fn true_false_hold_respects_both_minimum_durations_in_both_executors() {
        let graph: Block = serde_json::from_value(json!({
            "name":"hold", "block_type":"Buildings.Controls.OBC.CDL.Logical.TrueFalseHold",
            "inputs":[{"name":"u","type":"Boolean"}],
            "outputs":[{"name":"y","type":"Boolean"}],
            "parameters":[{"name":"trueHoldDuration","value":120.0},
                          {"name":"falseHoldDuration","value":60.0}]
        }))
        .unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (input, expected) in [
            (false, false),
            (true, false),
            (true, true),
            (false, true),
            (false, true),
            (false, true),
            (false, false),
            (true, false),
            (true, true),
        ] {
            let inputs = HashMap::from([("u".into(), json!(input))]);
            assert_eq!(
                dynamic.execute(&graph, &inputs, 30.0, "").unwrap()["y"],
                json!(expected)
            );
            assert_eq!(
                compiled.execute(&inputs, 30.0, false).unwrap().0["y"],
                json!(expected)
            );
        }
        let mut invalid = graph.clone();
        invalid.parameters[0].value = json!(-1.0);
        assert!(!validate(&invalid).is_empty());
    }
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
                    json!(wanted)
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
            assert_eq!(optimized["passed"], json!(passed));
        }
    }
    #[test]
    fn water_hydraulics_couple_pump_speed_valve_area_pressure_and_flow() {
        let model = |speed, area| water_loop_hydraulics(speed, area, 4.31, 80_000.0);
        let (design_pressure, design_flow) = model(1.0, 1.0);
        assert_eq!(design_pressure, 40_000.0);
        assert_eq!(design_flow, 4.31);
        let (closed_pressure, closed_flow) = model(1.0, 0.2);
        assert!(closed_pressure > design_pressure);
        assert!(closed_flow < design_flow);
        let (slow_pressure, slow_flow) = model(0.5, 1.0);
        assert_eq!(slow_pressure, design_pressure / 4.0);
        assert_eq!(slow_flow, design_flow / 2.0);
        assert_eq!(model(0.0, 1.0), (0.0, 0.0));
        let (lower_head, lower_flow) = water_loop_hydraulics(1.0, 1.0, 4.31, 40_000.0);
        assert_eq!(lower_head, 20_000.0);
        assert!(lower_flow < design_flow);
        let (same_head, smaller_flow) = water_loop_hydraulics(1.0, 1.0, 2.0, 80_000.0);
        assert_eq!(same_head, design_pressure);
        assert_eq!(smaller_flow, 2.0);
    }
    #[test]
    fn physical_water_pump_curve_changes_measured_feedback_from_same_cdl_request() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: -5.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        let mut smaller = Simulation::new(
            graph,
            &weather,
            r#"{"step_seconds":900,"water_pump_design_flow_kg_s":[2.0,4.31],"water_pump_shutoff_pressure_pa":[40000,80000]}"#,
        )
        .unwrap();
        for sim in [&mut normal, &mut smaller] {
            start_present_window(sim);
            sim.temps = [16.0; 10];
            sim.last_heat_request = 100.0;
            sim.last_water_flow_kg_s = [2.0, 2.0];
            sim.step_chunk(1).unwrap();
        }
        assert!(normal.last_water_dp_pa[0] > 0.0);
        assert!(normal.last_water_flow_kg_s[0] > 0.0);
        assert!(smaller.last_water_dp_pa[0] < normal.last_water_dp_pa[0]);
        assert!(smaller.last_water_flow_kg_s[0] < normal.last_water_flow_kg_s[0]);
        assert_eq!(smaller.last_water_dp_pa[1], normal.last_water_dp_pa[1]);
        assert_eq!(
            smaller.last_water_flow_kg_s[1],
            normal.last_water_flow_kg_s[1]
        );
    }
    fn add_ahu_ventilation_inputs(inputs: &mut HashMap<String, Value>) {
        inputs.entry("operation_mode".into()).or_insert(json!(1));
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
                fast["heat_pump_start_fault"].as_bool().unwrap()
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
            fast["heat_pump_start_fault"].as_bool().unwrap()
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
                    fast[&fault_name].as_bool().unwrap()
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
    fn physical_vav_design_flow_changes_delivery_without_changing_cdl_flow_request() {
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
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
        let mut default = Simulation::new(graph, &weather, "{}").unwrap();
        let mut smaller = Simulation::new(
            graph,
            &weather,
            &json!({"terminal_design_flow": [0.25,0.95,0.95,0.95,0.95,0.95,0.95,0.95,0.95,0.95]})
                .to_string(),
        )
        .unwrap();
        start_present_window(&mut default);
        start_present_window(&mut smaller);
        let default_rows = run_weather_intervals(&mut default, 4);
        let smaller_rows = run_weather_intervals(&mut smaller, 4);
        let baseline = &default_rows["rows"][0];
        let reduced = &smaller_rows["rows"][0];
        assert_eq!(baseline["requested_airflow"], reduced["requested_airflow"]);
        assert!(
            baseline["zone_airflow"][0].as_f64().unwrap()
                > reduced["zone_airflow"][0].as_f64().unwrap() + 0.01
        );
        assert_ne!(
            default_rows["rows"][3]["zone_c"][0],
            smaller_rows["rows"][3]["zone_c"][0]
        );
    }
    #[test]
    fn physical_fan_design_flow_changes_delivery_and_cdl_pressure_requests() {
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
        let target = "FourPipeG36Controls|output.zone_01_pressure_reset_request";
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        let mut undersized = Simulation::new(
            graph,
            &weather,
            &json!({"ahu_fan_design_airflow": [1.0,5.0]}).to_string(),
        )
        .unwrap();
        for sim in [&mut normal, &mut undersized] {
            sim.set_traces(&json!([target]).to_string()).unwrap();
            start_present_window(sim);
            sim.temps = [28.0; 10];
        }
        let normal_rows = run_weather_intervals(&mut normal, 16);
        let undersized_rows = run_weather_intervals(&mut undersized, 16);
        let normal_first = &normal_rows["rows"][0];
        let undersized_first = &undersized_rows["rows"][0];
        assert_eq!(
            normal_first["ahu_1_actual_fan_speed"],
            undersized_first["ahu_1_actual_fan_speed"]
        );
        assert!(
            normal_first["zone_airflow"][0].as_f64().unwrap()
                > undersized_first["zone_airflow"][0].as_f64().unwrap() + 0.01
        );
        let normal_requests: Vec<f64> = normal_rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["traces"][target].as_f64().unwrap())
            .collect();
        let undersized_requests: Vec<f64> = undersized_rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["traces"][target].as_f64().unwrap())
            .collect();
        assert!(
            undersized_requests
                .iter()
                .zip(&normal_requests)
                .any(|(small, design)| small > design),
            "normal={normal_requests:?}, undersized={undersized_requests:?}"
        );
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
    fn conventional_requires_cdl_driven_physical_commands() {
        let mut graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        assert!(conventional_control_contract(&graph).is_empty());
        let missing = [
            "preconditioning_mode",
            "zone_01_occupancy_confirmed",
            "plant_heat_pump_heating_kw",
            "plant_heat_pump_requested_mode",
            "plant_heat_pump_start_fault",
            "ahu_1_fan_speed",
            "zone_01_reheat_valve_position",
        ];
        graph
            .connections
            .retain(|edge| !missing.contains(&edge.target().0.as_str()));
        let errors = conventional_control_contract(&graph);
        assert_eq!(errors.len(), missing.len());
        for name in missing {
            assert!(errors.iter().any(|error| error.contains(name)), "{name}");
        }

        let mut graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        let edge = graph
            .connections
            .iter_mut()
            .find(|edge| edge.target().0 == "zone_01_damper_position")
            .unwrap();
        edge.from_path = "zone_01_supply_pressure".into();
        edge.from_block.clear();
        edge.from_output.clear();
        assert!(conventional_control_contract(&graph)
            .iter()
            .any(|error| error.contains("zone_01_damper_position")));
    }
    #[test]
    fn conventional_heat_pump_mode_is_computed_by_editable_cdl() {
        let building: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        assert!(!is_g36_graph(&building));
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        for (heat, cool, expected) in [
            (0.0, 0.0, 0),
            (100.0, 0.0, 1),
            (0.0, 100.0, 2),
            (100.0, 100.0, 3),
        ] {
            let mut inputs: HashMap<String, Value> = plant
                .inputs
                .iter()
                .map(|port| (port.name.clone(), json!(0.0)))
                .collect();
            inputs.insert("heat_request_kw".into(), json!(heat));
            inputs.insert("cool_request_kw".into(), json!(cool));
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
            let slow = dynamic.execute(plant, &inputs, 900.0, "").unwrap();
            let fast = compiled.execute(&inputs, 900.0, false).unwrap().0;
            assert_eq!(slow["heat_pump_requested_mode"], json!(expected));
            assert_eq!(fast["heat_pump_requested_mode"], json!(expected));
        }
    }
    #[test]
    fn g36_damper_zero_flow_override_uses_pi_until_unoccupied() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut inputs: HashMap<String, Value> = zone
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else if port.kind == "Integer" {
                        json!(0)
                    } else {
                        json!(22.0)
                    },
                )
            })
            .collect();
        inputs.insert("scheduled_occupied".into(), json!(true));
        inputs.insert("supply_fan_proven".into(), json!(true));
        inputs.insert("measured_airflow".into(), json!(0.01));
        inputs.insert("flow_override_code".into(), json!(2));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
        let mut previous = 0.0;
        for _ in 0..10 {
            let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(slow["damper_position"], fast["damper_position"]);
            previous = slow["damper_position"].as_f64().unwrap();
        }
        assert!(previous > 0.5, "failed to prime damper PI: {previous}");
        inputs.insert("flow_override_code".into(), json!(1));
        let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow["airflow_request"], json!(0.0));
        assert_eq!(slow["damper_position"], fast["damper_position"]);
        assert!(
            slow["damper_position"].as_f64().unwrap() > 0.0,
            "zero-flow override must not bypass the CDL PI while occupied"
        );
        inputs.insert("scheduled_occupied".into(), json!(false));
        let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow["damper_position"], json!(0.0));
        assert_eq!(slow["damper_position"], fast["damper_position"]);
    }
    #[test]
    fn g36_vav_flow_overrides_replay_pinned_damper_valves_setpoint_trace() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let mut zone = building
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap()
            .clone();
        for (name, value) in [
            ("flow_override_cooling_max", 0.09),
            ("flow_override_design_min", 0.01),
            ("flow_override_heating_max", 0.08),
        ] {
            zone.blocks
                .iter_mut()
                .find(|block| block.name == name)
                .unwrap()
                .parameters
                .iter_mut()
                .find(|parameter| parameter.name == "k")
                .unwrap()
                .value = json!(value);
        }
        let archive = include_str!("../../verification/g36_damper_valves_dymola.txt");
        let series = |name: &str| -> Vec<f64> {
            archive
                .lines()
                .find(|line| line.starts_with(name))
                .unwrap()
                .split_once('=')
                .unwrap()
                .1
                .trim_matches(['[', ']'])
                .split(',')
                .map(|field| field.trim().parse::<f64>().unwrap())
                .collect()
        };
        let override_code = series("damValFan1.oveFloSet=");
        let reference_flow = series("damValFan1.VSet_flow=");
        assert_eq!((override_code.len(), reference_flow.len()), (101, 101));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&zone).unwrap().unwrap();
        let mut coverage = [0usize; 4];
        for (index, (&code, &expected)) in
            override_code.iter().zip(reference_flow.iter()).enumerate()
        {
            if !(1.0..=3.0).contains(&code) {
                continue;
            }
            coverage[code as usize] += 1;
            let mut inputs: HashMap<String, Value> = zone
                .inputs
                .iter()
                .map(|port| {
                    (
                        port.name.clone(),
                        if port.kind == "Boolean" {
                            json!(false)
                        } else if port.kind == "Integer" {
                            json!(0)
                        } else {
                            json!(22.0)
                        },
                    )
                })
                .collect();
            inputs.insert("flow_override_code".into(), json!(code as i64));
            let slow = dynamic.execute(&zone, &inputs, 72.0, "").unwrap();
            let fast = compiled.execute(&inputs, 72.0, false).unwrap().0;
            let actual = slow["airflow_request"].as_f64().unwrap();
            assert!(
                (actual - expected).abs() < 1e-6,
                "sample {index} code {code}: {actual} vs {expected}"
            );
            assert_eq!(slow["airflow_request"], fast["airflow_request"]);
        }
        assert!(
            coverage[1] > 0 && coverage[2] > 0 && coverage[3] > 0,
            "{coverage:?}"
        );
        let mut inputs: HashMap<String, Value> = zone
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else if port.kind == "Integer" {
                        json!(0)
                    } else {
                        json!(22.0)
                    },
                )
            })
            .collect();
        inputs.insert("flow_override_code".into(), json!(4));
        assert_eq!(
            dynamic.execute(&zone, &inputs, 72.0, "").unwrap()["airflow_request"],
            json!(0.08)
        );
    }
    #[test]
    fn g36_normal_vav_airflow_replays_unforced_obc_trace() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let normal = zone
            .blocks
            .iter()
            .find(|block| block.name == "normal_airflow_sequence")
            .unwrap();
        let archive = include_str!("../../verification/g36_damper_valves_dymola.txt");
        let series = |name: &str| -> Vec<f64> {
            archive
                .lines()
                .find(|line| line.starts_with(name))
                .unwrap()
                .split_once('=')
                .unwrap()
                .1
                .trim_matches(['[', ']'])
                .split(',')
                .map(|field| field.trim().parse::<f64>().unwrap())
                .collect()
        };
        let code = series("damValFan1.oveFloSet=");
        let expected = series("damValFan1.VSet_flow=");
        let cooling = series("damValFan1.uCoo=");
        let heating = series("damValFan1.uHea=");
        let supply = series("damValFan1.TSup=");
        let zone_temp = series("damValFan1.TZon=")[0] - 273.15;
        let discharge = series("damValFan1.TDis=");
        let cooling_min = series("damValFan1.VActCooMin_flow=")[0];
        let cooling_max = series("damValFan1.VActCooMax_flow=")[0];
        let minimum = series("damValFan1.VActMin_flow=")[0];
        let heating_min = series("damValFan1.VActHeaMin_flow=")[0];
        let heating_max = series("damValFan1.VActHeaMax_flow=")[0];
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(normal).unwrap().unwrap();
        let mut checked = 0;
        for index in 0..101 {
            let inputs = HashMap::from([
                ("cooling_signal".into(), json!(cooling[index])),
                ("heating_signal".into(), json!(heating[index])),
                ("cooling_min".into(), json!(cooling_min)),
                ("cooling_max".into(), json!(cooling_max)),
                ("minimum_flow".into(), json!(minimum)),
                ("heating_min".into(), json!(heating_min)),
                ("heating_max".into(), json!(heating_max)),
                ("supply_temp".into(), json!(supply[index] - 273.15)),
                ("zone_temp".into(), json!(zone_temp)),
                ("discharge_temp".into(), json!(discharge[index] - 273.15)),
                ("heating_warm_margin".into(), json!(3.0)),
            ]);
            let slow = dynamic.execute(normal, &inputs, 72.0, "").unwrap()["y"]
                .as_f64()
                .unwrap();
            let fast = compiled.execute(&inputs, 72.0, false).unwrap().0["y"]
                .as_f64()
                .unwrap();
            assert!(
                (slow - fast).abs() < 1e-10,
                "sample {index}: dynamic {slow}, compiled {fast}"
            );
            if code[index] == 0.0 {
                checked += 1;
                assert!(
                    (slow - expected[index]).abs() < 1e-5,
                    "sample {index}: {slow} vs {}",
                    expected[index]
                );
            }
        }
        assert!(checked >= 15, "only {checked} unforced samples checked");
    }
    #[test]
    fn g36_normal_vav_airflow_covers_deadband_and_heating_equations() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let normal = zone
            .blocks
            .iter()
            .find(|block| block.name == "normal_airflow_sequence")
            .unwrap();
        for (cool, heat, supply, discharge, expected) in [
            (0.0, 0.0, 14.0, 22.0, 0.15),
            (0.5, 0.0, 14.0, 22.0, 0.5),
            (0.5, 0.0, 25.0, 22.0, 0.15),
            (0.0, 0.8, 14.0, 22.0, 0.2),
            (0.0, 0.8, 14.0, 26.0, 0.62),
        ] {
            let inputs = HashMap::from([
                ("cooling_signal".into(), json!(cool)),
                ("heating_signal".into(), json!(heat)),
                ("cooling_min".into(), json!(0.1)),
                ("cooling_max".into(), json!(0.9)),
                ("minimum_flow".into(), json!(0.15)),
                ("heating_min".into(), json!(0.2)),
                ("heating_max".into(), json!(0.9)),
                ("supply_temp".into(), json!(supply)),
                ("zone_temp".into(), json!(22.0)),
                ("discharge_temp".into(), json!(discharge)),
                ("heating_warm_margin".into(), json!(3.0)),
            ]);
            let slow = Executor::default()
                .execute(normal, &inputs, 60.0, "")
                .unwrap()["y"]
                .as_f64()
                .unwrap();
            let fast = CompiledExecutor::try_new(normal)
                .unwrap()
                .unwrap()
                .execute(&inputs, 60.0, false)
                .unwrap()
                .0["y"]
                .as_f64()
                .unwrap();
            assert!(
                (slow - expected).abs() < 1e-10,
                "cool={cool}, heat={heat}, supply={supply}, discharge={discharge}: {slow}"
            );
            assert!(
                (fast - expected).abs() < 1e-10,
                "compiled cool={cool}, heat={heat}: {fast}"
            );
        }
    }
    #[test]
    fn g36_discharge_setpoint_replays_pinned_damper_valves_trace() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let names = [
            "discharge_upper",
            "discharge_max_rise",
            "discharge_span",
            "discharge_positive_span",
            "discharge_zero_rise",
            "discharge_demand_double",
            "discharge_demand_limit",
            "discharge_reset_rise",
            "discharge_reset_target",
            "discharge_target_cap",
            "maximum_discharge_temp",
            "discharge_target",
            "discharge_minimum",
            "discharge_baseline",
            "discharge_heat_detected",
            "discharge_heat_hold",
        ];
        let mut branch = serde_json::to_value(zone).unwrap();
        branch["name"] = json!("PinnedDamperValvesDischargeTarget");
        branch["inputs"] = json!([
            {"name":"heating_signal","type":"Real","unit":"fraction (0–1)"},
            {"name":"heating_setpoint","type":"Real","unit":"°C"},
            {"name":"supply_air_setpoint","type":"Real","unit":"°C"}
        ]);
        branch["outputs"] = json!([{"name":"y","type":"Real","unit":"°C"}]);
        branch["blocks"]
            .as_array_mut()
            .unwrap()
            .retain(|block| names.contains(&block["name"].as_str().unwrap()));
        let original_edges = branch["connections"].as_array().unwrap().clone();
        let mut edges: Vec<Value> = original_edges
            .iter()
            .filter_map(|edge| {
                let target = edge["to"].as_str().unwrap().split('.').next().unwrap();
                if !names.contains(&target) {
                    return None;
                }
                let source = match edge["from"].as_str().unwrap() {
                    "heat_pi.y" => "heating_signal",
                    "window_heat_target.y" => "heating_setpoint",
                    other => other,
                };
                assert!(
                    names.contains(&source.split('.').next().unwrap())
                        || ["heating_signal", "heating_setpoint", "supply_air_setpoint"]
                            .contains(&source),
                    "unexpected discharge target source {source}"
                );
                Some(json!({"from":source,"to":edge["to"]}))
            })
            .collect();
        edges.push(json!({"from":"discharge_target.y","to":"y"}));
        branch["connections"] = json!(edges);
        let branch: Block = serde_json::from_value(branch).unwrap();
        assert!(validate(&branch).is_empty());
        let archive = include_str!("../../verification/g36_damper_valves_dymola.txt");
        let series = |name: &str| -> Vec<f64> {
            archive
                .lines()
                .find(|line| line.starts_with(name))
                .unwrap()
                .split_once('=')
                .unwrap()
                .1
                .trim_matches(['[', ']'])
                .split(',')
                .map(|field| field.trim().parse::<f64>().unwrap())
                .collect()
        };
        let heating = series("damValFan1.uHea=");
        let expected = series("damValFan1.TDisSet=");
        let supply = series("damValFan1.TSupSet=")[0] - 273.15;
        let heating_setpoint = series("damValFan1.THeaSet=")[0] - 273.15;
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        for index in 0..101 {
            let inputs = HashMap::from([
                ("heating_signal".into(), json!(heating[index])),
                ("heating_setpoint".into(), json!(heating_setpoint)),
                ("supply_air_setpoint".into(), json!(supply)),
            ]);
            let slow = dynamic.execute(&branch, &inputs, 72.0, "").unwrap()["y"]
                .as_f64()
                .unwrap();
            let fast = compiled.execute(&inputs, 72.0, false).unwrap().0["y"]
                .as_f64()
                .unwrap();
            let reference = expected[index] - 273.15;
            assert!(
                (slow - reference).abs() < 0.001,
                "sample {index}: dynamic {slow}, reference {reference}"
            );
            assert!(
                (fast - reference).abs() < 0.001,
                "sample {index}: compiled {fast}, reference {reference}"
            );
        }
    }
    #[test]
    fn g36_reheat_pi_resets_when_cdl_enables_the_valve() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let pi = zone
            .blocks
            .iter()
            .find(|block| block.name == "discharge_valve_pi")
            .unwrap();
        assert_eq!(
            pi.block_type,
            "Buildings.Controls.OBC.CDL.Reals.PIDWithReset"
        );
        assert!(zone.connections.iter().any(|edge| {
            let (source, source_port) = edge.source();
            let (target, target_port) = edge.target();
            source == "discharge_valve_enable"
                && source_port == "y"
                && target == "discharge_valve_pi"
                && target_port == "trigger"
        }));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(pi).unwrap().unwrap();
        let mut inputs = HashMap::from([
            ("u_s".into(), json!(30.0)),
            ("u_m".into(), json!(15.0)),
            ("trigger".into(), json!(false)),
        ]);
        for _ in 0..30 {
            let slow = dynamic.execute(pi, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(slow["y"], fast["y"]);
        }
        inputs.insert("trigger".into(), json!(true));
        let slow = dynamic.execute(pi, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(slow["y"], json!(0.0));
        assert_eq!(fast["y"], json!(0.0));
        let slow = dynamic.execute(pi, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert!(slow["y"].as_f64().unwrap() > 0.0);
        assert_eq!(slow["y"], fast["y"]);
    }
    #[test]
    fn g36_damper_pi_replays_pinned_damper_valves_trace() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut pi = zone
            .blocks
            .iter()
            .find(|block| block.name == "damper_flow_pi")
            .unwrap()
            .clone();
        for parameter in &mut pi.parameters {
            if parameter.name == "k" {
                parameter.value = json!(1.0);
            }
            if parameter.name == "r" {
                parameter.value = json!(1.0);
            }
        }
        let archive = include_str!("../../verification/g36_damper_valves_dymola.txt");
        let expected: Vec<f64> = archive
            .lines()
            .find(|line| line.starts_with("damValFan1.yDam="))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|value| value.trim().parse().unwrap())
            .collect();
        assert_eq!(expected.len(), 101);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&pi).unwrap().unwrap();
        let mut largest = (0usize, 0.0_f64);
        // The validation's rounded ramp selects overrides 1, 2, and 3 at
        // 1334, 2000, and 2667 s. The 90%-width fan pulse starts at 180 s.
        for second in 0..=7200 {
            let t = second as f64;
            let setpoint = if second < 1334 {
                0.075
            } else if second < 2000 {
                0.0
            } else if second < 2667 {
                0.09
            } else {
                0.01
            };
            let fan = (180..4500).contains(&second) || second >= 4980;
            let inputs = HashMap::from([
                ("u_s".into(), json!(setpoint / 0.09)),
                ("u_m".into(), json!((0.01 + 0.06 * t / 7200.0) / 0.09)),
                ("trigger".into(), json!(fan)),
            ]);
            let slow = dynamic.execute(&pi, &inputs, 1.0, "").unwrap()["y"]
                .as_f64()
                .unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0["y"]
                .as_f64()
                .unwrap();
            assert!(
                (slow - fast).abs() < 1e-9,
                "second {second}: {slow} vs {fast}"
            );
            if second % 72 == 0 {
                let index = second / 72;
                let excess = (slow - expected[index]).abs();
                if excess > largest.1 {
                    largest = (index, excess);
                }
            }
        }
        assert!(
            largest.1 < 0.03,
            "damper excess {} at sample {}",
            largest.1,
            largest.0
        );
    }
    #[test]
    fn g36_vav_damper_valves_branches_replay_all_pinned_outputs() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut branch = serde_json::to_value(zone).unwrap();
        branch["name"] = json!("PinnedDamperValvesAirflowBranch");
        branch["inputs"] = json!([
            {"name":"cooling_signal","type":"Real"},
            {"name":"heating_signal","type":"Real"},
            {"name":"supply_temp","type":"Real"},
            {"name":"zone_temp","type":"Real"},
            {"name":"discharge_temp","type":"Real"},
            {"name":"cooling_min","type":"Real"},
            {"name":"cooling_max","type":"Real"},
            {"name":"minimum_flow","type":"Real"},
            {"name":"heating_min","type":"Real"},
            {"name":"heating_max","type":"Real"},
            {"name":"heating_warm_margin","type":"Real"},
            {"name":"flow_override_code","type":"Integer"},
            {"name":"measured_airflow","type":"Real"},
            {"name":"supply_fan_proven","type":"Boolean"},
            {"name":"mode","type":"Integer"},
            {"name":"heating_setpoint","type":"Real","unit":"°C"},
            {"name":"supply_air_setpoint","type":"Real","unit":"°C"},
            {"name":"occupied","type":"Boolean"}
        ]);
        branch["outputs"] = json!([
            {"name":"airflow_request","type":"Real"},
            {"name":"damper_position","type":"Real"},
            {"name":"discharge_setpoint","type":"Real","unit":"°C"},
            {"name":"reheat_valve_position","type":"Real"}
        ]);
        let reheat_names = [
            "discharge_upper",
            "discharge_max_rise",
            "discharge_span",
            "discharge_positive_span",
            "discharge_zero_rise",
            "discharge_demand_double",
            "discharge_demand_limit",
            "discharge_reset_rise",
            "discharge_reset_target",
            "discharge_target_cap",
            "maximum_discharge_temp",
            "discharge_target",
            "discharge_minimum",
            "discharge_baseline",
            "discharge_heat_detected",
            "discharge_heat_hold",
            "discharge_valve_pi",
            "discharge_below_minimum",
            "discharge_minimum_occupied",
            "discharge_valve_enable",
            "discharge_valve_closed",
            "discharge_valve_select",
        ];
        branch["blocks"].as_array_mut().unwrap().retain(|block| {
            let name = block["name"].as_str().unwrap();
            name.starts_with("flow_override_")
                || reheat_names.contains(&name)
                || [
                    "normal_airflow_sequence",
                    "damper_flow_pi",
                    "damper_closed",
                    "damper_feedback_output",
                    "damper_is_unoccupied",
                    "damper_operation_enabled",
                    "mode_unoccupied",
                ]
                .contains(&name)
        });
        for (name, value) in [
            ("flow_override_cooling_max", 0.09),
            ("flow_override_design_min", 0.01),
            ("flow_override_heating_max", 0.08),
        ] {
            branch["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == name)
                .unwrap()["parameters"][0]["value"] = json!(value);
        }
        let pi = branch["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "damper_flow_pi")
            .unwrap();
        for parameter in pi["parameters"].as_array_mut().unwrap() {
            if parameter["name"] == "k" {
                parameter["value"] = json!(1.0);
            }
            if parameter["name"] == "r" {
                parameter["value"] = json!(0.09);
            }
        }
        let valve_pi = branch["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "discharge_valve_pi")
            .unwrap();
        for parameter in valve_pi["parameters"].as_array_mut().unwrap() {
            if parameter["name"] == "k" {
                parameter["value"] = json!(0.5);
            }
            if parameter["name"] == "Ti" {
                parameter["value"] = json!(300.0);
            }
        }
        let names: std::collections::HashSet<String> = branch["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|block| block["name"].as_str().unwrap().to_string())
            .collect();
        let mut edges: Vec<Value> = branch["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|edge| {
                let source = match edge["from"].as_str().unwrap() {
                    "heat_pi.y" => "heating_signal",
                    "window_heat_target.y" => "heating_setpoint",
                    "local_occupied.y" => "occupied",
                    other => other,
                };
                let target = edge["to"].as_str().unwrap();
                if target.starts_with("normal_airflow_sequence.")
                    || !names.contains(target.split('.').next().unwrap())
                    || !(names.contains(source.split('.').next().unwrap())
                        || [
                            "heating_signal",
                            "heating_setpoint",
                            "supply_air_setpoint",
                            "discharge_temp",
                            "occupied",
                        ]
                        .contains(&source))
                {
                    return None;
                }
                Some(json!({"from":source,"to":target}))
            })
            .collect();
        edges.extend([
            json!({"from":"normal_airflow_sequence.y","to":"flow_override_select.u2"}),
            json!({"from":"flow_override_code","to":"flow_override_is_zero.u1"}),
            json!({"from":"flow_override_code","to":"flow_override_is_cooling.u1"}),
            json!({"from":"flow_override_code","to":"flow_override_is_minimum.u1"}),
            json!({"from":"flow_override_code","to":"flow_override_is_heating.u1"}),
            json!({"from":"measured_airflow","to":"damper_flow_pi.u_m"}),
            json!({"from":"supply_fan_proven","to":"damper_flow_pi.trigger"}),
            json!({"from":"mode","to":"damper_is_unoccupied.u1"}),
            json!({"from":"flow_override_select.y","to":"airflow_request"}),
            json!({"from":"damper_feedback_output.y","to":"damper_position"}),
            json!({"from":"discharge_target.y","to":"discharge_setpoint"}),
            json!({"from":"discharge_valve_select.y","to":"reheat_valve_position"}),
        ]);
        for input in [
            "cooling_signal",
            "heating_signal",
            "supply_temp",
            "zone_temp",
            "discharge_temp",
            "cooling_min",
            "cooling_max",
            "minimum_flow",
            "heating_min",
            "heating_max",
            "heating_warm_margin",
        ] {
            edges.push(json!({"from":input,"to":format!("normal_airflow_sequence.{input}")}));
        }
        branch["connections"] = json!(edges);
        let branch: Block = serde_json::from_value(branch).unwrap();
        assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
        let archive = include_str!("../../verification/g36_damper_valves_dymola.txt");
        let series = |name: &str| -> Vec<f64> {
            archive
                .lines()
                .find(|line| line.starts_with(name))
                .unwrap()
                .split_once('=')
                .unwrap()
                .1
                .trim_matches(['[', ']'])
                .split(',')
                .map(|field| field.trim().parse().unwrap())
                .collect()
        };
        let expected_flow = series("damValFan1.VSet_flow=");
        let expected_damper = series("damValFan1.yDam=");
        let expected_discharge = series("damValFan1.TDisSet=");
        let expected_valve = series("damValFan1.yVal=");
        assert_eq!(
            (
                expected_flow.len(),
                expected_damper.len(),
                expected_discharge.len(),
                expected_valve.len()
            ),
            (101, 101, 101, 101)
        );
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        let mut largest_flow = (0usize, 0.0_f64);
        let mut largest_damper = (0usize, 0.0_f64);
        let mut largest_discharge = (0usize, 0.0_f64);
        let mut largest_valve = (0usize, 0.0_f64);
        for second in 0..=7200 {
            let t = second as f64;
            let inputs = HashMap::from([
                (
                    "cooling_signal".into(),
                    json!((1.0 - (t - 900.0) / 3600.0).clamp(0.0, 1.0)),
                ),
                (
                    "heating_signal".into(),
                    json!(((t - 5500.0) / 3600.0).clamp(0.0, 1.0)),
                ),
                (
                    "supply_temp".into(),
                    json!(13.0 + (std::f64::consts::TAU * t / 3600.0).sin()),
                ),
                ("zone_temp".into(), json!(22.0)),
                (
                    "discharge_temp".into(),
                    json!(13.0 + 1.2 * (std::f64::consts::TAU * t / 3600.0).sin()),
                ),
                ("cooling_min".into(), json!(0.015)),
                ("cooling_max".into(), json!(0.075)),
                ("minimum_flow".into(), json!(0.01)),
                ("heating_min".into(), json!(0.015)),
                ("heating_max".into(), json!(0.06)),
                ("heating_warm_margin".into(), json!(3.0)),
                (
                    "flow_override_code".into(),
                    json!((((t - 1000.0) * 3.0 / 2000.0).clamp(0.0, 3.0).round()) as i64),
                ),
                ("measured_airflow".into(), json!(0.01 + 0.06 * t / 7200.0)),
                (
                    "supply_fan_proven".into(),
                    json!((180..4500).contains(&second) || second >= 4980),
                ),
                ("mode".into(), json!(1)),
                ("heating_setpoint".into(), json!(20.0)),
                ("supply_air_setpoint".into(), json!(13.0)),
                ("occupied".into(), json!(true)),
            ]);
            let slow = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
            for output in [
                "airflow_request",
                "damper_position",
                "discharge_setpoint",
                "reheat_valve_position",
            ] {
                assert!(
                    (slow[output].as_f64().unwrap() - fast[output].as_f64().unwrap()).abs() < 1e-9,
                    "second {second} output {output}"
                );
            }
            if second % 72 == 0 {
                let index = second / 72;
                let flow_excess =
                    (slow["airflow_request"].as_f64().unwrap() - expected_flow[index]).abs();
                let damper_excess =
                    (slow["damper_position"].as_f64().unwrap() - expected_damper[index]).abs();
                let discharge_excess = (slow["discharge_setpoint"].as_f64().unwrap()
                    - (expected_discharge[index] - 273.15))
                    .abs();
                let valve_excess =
                    (slow["reheat_valve_position"].as_f64().unwrap() - expected_valve[index]).abs();
                if flow_excess > largest_flow.1 {
                    largest_flow = (index, flow_excess);
                }
                if damper_excess > largest_damper.1 {
                    largest_damper = (index, damper_excess);
                }
                if discharge_excess > largest_discharge.1 {
                    largest_discharge = (index, discharge_excess);
                }
                if valve_excess > largest_valve.1 {
                    largest_valve = (index, valve_excess);
                }
            }
        }
        assert!(
            largest_flow.1 < 1e-5,
            "flow excess {} at sample {}",
            largest_flow.1,
            largest_flow.0
        );
        assert!(
            largest_damper.1 < 0.003,
            "damper excess {} at sample {}",
            largest_damper.1,
            largest_damper.0
        );
        assert!(
            largest_discharge.1 < 0.00002,
            "discharge target excess {} at sample {}",
            largest_discharge.1,
            largest_discharge.0
        );
        assert!(
            largest_valve.1 < 0.008,
            "reheat valve excess {} at sample {}",
            largest_valve.1,
            largest_valve.0
        );
    }
    #[test]
    fn g36_reheat_valve_core_replays_pinned_damper_valves_trace() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let names = [
            "discharge_upper",
            "discharge_max_rise",
            "discharge_span",
            "discharge_positive_span",
            "discharge_zero_rise",
            "discharge_demand_double",
            "discharge_demand_limit",
            "discharge_reset_rise",
            "discharge_reset_target",
            "discharge_target_cap",
            "maximum_discharge_temp",
            "discharge_target",
            "discharge_minimum",
            "discharge_baseline",
            "discharge_heat_detected",
            "discharge_heat_hold",
            "discharge_valve_pi",
            "discharge_below_minimum",
            "discharge_minimum_occupied",
            "discharge_valve_enable",
            "discharge_valve_closed",
            "discharge_valve_select",
        ];
        let mut branch = serde_json::to_value(zone).unwrap();
        branch["name"] = json!("PinnedDamperValvesReheatCore");
        branch["inputs"] = json!([
            {"name":"heating_signal","type":"Real","unit":"fraction (0–1)"},
            {"name":"heating_setpoint","type":"Real","unit":"°C"},
            {"name":"supply_air_setpoint","type":"Real","unit":"°C"},
            {"name":"discharge_temp","type":"Real","unit":"°C"},
            {"name":"occupied","type":"Boolean"}
        ]);
        branch["outputs"] = json!([{"name":"y","type":"Real","unit":"fraction (0–1)"}]);
        branch["blocks"]
            .as_array_mut()
            .unwrap()
            .retain(|block| names.contains(&block["name"].as_str().unwrap()));
        let pi = branch["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "discharge_valve_pi")
            .unwrap();
        for parameter in pi["parameters"].as_array_mut().unwrap() {
            if parameter["name"] == "k" {
                parameter["value"] = json!(0.5);
            }
            if parameter["name"] == "Ti" {
                parameter["value"] = json!(300.0);
            }
        }
        let original = branch["connections"].as_array().unwrap().clone();
        let mut edges: Vec<Value> = original
            .iter()
            .filter_map(|edge| {
                let target = edge["to"].as_str().unwrap().split('.').next().unwrap();
                if !names.contains(&target) {
                    return None;
                }
                let source = match edge["from"].as_str().unwrap() {
                    "heat_pi.y" => "heating_signal",
                    "window_heat_target.y" => "heating_setpoint",
                    "local_occupied.y" => "occupied",
                    other => other,
                };
                assert!(
                    names.contains(&source.split('.').next().unwrap())
                        || [
                            "heating_signal",
                            "heating_setpoint",
                            "supply_air_setpoint",
                            "discharge_temp",
                            "occupied"
                        ]
                        .contains(&source),
                    "unexpected reheat source {source}"
                );
                Some(json!({"from":source,"to":edge["to"]}))
            })
            .collect();
        edges.push(json!({"from":"discharge_valve_select.y","to":"y"}));
        branch["connections"] = json!(edges);
        let branch: Block = serde_json::from_value(branch).unwrap();
        assert!(validate(&branch).is_empty());
        let archive = include_str!("../../verification/g36_damper_valves_dymola.txt");
        let expected: Vec<f64> = archive
            .lines()
            .find(|line| line.starts_with("damValFan1.yVal="))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|field| field.trim().parse().unwrap())
            .collect();
        assert_eq!(expected.len(), 101);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        let mut largest = (0usize, 0.0_f64);
        for second in 0..=7200 {
            let t = second as f64;
            let inputs = HashMap::from([
                (
                    "heating_signal".into(),
                    json!(((t - 5500.0) / 3600.0).clamp(0.0, 1.0)),
                ),
                ("heating_setpoint".into(), json!(20.0)),
                ("supply_air_setpoint".into(), json!(13.0)),
                (
                    "discharge_temp".into(),
                    json!(13.0 + 1.2 * (std::f64::consts::TAU * t / 3600.0).sin()),
                ),
                ("occupied".into(), json!(true)),
            ]);
            let slow = dynamic.execute(&branch, &inputs, 1.0, "").unwrap()["y"]
                .as_f64()
                .unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0["y"]
                .as_f64()
                .unwrap();
            assert!(
                (slow - fast).abs() < 1e-9,
                "second {second}: {slow} vs {fast}"
            );
            if second % 72 == 0 {
                let index = second / 72;
                let excess = (slow - expected[index]).abs();
                if excess > largest.1 {
                    largest = (index, excess);
                }
            }
        }
        assert!(
            largest.1 < 0.03,
            "largest valve trace excess {} at sample {}",
            largest.1,
            largest.0
        );
    }
    #[test]
    fn g36_cdl_flow_override_changes_physical_zone_airflow() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "zone_01_flow_override_none")
            .unwrap()["parameters"][0]["value"] = json!(1);
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 20.0,
                solar_kw: 0.0,
                internal_kw: 16.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        let mut forced = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        for sim in [&mut normal, &mut forced] {
            start_present_window(sim);
            sim.temps = [27.0; 10];
        }
        let baseline = run_weather_intervals(&mut normal, 2);
        let override_rows = run_weather_intervals(&mut forced, 2);
        let normal_row = &baseline["rows"][1];
        let forced_row = &override_rows["rows"][1];
        assert!(
            normal_row["requested_airflow"].as_f64().unwrap()
                > forced_row["requested_airflow"].as_f64().unwrap()
        );
        assert!(
            normal_row["zone_airflow"][0].as_f64().unwrap()
                > forced_row["zone_airflow"][0].as_f64().unwrap()
        );
        assert!(
            forced_row["zone_airflow"][0].as_f64().unwrap() < 0.005,
            "the OBC-style PI should nearly close the damper after a zero-flow override"
        );
    }
    #[test]
    fn conventional_source_fault_status_comes_from_cdl() {
        let building: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        let inputs: HashMap<String, Value> = building
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
        let mut dynamic = Executor::default();
        let normal = dynamic.execute(&building, &inputs, 900.0, "").unwrap();
        assert_eq!(normal["plant_heat_pump_start_fault"], json!(false));
        let mut edited = building.clone();
        let plant = edited
            .blocks
            .iter_mut()
            .find(|block| block.name == "plant")
            .unwrap();
        plant
            .blocks
            .iter_mut()
            .find(|block| block.name == "conventional_heat_pump_start_fault_off")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(true);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&edited).unwrap().unwrap();
        let slow = dynamic.execute(&edited, &inputs, 900.0, "").unwrap();
        let fast = compiled.execute(&inputs, 900.0, false).unwrap().0;
        assert_eq!(slow["plant_heat_pump_start_fault"], json!(true));
        assert_eq!(fast["plant_heat_pump_start_fault"], json!(true));
    }
    #[test]
    fn conventional_status_follows_edited_cdl() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        assert!(!is_g36_graph(&graph));
        assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
        assert!(conventional_control_contract(&graph).is_empty());
        assert!(
            undriven_building_outputs(&graph).is_empty(),
            "{:?}",
            undriven_building_outputs(&graph)
        );
        let inputs: HashMap<String, Value> = graph
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else {
                        json!(22.0)
                    },
                )
            })
            .collect();
        let mut executor = Executor::default();
        let normal = executor.execute(&graph, &inputs, 900.0, "").unwrap();
        assert_eq!(normal["preconditioning_mode"], false);
        assert_eq!(normal["zone_01_occupancy_confirmed"], false);

        let mut edited = graph;
        edited
            .blocks
            .iter_mut()
            .find(|block| block.name == "conventional_preconditioning_off")
            .unwrap()
            .parameters[0]
            .value = json!(true);
        let mut executor = Executor::default();
        let changed = executor.execute(&edited, &inputs, 900.0, "").unwrap();
        assert_eq!(changed["preconditioning_mode"], true);
        assert_eq!(changed["zone_01_occupancy_confirmed"], false);
        edited
            .blocks
            .iter_mut()
            .find(|block| block.name == "conventional_preconditioning_off")
            .unwrap()
            .parameters[0]
            .value = json!(false);
        edited
            .blocks
            .iter_mut()
            .find(|block| block.name == "conventional_occupancy_false")
            .unwrap()
            .parameters[0]
            .value = json!(true);
        let mut executor = Executor::default();
        let changed = executor.execute(&edited, &inputs, 900.0, "").unwrap();
        let mut compiled = CompiledExecutor::try_new(&edited).unwrap().unwrap();
        let faster = compiled.execute(&inputs, 900.0, false).unwrap().0;
        assert_eq!(changed["preconditioning_mode"], false);
        assert_eq!(changed["zone_01_occupancy_confirmed"], true);
        for name in ["preconditioning_mode", "zone_01_occupancy_confirmed"] {
            assert_eq!(changed[name], faster[name]);
        }
    }
    #[test]
    fn building_rejects_undriven_alarm_and_request_outputs() {
        let mut graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        assert!(undriven_building_outputs(&graph).is_empty());
        let alarm = "zone_01_low_airflow_alarm_level";
        let edge = graph
            .connections
            .iter_mut()
            .find(|edge| edge.target().0 == alarm)
            .unwrap();
        edge.from_path = "zone_01_room_temp".into();
        edge.from_block.clear();
        edge.from_output.clear();
        assert!(undriven_building_outputs(&graph)
            .iter()
            .any(|error| error.contains(alarm)));

        let mut conventional: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/four_pipe_building.json"
        ))
        .unwrap();
        assert!(undriven_building_outputs(&conventional).is_empty());
        conventional
            .connections
            .retain(|edge| edge.target().0 != "zone_01_cooling_demand");
        assert!(undriven_building_outputs(&conventional)
            .iter()
            .any(|error| error.contains("zone_01_cooling_demand")));
    }
    #[test]
    fn renaming_a_g36_program_does_not_disable_its_control_contract() {
        let mut graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        graph.name = "CustomBuildingSequence".into();
        assert!(is_g36_graph(&graph));
        assert!(g36_control_contract(&graph).is_empty());
        graph
            .connections
            .retain(|edge| edge.target().0 != "plant_heat_pump_requested_mode");
        assert!(g36_control_contract(&graph)
            .iter()
            .any(|error| error.contains("plant_heat_pump_requested_mode")));
        graph
            .blocks
            .iter_mut()
            .find(|block| block.name == "plant")
            .unwrap()
            .name = "custom_plant".into();
        graph
            .blocks
            .iter_mut()
            .find(|block| block.name == "zone_group_1")
            .unwrap()
            .name = "custom_group".into();
        assert!(is_g36_graph(&graph));
    }
    #[test]
    fn removing_g36_signature_outputs_does_not_enable_raw_occupancy_fallback() {
        let mut graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        graph.name = "EditedBuildingControls".into();
        for name in [
            "plant_heat_pump_requested_mode",
            "zone_01_occupancy_confirmed",
            "preconditioning_mode",
        ] {
            graph.outputs.retain(|output| output.name != name);
            graph.connections.retain(|edge| edge.target().0 != name);
        }
        assert!(is_g36_graph(&graph));
        let errors = g36_control_contract(&graph);
        for name in [
            "plant_heat_pump_requested_mode",
            "zone_01_occupancy_confirmed",
            "preconditioning_mode",
        ] {
            assert!(errors.iter().any(|error| error.contains(name)), "{name}");
        }
    }
    #[test]
    fn g36_rejects_raw_sensor_passthrough_as_an_actuator_command() {
        let mut graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let edge = graph
            .connections
            .iter_mut()
            .find(|edge| edge.target().0 == "zone_01_airflow_request")
            .unwrap();
        edge.from_path = "zone_01_room_temp".into();
        edge.from_block.clear();
        edge.from_output.clear();
        assert!(g36_control_contract(&graph)
            .iter()
            .any(|error| error.contains("zone_01_airflow_request")));
    }
    #[test]
    fn g36_rejects_raw_passthrough_inside_a_composite_output() {
        let mut graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        assert!(g36_control_contract(&graph).is_empty());
        let zone = graph
            .blocks
            .iter_mut()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let edge = zone
            .connections
            .iter_mut()
            .find(|edge| edge.target().0 == "damper_position")
            .unwrap();
        edge.from_path = "room_temp".into();
        assert!(g36_control_contract(&graph)
            .iter()
            .any(|error| error.contains("zone_01_damper_position")));
        let zone = graph
            .blocks
            .iter_mut()
            .find(|block| block.name == "zone_01")
            .unwrap();
        zone.blocks.clear();
        zone.connections.clear();
        zone.category = None;
        assert!(g36_control_contract(&graph)
            .iter()
            .any(|error| error.contains("zone_01_damper_position")));
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
        let mut renamed_graph: Value = serde_json::from_str(graph).unwrap();
        renamed_graph["name"] = json!("CustomBuildingSequence");
        let mut renamed = Simulation::new(&renamed_graph.to_string(), &weather, "{}").unwrap();
        let mode_trace = "FourPipeG36Controls|output.plant_heat_pump_requested_mode";
        for sim in [&mut recovered, &mut independent] {
            sim.set_traces(&json!([mode_trace]).to_string()).unwrap();
            start_present_window(sim);
            sim.temps = [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0];
            sim.last_heat_request = 100.0;
            sim.last_cool_request = 100.0;
        }
        start_present_window(&mut renamed);
        renamed.temps = [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0];
        renamed.last_heat_request = 100.0;
        renamed.last_cool_request = 100.0;
        let recovered_rows = run_weather_intervals(&mut recovered, 3);
        let independent_rows = run_weather_intervals(&mut independent, 3);
        let renamed_rows = run_weather_intervals(&mut renamed, 3);
        for i in 0..3 {
            let recovery = &recovered_rows["rows"][i];
            let separate = &independent_rows["rows"][i];
            let renamed = &renamed_rows["rows"][i];
            let heat = recovery["heat_pump_heating_used_kw"].as_f64().unwrap();
            let cool = recovery["heat_pump_cooling_used_kw"].as_f64().unwrap();
            let work = recovery["heat_pump_electric_kw"].as_f64().unwrap();
            let ambient = recovery["heat_pump_ambient_exchange_kw"].as_f64().unwrap();
            assert!(heat > 0.0 && cool > 0.0);
            assert_eq!(recovery["traces"][mode_trace], json!(3.0));
            assert_eq!(
                renamed["heat_pump_electric_kw"],
                recovery["heat_pump_electric_kw"]
            );
            assert_eq!(renamed["electric_kw"], recovery["electric_kw"]);
            assert_eq!(separate["traces"][mode_trace], json!(1.0));
            assert!((heat - cool - work - ambient).abs() < 1e-9);
            assert!(separate["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
            assert_eq!(separate["heat_pump_cooling_used_kw"], json!(0.0));
            let separate_heat = separate["heat_pump_heating_used_kw"].as_f64().unwrap();
            let separate_work = separate["heat_pump_electric_kw"].as_f64().unwrap();
            let separate_ambient = separate["heat_pump_ambient_exchange_kw"].as_f64().unwrap();
            assert!((separate_heat - separate_work - separate_ambient).abs() < 1e-9);
        }
    }
    #[test]
    fn conventional_cdl_mode_changes_physical_compressor_delivery() {
        let graph = include_str!("../../web/public/examples/four_pipe_building.json");
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
        let trace = "FourPipeBuildingControls|output.plant_heat_pump_requested_mode";
        for sim in [&mut recovered, &mut independent] {
            sim.set_traces(&json!([trace]).to_string()).unwrap();
            start_present_window(sim);
            sim.temps = [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0];
            sim.last_heat_request = 100.0;
            sim.last_cool_request = 100.0;
        }
        let recovery = run_weather_intervals(&mut recovered, 1);
        let separate = run_weather_intervals(&mut independent, 1);
        let shared = &recovery["rows"][0];
        let independent = &separate["rows"][0];
        assert_eq!(shared["traces"][trace], json!(3.0));
        assert_eq!(independent["traces"][trace], json!(1.0));
        assert!(
            independent["heat_pump_heating_used_kw"].as_f64().unwrap()
                >= shared["heat_pump_heating_used_kw"].as_f64().unwrap()
        );
        assert!(shared["heat_pump_cooling_used_kw"].as_f64().unwrap() > 0.0);
        assert_eq!(independent["heat_pump_cooling_used_kw"], json!(0.0));
        assert!(independent["heat_pump_electric_kw"].as_f64().unwrap() > 0.0);
        for (mode, expected_heat, expected_cool) in [(0, false, false), (2, false, true)] {
            let mut edited: Value = serde_json::from_str(graph).unwrap();
            let plant = edited["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "plant")
                .unwrap();
            plant["connections"]
                .as_array_mut()
                .unwrap()
                .retain(|edge| edge["to"] != "heat_pump_requested_mode");
            plant["connections"].as_array_mut().unwrap().push(
                json!({"from":"heat_pump_mode_cooling_weight.y","to":"heat_pump_requested_mode"}),
            );
            plant["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "heat_pump_mode_cooling_weight")
                .unwrap()["parameters"][0]["value"] = json!(mode);
            let mut sim = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
            start_present_window(&mut sim);
            sim.temps = [16.0, 16.0, 16.0, 16.0, 16.0, 28.0, 28.0, 28.0, 28.0, 28.0];
            sim.last_heat_request = 100.0;
            sim.last_cool_request = 100.0;
            let mut dynamic = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
            dynamic.compiled = None;
            start_present_window(&mut dynamic);
            dynamic.temps = sim.temps;
            dynamic.last_heat_request = sim.last_heat_request;
            dynamic.last_cool_request = sim.last_cool_request;
            let row = run_weather_intervals(&mut sim, 1)["rows"][0].clone();
            let dynamic_row = run_weather_intervals(&mut dynamic, 1)["rows"][0].clone();
            assert_eq!(
                row["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0,
                expected_heat
            );
            assert_eq!(
                row["heat_pump_cooling_used_kw"].as_f64().unwrap() > 0.0,
                expected_cool
            );
            assert_eq!(sim.source_running[0], expected_heat || expected_cool);
            assert_eq!(
                row["heat_pump_heating_used_kw"],
                dynamic_row["heat_pump_heating_used_kw"]
            );
            assert_eq!(
                row["heat_pump_cooling_used_kw"],
                dynamic_row["heat_pump_cooling_used_kw"]
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
        let base_row = &normal["rows"][5];
        let high_row = &higher["rows"][5];
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
                "pressure_request_sample",
                "sat_previous",
                "sat_request_sample",
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
            if block["block_type"] == "VAVBoxController" {
                for name in [
                    "sat_previous_cooling_setpoint",
                    "sat_sampled_cooling_setpoint",
                    "sat_clear_previous",
                    "pressure_damper_sample",
                    "pressure_setpoint_sample",
                    "pressure_measured_sample",
                ] {
                    block["blocks"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|child| child["name"] == name)
                        .unwrap()["parameters"][0]["value"] = json!(300.0);
                }
            }
            if block["name"].as_str().is_some_and(|name| {
                name.ends_with("_target_previous")
                    || name.ends_with("_unoccupied_heat_previous")
                    || name.ends_with("_unoccupied_cool_previous")
            }) {
                block["parameters"][0]["value"] = json!(300.0);
            }
            if block["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("zone_group_"))
            {
                for child in block["blocks"].as_array_mut().unwrap() {
                    if child["name"].as_str().is_some_and(|name| {
                        name.starts_with("optimal_") && name.ends_with("_rate_previous")
                    }) {
                        child["parameters"][0]["value"] = json!(300.0);
                    }
                }
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
    fn simulation_step_resolves_sample_trigger_period_and_phase() {
        let mut graph: Block = serde_json::from_value(json!({
            "name": "clock", "block_type": "Buildings.Controls.OBC.CDL.Logical.Sources.SampleTrigger",
            "outputs": [{"name": "y", "type": "Boolean"}],
            "parameters": [{"name": "period", "value": 60.0}, {"name": "shift", "value": 15.0}]
        })).unwrap();
        assert_eq!(sampled_step(&graph, 900).unwrap(), 15);
        graph.parameters[1].value = json!(30.0);
        assert_eq!(sampled_step(&graph, 900).unwrap(), 30);
        graph.parameters[0].value = json!(45.0);
        assert_eq!(sampled_step(&graph, 900).unwrap(), 15);
        graph.parameters[1].value = json!(5.0);
        assert!(sampled_step(&graph, 900)
            .unwrap_err()
            .contains("SampleTrigger shift"));
        graph.parameters[1].value = json!(0.0);
        graph.parameters[0].value = json!(10.0);
        assert!(sampled_step(&graph, 900)
            .unwrap_err()
            .contains("SampleTrigger period"));

        let mut building: Value = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
        .unwrap();
        for group in building["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|block| {
                block["name"]
                    .as_str()
                    .is_some_and(|name| name.starts_with("zone_group_"))
            })
        {
            let trigger = group["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "optimal_daily_sample")
                .unwrap();
            trigger["parameters"][0]["value"] = json!(45.0);
            trigger["parameters"][1]["value"] = json!(15.0);
        }
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
        let sim = Simulation::new(&building.to_string(), &weather, "{}").unwrap();
        assert_eq!(sim.effective_step_seconds(), 15);
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
    fn obc_integer_conversions_and_comparisons_match_both_executors() {
        let graph: Block = serde_json::from_value(json!({
            "name":"integer_reference","block_type":"Composite","category":"composite",
            "inputs":[{"name":"condition","type":"Boolean"},{"name":"real","type":"Real"},{"name":"match_value","type":"Integer"}],
            "outputs":[{"name":"converted","type":"Integer"},{"name":"rounded","type":"Integer"},{"name":"above","type":"Boolean"},{"name":"equal","type":"Boolean"}],
            "blocks":[
                {"name":"boolean_to_integer","block_type":"Buildings.Controls.OBC.CDL.Conversions.BooleanToInteger","inputs":[{"name":"u","type":"Boolean"}],"outputs":[{"name":"y","type":"Integer"}],"parameters":[{"name":"integerTrue","value":7},{"name":"integerFalse","value":-3}]},
                {"name":"real_to_integer","block_type":"Buildings.Controls.OBC.CDL.Conversions.RealToInteger","inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Integer"}]},
                {"name":"greater_threshold","block_type":"Buildings.Controls.OBC.CDL.Integers.GreaterThreshold","inputs":[{"name":"u","type":"Integer"}],"outputs":[{"name":"y","type":"Boolean"}],"parameters":[{"name":"t","value":2}]},
                {"name":"integer_equal","block_type":"Buildings.Controls.OBC.CDL.Integers.Equal","inputs":[{"name":"u1","type":"Integer"},{"name":"u2","type":"Integer"}],"outputs":[{"name":"y","type":"Boolean"}]}
            ],
            "connections":[
                {"from":"condition","to":"boolean_to_integer.u"},
                {"from":"real","to":"real_to_integer.u"},
                {"from":"real_to_integer.y","to":"greater_threshold.u"},
                {"from":"real_to_integer.y","to":"integer_equal.u1"},
                {"from":"match_value","to":"integer_equal.u2"},
                {"from":"boolean_to_integer.y","to":"converted"},
                {"from":"real_to_integer.y","to":"rounded"},
                {"from":"greater_threshold.y","to":"above"},
                {"from":"integer_equal.y","to":"equal"}
            ]
        })).unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (condition, real, match_value, expected_integer, above, equal) in [
            (false, -2.5, -3, -3, false, true),
            (true, -0.5, 0, -1, false, false),
            (false, 2.5, 3, 3, true, true),
            (true, 1.5, 2, 2, false, true),
        ] {
            let inputs = HashMap::from([
                ("condition".into(), json!(condition)),
                ("real".into(), json!(real)),
                ("match_value".into(), json!(match_value)),
            ]);
            let reference = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
            let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0;
            for actual in [&reference, &optimized] {
                assert_eq!(actual["converted"], json!(if condition { 7 } else { -3 }));
                assert_eq!(actual["rounded"], json!(expected_integer));
                assert_eq!(actual["above"], json!(above));
                assert_eq!(actual["equal"], json!(equal));
            }
        }
    }
    #[test]
    fn boolean_pre_converges_feedback_within_event_in_both_executors() {
        let graph: Block = serde_json::from_value(json!({
            "name":"previous_boolean", "block_type":"composite",
            "inputs":[{"name":"request","type":"Boolean"}],
            "outputs":[{"name":"latched","type":"Boolean"},{"name":"previous","type":"Boolean"}],
            "blocks":[
                {"name":"pre","block_type":"Buildings.Controls.OBC.CDL.Logical.Pre",
                 "inputs":[{"name":"u","type":"Boolean"}],
                 "outputs":[{"name":"y","type":"Boolean"}],
                 "parameters":[{"name":"pre_u_start","type":"Boolean","value":false}]},
                {"name":"feedback","block_type":"Buildings.Controls.OBC.CDL.Logical.Or",
                 "inputs":[{"name":"u1","type":"Boolean"},{"name":"u2","type":"Boolean"}],
                 "outputs":[{"name":"y","type":"Boolean"}]}
            ],
            "connections":[
                {"from":"request","to":"feedback.u1"},
                {"from":"pre.y","to":"feedback.u2"},
                {"from":"feedback.y","to":"pre.u"},
                {"from":"feedback.y","to":"latched"},
                {"from":"pre.y","to":"previous"}
            ]
        }))
        .unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (request, expected_latched, expected_previous) in [
            (false, false, false),
            (true, true, true),
            (false, true, true),
            (false, true, true),
        ] {
            let inputs = HashMap::from([("request".into(), json!(request))]);
            let ordinary = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            for result in [&ordinary, &fast] {
                assert_eq!(result["latched"], json!(expected_latched));
                assert_eq!(result["previous"], json!(expected_previous));
            }
        }
    }

    #[test]
    fn boolean_pre_rejects_oscillating_event_loop() {
        let graph: Block = serde_json::from_value(json!({
            "name":"oscillating_boolean", "block_type":"composite",
            "inputs":[],
            "outputs":[{"name":"y","type":"Boolean"}],
            "blocks":[
                {"name":"pre","block_type":"Buildings.Controls.OBC.CDL.Logical.Pre",
                 "inputs":[{"name":"u","type":"Boolean"}],
                 "outputs":[{"name":"y","type":"Boolean"}]},
                {"name":"invert","block_type":"Buildings.Controls.OBC.CDL.Logical.Not",
                 "inputs":[{"name":"u","type":"Boolean"}],
                 "outputs":[{"name":"y","type":"Boolean"}]}
            ],
            "connections":[
                {"from":"pre.y","to":"invert.u"},
                {"from":"invert.y","to":"pre.u"},
                {"from":"invert.y","to":"y"}
            ]
        }))
        .unwrap();
        assert!(validate(&graph).is_empty());
        assert!(Executor::default()
            .execute(&graph, &HashMap::new(), 60.0, "")
            .unwrap_err()
            .contains("did not converge"));
        assert!(CompiledExecutor::try_new(&graph)
            .unwrap()
            .unwrap()
            .execute(&HashMap::new(), 60.0, false)
            .unwrap_err()
            .contains("did not converge"));
    }

    #[test]
    fn real_greater_threshold_uses_obc_hysteresis_in_both_executors() {
        let graph: Block = serde_json::from_value(json!({
            "name":"hysteretic_threshold",
            "block_type":"Buildings.Controls.OBC.CDL.Reals.GreaterThreshold",
            "inputs":[{"name":"u","type":"Real"}],
            "outputs":[{"name":"y","type":"Boolean"}],
            "parameters":[{"name":"t","value":10.0},{"name":"h","value":2.0}]
        }))
        .unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (value, expected) in [
            (9.0, false),
            (11.0, true),
            (9.0, true),
            (8.0, false),
            (11.0, true),
            (8.0, false),
        ] {
            let inputs = HashMap::from([("u".into(), json!(value))]);
            assert_eq!(
                dynamic.execute(&graph, &inputs, 60.0, "").unwrap()["y"],
                json!(expected)
            );
            assert_eq!(
                compiled.execute(&inputs, 60.0, false).unwrap().0["y"],
                json!(expected)
            );
        }
        let mut invalid = graph.clone();
        invalid
            .parameters
            .iter_mut()
            .find(|p| p.name == "h")
            .unwrap()
            .value = json!(-1.0);
        assert!(validate(&invalid)
            .iter()
            .any(|issue| issue.contains("nonnegative")));
    }

    #[test]
    fn real_less_threshold_uses_obc_hysteresis_in_both_executors() {
        let graph: Block = serde_json::from_value(json!({
            "name":"low_discharge_threshold",
            "block_type":"Buildings.Controls.OBC.CDL.Reals.LessThreshold",
            "inputs":[{"name":"u","type":"Real"}],
            "outputs":[{"name":"y","type":"Boolean"}],
            "parameters":[{"name":"t","value":10.0},{"name":"h","value":0.25}]
        }))
        .unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (value, expected) in [
            (10.1, false),
            (9.9, true),
            (10.1, true),
            (10.24, true),
            (10.25, false),
            (9.9, true),
        ] {
            let inputs = HashMap::from([("u".into(), json!(value))]);
            assert_eq!(
                dynamic.execute(&graph, &inputs, 60.0, "").unwrap()["y"],
                json!(expected)
            );
            assert_eq!(
                compiled.execute(&inputs, 60.0, false).unwrap().0["y"],
                json!(expected)
            );
        }
        let mut invalid = graph.clone();
        invalid
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "h")
            .unwrap()
            .value = json!(-0.1);
        assert!(validate(&invalid)
            .iter()
            .any(|error| error.contains("hysteresis")));
    }

    #[test]
    fn real_pair_comparators_use_obc_hysteresis_in_both_executors() {
        let graph: Block = serde_json::from_value(json!({
            "name":"pair_comparators", "block_type":"Composite", "category":"composite",
            "inputs":[{"name":"a","type":"Real"},{"name":"b","type":"Real"}],
            "outputs":[{"name":"greater","type":"Boolean"},{"name":"less","type":"Boolean"}],
            "blocks":[
                {"name":"g","block_type":"Buildings.Controls.OBC.CDL.Reals.Greater",
                 "inputs":[{"name":"u1","type":"Real"},{"name":"u2","type":"Real"}],
                 "outputs":[{"name":"y","type":"Boolean"}],
                 "parameters":[{"name":"h","value":0.2},{"name":"pre_y_start","type":"Boolean","value":false}]},
                {"name":"l","block_type":"Buildings.Controls.OBC.CDL.Reals.Less",
                 "inputs":[{"name":"u1","type":"Real"},{"name":"u2","type":"Real"}],
                 "outputs":[{"name":"y","type":"Boolean"}],
                 "parameters":[{"name":"h","value":0.2},{"name":"pre_y_start","type":"Boolean","value":false}]}
            ],
            "connections":[
                {"from":"a","to":"g.u1"},{"from":"b","to":"g.u2"},
                {"from":"a","to":"l.u1"},{"from":"b","to":"l.u2"},
                {"from":"g.y","to":"greater"},{"from":"l.y","to":"less"}
            ]
        })).unwrap();
        assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (a, greater, less) in [
            (1.0, false, false),
            (1.1, true, false),
            (0.9, true, true),
            (0.75, false, true),
            (1.15, true, true),
            (1.25, true, false),
        ] {
            let inputs = HashMap::from([("a".into(), json!(a)), ("b".into(), json!(1.0))]);
            for result in [
                dynamic.execute(&graph, &inputs, 60.0, "").unwrap(),
                compiled.execute(&inputs, 60.0, false).unwrap().0,
            ] {
                assert_eq!(result["greater"], json!(greater), "a={a}");
                assert_eq!(result["less"], json!(less), "a={a}");
            }
        }
        let mut initially_true = graph.clone();
        for block in &mut initially_true.blocks {
            block
                .parameters
                .iter_mut()
                .find(|p| p.name == "pre_y_start")
                .unwrap()
                .value = json!(true);
        }
        let inputs = HashMap::from([("a".into(), json!(1.1)), ("b".into(), json!(1.0))]);
        for result in [
            Executor::default()
                .execute(&initially_true, &inputs, 60.0, "")
                .unwrap(),
            CompiledExecutor::try_new(&initially_true)
                .unwrap()
                .unwrap()
                .execute(&inputs, 60.0, false)
                .unwrap()
                .0,
        ] {
            assert_eq!(result["greater"], json!(true));
            assert_eq!(result["less"], json!(true));
        }
        for block in &mut initially_true.blocks {
            block
                .parameters
                .iter_mut()
                .find(|p| p.name == "h")
                .unwrap()
                .value = json!(-0.1);
        }
        assert!(validate(&initially_true)
            .iter()
            .any(|issue| issue.contains("nonnegative")));
    }

    #[test]
    fn pinned_optimal_start_calculation_executes_in_both_engines() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/obc_optimal_start_calculation.json"
        ))
        .unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for (error, start_calculation, next_occupancy) in [
            (2.0, false, 20_000.0),
            (2.0, true, 10_800.0),
            (2.0, false, 6_000.0),
            (0.0, false, 1_000.0),
        ] {
            let inputs = HashMap::from([
                ("TDif".into(), json!(error)),
                ("staCal".into(), json!(start_calculation)),
                ("tNexOcc".into(), json!(next_occupancy)),
            ]);
            let ordinary = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(ordinary["optOn"], fast["optOn"]);
            let a = ordinary["tOpt"].as_f64().unwrap();
            let b = fast["tOpt"].as_f64().unwrap();
            assert!(a.is_finite() && (0.0..=10_800.0).contains(&a));
            assert!((a - b).abs() < 1e-9);
        }
    }

    #[test]
    fn pinned_optimal_start_learns_a_completed_recovery_across_days() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/obc_optimal_start_calculation.json"
        ))
        .unwrap();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for minute in 0..=1440 {
            let seconds = minute * 60;
            let day_second = seconds % 86_400;
            // Three kelvin of error clears after 1,740 seconds of measured
            // recovery. OBC samples its timer at the falling edge of the
            // recovery latch, then samples that slope at the next midnight.
            let error = if day_second == 0 {
                3.0
            } else if day_second <= 1_860 {
                3.0 - 3.2 * (day_second - 60) as f64 / 1_800.0
            } else {
                -0.2
            };
            let next_occupancy = if day_second < 10_800 {
                10_800 - day_second
            } else {
                97_200 - day_second
            };
            let inputs = HashMap::from([
                ("TDif".into(), json!(error)),
                ("staCal".into(), json!(day_second == 0)),
                ("tNexOcc".into(), json!(next_occupancy)),
            ]);
            let ordinary = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(ordinary["optOn"], fast["optOn"], "minute {minute}");
            assert_eq!(ordinary["tOpt"], fast["tOpt"], "minute {minute}");
            if minute == 0 {
                assert_eq!(ordinary["tOpt"], json!(10_800.0));
            }
            if minute == 1440 {
                // First midnight samples an initial zero slope. The next
                // samples 3 K / 1740 s, so the two-sample mean is 3/3480 K/s.
                // A fresh 3 K error therefore predicts 3 / (3/3480) s.
                assert_eq!(ordinary["tOpt"], json!(3_480.0));
            }
        }
    }

    #[test]
    #[ignore = "Manual cost check for nested OBC optimal-start event iteration"]
    fn benchmark_obc_optimal_start_group() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
        let mut inputs = HashMap::from([
            ("seconds_until_occupied".into(), json!(12_000.0)),
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
        let start = std::time::Instant::now();
        for index in 0..1_000 {
            inputs.insert(
                "seconds_until_occupied".into(),
                json!(12_000.0 - index as f64 * 60.0),
            );
            compiled.execute(&inputs, 60.0, false).unwrap();
        }
        println!("1000 group steps: {:?}", start.elapsed());
    }

    #[test]
    fn pinned_optimal_start_drives_g36_group_from_editable_cdl() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("seconds_until_occupied".into(), json!(24_000.0)),
            ("lookahead_allowed".into(), json!(true)),
            ("scheduled_occupied".into(), json!(false)),
        ]);
        for zone in 1..=5 {
            inputs.insert(
                format!("room_{zone}"),
                json!(if zone == 1 { 19.0 } else { 21.0 }),
            );
            inputs.insert(format!("heat_target_{zone}"), json!(21.0));
            inputs.insert(format!("cool_target_{zone}"), json!(24.0));
            inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
            inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
            inputs.insert(format!("window_closed_{zone}"), json!(true));
            inputs.insert(format!("override_{zone}"), json!(false));
        }
        let mut dynamic = Executor::default();
        dynamic.trace_targets = vec![
            "zone_group_1|optimal_warmup_1_obc.tOpt".into(),
            "zone_group_1|optimal_start_sample_event.y".into(),
        ];
        let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
        let mut warmup_seen = false;
        for minute in 0..=340 {
            inputs.insert(
                "seconds_until_occupied".into(),
                json!(24_000.0 - minute as f64 * 60.0),
            );
            let ordinary = dynamic.execute(group, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(ordinary["warmup"], fast["warmup"]);
            assert_eq!(ordinary["preconditioning"], fast["preconditioning"]);
            if minute < 220 {
                assert_eq!(ordinary["warmup"], false);
            }
            if minute == 220 {
                assert_eq!(
                    dynamic.trace_values["zone_group_1|optimal_start_sample_event.y"],
                    true
                );
                assert_eq!(
                    dynamic.trace_values["zone_group_1|optimal_warmup_1_obc.tOpt"],
                    7200.0
                );
            }
            warmup_seen |= ordinary["warmup"] == true;
        }
        assert!(warmup_seen, "the sampled OBC prediction must start warm-up");
        inputs.insert("window_closed_1".into(), json!(false));
        let closed = dynamic.execute(group, &inputs, 60.0, "").unwrap();
        assert_eq!(closed["warmup"], false, "open-window zone must be excluded");
        assert_eq!(
            closed["warmup"],
            compiled.execute(&inputs, 60.0, false).unwrap().0["warmup"]
        );
        let mut tuned = group.clone();
        let learner = tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == "optimal_warmup_1_obc")
            .unwrap();
        learner
            .blocks
            .iter_mut()
            .find(|block| block.name == "defTemSlo")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(1.0 / 600.0);
        inputs.insert("window_closed_1".into(), json!(true));
        let mut faster = CompiledExecutor::try_new(&tuned).unwrap().unwrap();
        let mut result = HashMap::new();
        for minute in 0..=340 {
            inputs.insert(
                "seconds_until_occupied".into(),
                json!(24_000.0 - minute as f64 * 60.0),
            );
            result = faster.execute(&inputs, 60.0, false).unwrap().0;
        }
        assert_eq!(
            result["warmup"], false,
            "editing the OBC slope floor must delay warm-up"
        );
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
            assert_eq!(optimized["y"], json!(expected));
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
            json!(true)
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
        // Calls occur at t=0, 60, 120, 180, and 240 seconds.
        for expected in [2.0, 2.0, 3.0, 3.0, 4.0] {
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
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
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
    fn g36_supply_fan_alarm_follows_cdl_command_and_raw_motor_status() {
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
            ("actual_supply_air_temp".into(), json!(16.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(false)),
            ("actual_fan_speed".into(), json!(0.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        for step in 0..4 {
            let result = dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
            let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
            assert!(result["fan_speed"].as_f64().unwrap() > 0.01);
            assert_eq!(
                result["supply_fan_alarm_level"],
                json!(if step == 0 { 0 } else { 2 })
            );
            assert_eq!(
                result["supply_fan_alarm_level"].as_f64().unwrap(),
                fast["supply_fan_alarm_level"].as_f64().unwrap()
            );
        }
        inputs.insert("supply_fan_proven".into(), json!(true));
        let proven = dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
        assert_eq!(proven["supply_fan_alarm_level"], json!(0));
        inputs.insert("flow_request".into(), json!(0.0));
        inputs.insert("occupied_mode".into(), json!(false));
        for step in 0..6 {
            let result = dynamic.execute(ahu, &inputs, 15.0, "").unwrap();
            let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
            assert_eq!(result["fan_speed"], json!(0.0));
            assert_eq!(
                result["supply_fan_alarm_level"],
                json!(if step < 4 { 0 } else { 4 })
            );
            assert_eq!(
                result["supply_fan_alarm_level"].as_f64().unwrap(),
                fast["supply_fan_alarm_level"].as_f64().unwrap()
            );
        }
    }
    #[test]
    fn g36_fan_speed_tracking_alarm_matches_dynamic_and_compiled_cdl() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(28.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(29.0)),
            ("mixed_air_temp".into(), json!(28.0)),
            ("actual_supply_air_temp".into(), json!(20.0)),
            ("duct_pressure".into(), json!(0.2)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(0.3)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        for step in 0..8 {
            let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert!(result["fan_speed"].as_f64().unwrap() > 0.45);
            assert_eq!(
                result["fan_speed_tracking_alarm_level"],
                json!(if step < 5 { 0 } else { 3 })
            );
            assert_eq!(
                result["fan_speed_tracking_alarm_level"].as_f64().unwrap(),
                fast["fan_speed_tracking_alarm_level"].as_f64().unwrap()
            );
        }
        inputs.insert("actual_fan_speed".into(), json!(1.0));
        let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(result["fan_speed_tracking_alarm_level"], json!(0));
        assert_eq!(fast["fan_speed_tracking_alarm_level"].as_f64(), Some(0.0));
    }
    #[test]
    fn g36_ahu_damper_tracking_alarms_match_dynamic_and_compiled_cdl() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(35.0)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(28.0)),
            ("mixed_air_temp".into(), json!(28.0)),
            ("actual_supply_air_temp".into(), json!(20.0)),
            ("duct_pressure".into(), json!(0.2)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(1.0)),
            ("actual_return_damper_position".into(), json!(0.0)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        for step in 0..13 {
            let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            for damper in ["outdoor", "return"] {
                let alarm = format!("{damper}_damper_tracking_alarm_level");
                let expected = if step < 10 { 0.0 } else { 3.0 };
                assert_eq!(result[&alarm].as_f64(), Some(expected));
                assert_eq!(fast[&alarm].as_f64(), Some(expected));
            }
        }
        inputs.insert("actual_outdoor_damper_position".into(), json!(0.2));
        inputs.insert("actual_return_damper_position".into(), json!(0.8));
        let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        for damper in ["outdoor", "return"] {
            let alarm = format!("{damper}_damper_tracking_alarm_level");
            assert_eq!(result[&alarm].as_f64(), Some(0.0));
            assert_eq!(fast[&alarm].as_f64(), Some(0.0));
        }
    }
    #[test]
    fn physical_fan_failure_reduces_air_delivery_and_cdl_reports_it() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 28.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        let mut failed = Simulation::new(
            graph,
            &weather,
            r#"{"fan_faults":[{"equipment":"ahu_1_stuck_off","start_hour":0,"duration_hours":2}]}"#,
        )
        .unwrap();
        normal.temps = [29.0; 10];
        failed.temps = [29.0; 10];
        let alarm = "FourPipeG36Controls|output.ahu_1_supply_fan_alarm_level";
        failed.set_traces(&json!([alarm]).to_string()).unwrap();
        let normal_rows = run_weather_intervals(&mut normal, 2);
        let failed_rows = run_weather_intervals(&mut failed, 2);
        assert!(normal.last_fan_running[0]);
        assert!(!failed.last_fan_running[0]);
        assert!(
            normal_rows["rows"][1]["zone_airflow"][0].as_f64().unwrap()
                > failed_rows["rows"][1]["zone_airflow"][0].as_f64().unwrap()
        );
        assert_eq!(failed_rows["rows"][1]["traces"][alarm], json!(2.0));
    }
    #[test]
    fn physical_fan_stuck_on_uses_energy_and_cdl_reports_failed_stop() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 22.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: false
            };
            35_040
        ])
        .unwrap();
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        let mut stuck = Simulation::new(
            graph,
            &weather,
            r#"{"fan_faults":[{"equipment":"ahu_1_stuck_on","start_hour":0,"duration_hours":2}]}"#,
        )
        .unwrap();
        let alarm = "FourPipeG36Controls|output.ahu_1_supply_fan_alarm_level";
        stuck.set_traces(&json!([alarm]).to_string()).unwrap();
        let _ = run_weather_intervals(&mut normal, 2);
        let fault_rows = run_weather_intervals(&mut stuck, 2);
        assert!(!normal.last_fan_running[0]);
        assert!(stuck.last_fan_running[0]);
        assert!(stuck.electricity_kwh > normal.electricity_kwh);
        assert_eq!(fault_rows["rows"][1]["traces"][alarm], json!(4.0));
    }
    #[test]
    fn physical_fan_speed_cap_reduces_delivery_and_cdl_reports_tracking_fault() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 28.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        let mut capped = Simulation::new(
            graph,
            &weather,
            r#"{"fan_faults":[{"equipment":"ahu_1_speed_capped","start_hour":0,"duration_hours":2}]}"#,
        )
        .unwrap();
        normal.temps = [29.0; 10];
        capped.temps = [29.0; 10];
        let command = "FourPipeG36Controls|output.ahu_1_fan_speed";
        let alarm = "FourPipeG36Controls|output.ahu_1_fan_speed_tracking_alarm_level";
        capped
            .set_traces(&json!([command, alarm]).to_string())
            .unwrap();
        let normal_rows = run_weather_intervals(&mut normal, 4);
        let fault_rows = run_weather_intervals(&mut capped, 4);
        let normal_row = &normal_rows["rows"][3];
        let fault_row = &fault_rows["rows"][3];
        assert!(fault_row["traces"][command].as_f64().unwrap() > 0.45);
        assert_eq!(fault_row["ahu_1_actual_fan_speed"], json!(0.3));
        assert!(normal_row["ahu_1_actual_fan_speed"].as_f64().unwrap() > 0.3);
        assert!(
            normal_row["delivered_airflow"].as_f64().unwrap()
                > fault_row["delivered_airflow"].as_f64().unwrap()
        );
        assert_eq!(fault_row["traces"][alarm], json!(3.0));
    }
    #[test]
    fn physical_ahu_damper_faults_change_air_and_cdl_reports_tracking() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 35.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let mut normal = Simulation::new(graph, &weather, "{}").unwrap();
        let mut outdoor = Simulation::new(
            graph,
            &weather,
            r#"{"ahu_damper_faults":[{"ahu":1,"damper":"outdoor","stuck_position":1,"start_hour":0,"duration_hours":12}]}"#,
        )
        .unwrap();
        let mut return_air = Simulation::new(
            graph,
            &weather,
            r#"{"ahu_damper_faults":[{"ahu":1,"damper":"return","stuck_position":0,"start_hour":0,"duration_hours":12}]}"#,
        )
        .unwrap();
        for sim in [&mut normal, &mut outdoor, &mut return_air] {
            sim.temps = [28.0; 10];
            // Start during the synthetic occupied period so fan proof can
            // qualify the damper tracking alarm after its ten-minute delay.
            sim.cursor = 32 * 900 / sim.effective_step_seconds;
        }
        let outdoor_alarm = "FourPipeG36Controls|output.ahu_1_outdoor_damper_tracking_alarm_level";
        let return_alarm = "FourPipeG36Controls|output.ahu_1_return_damper_tracking_alarm_level";
        let traces = json!([outdoor_alarm, return_alarm]).to_string();
        outdoor.set_traces(&traces).unwrap();
        return_air.set_traces(&traces).unwrap();
        let normal_rows = run_weather_intervals(&mut normal, 4);
        let outdoor_rows = run_weather_intervals(&mut outdoor, 4);
        let return_rows = run_weather_intervals(&mut return_air, 4);
        let base = &normal_rows["rows"][3];
        let open = &outdoor_rows["rows"][3];
        let closed = &return_rows["rows"][3];
        assert!(
            base["ahu_1_actual_outdoor_damper_position"]
                .as_f64()
                .unwrap()
                < 0.7
        );
        assert_eq!(open["ahu_1_actual_outdoor_damper_position"], json!(1.0));
        assert!(
            open["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
                > base["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
        );
        assert!(
            open["ahu_1_mixed_air_c"].as_f64().unwrap()
                > base["ahu_1_mixed_air_c"].as_f64().unwrap()
        );
        assert_eq!(
            open["traces"][outdoor_alarm],
            json!(3.0),
            "outdoor fault row: {open}"
        );
        assert_eq!(closed["ahu_1_actual_return_damper_position"], json!(0.0));
        // The economizer can compensate for a closed return path by opening
        // outdoor air. The resulting mix changes even if fan delivery holds.
        assert!(
            closed["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
                > base["ahu_1_outdoor_air_fraction"].as_f64().unwrap()
        );
        assert!(
            closed["ahu_1_mixed_air_c"].as_f64().unwrap()
                > base["ahu_1_mixed_air_c"].as_f64().unwrap()
        );
        assert_eq!(
            closed["traces"][return_alarm],
            json!(3.0),
            "return fault row: {closed}"
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
            ("mixed_air_temp".into(), json!(0.0)),
            ("actual_supply_air_temp".into(), json!(3.5)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.0)),
            ("freeze_stat_trip".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        for step in 0..6 {
            let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(result["freeze_stage_one"], json!(step == 5));
            assert_eq!(
                result["freeze_stage_one"].as_bool().unwrap(),
                fast["freeze_stage_one"].as_bool().unwrap()
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
                fast["freeze_stage_one"].as_bool().unwrap()
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
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        for step in 0..6 {
            let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(result["freeze_stage_two"], json!(step == 5));
            assert_eq!(
                result["freeze_stage_two"].as_bool().unwrap(),
                fast["freeze_stage_two"].as_bool().unwrap()
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
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
        let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(result["freeze_stage_three"], json!(true));
        assert_eq!(fast["freeze_stage_three"], json!(true));
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
            json!(true)
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
                json!(true)
            );
        }
        inputs.insert("freeze_reset".into(), json!(true));
        assert_eq!(
            dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["freeze_stage_three"],
            json!(false)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["freeze_stage_three"],
            json!(false)
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
                ("actual_fan_speed".into(), json!(1.0)),
                ("actual_outdoor_damper_position".into(), json!(0.2)),
                ("actual_return_damper_position".into(), json!(0.8)),
                ("occupied_mode".into(), json!(true)),
                ("freeze_reset".into(), json!(false)),
            ]);
            add_ahu_ventilation_inputs(&mut inputs);
            for zone in 1..=5 {
                inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
                inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
            }
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
            for step in 1..=trigger_step {
                let result = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
                let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
                assert_eq!(result["freeze_stage_three"], json!(step == trigger_step));
                assert_eq!(fast["freeze_stage_three"], json!(step == trigger_step));
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
    fn g36_supply_air_mode_selection_matches_pinned_obc_priority() {
        let building: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["name"] == "ahu_1")
            .unwrap();
        let mut branch = json!({
            "name":"SupplyModeReference","block_type":"Composite","category":"composite",
            "inputs":[{"name":"operation_mode","type":"Integer"},
                      {"name":"supply_fan_proven","type":"Boolean"},
                      {"name":"reset_max","type":"Real"},
                      {"name":"outdoor_c","type":"Real"}],
            "outputs":[{"name":"supply_air_temp","type":"Real"}],
            "blocks":ahu["blocks"].as_array().unwrap().iter()
                .filter(|block| (block["name"].as_str().unwrap().starts_with("sat_mode_")
                    && !block["name"].as_str().unwrap().ends_with("_permitted"))
                    || ["sat_minimum", "sat_outdoor_low", "sat_outdoor_high", "sat_weather_line"]
                        .contains(&block["name"].as_str().unwrap()))
                .cloned().collect::<Vec<_>>(),
            "connections":ahu["connections"].as_array().unwrap().iter()
                .filter(|edge| {
                    let target=edge["to"].as_str().unwrap();
                    (target.starts_with("sat_mode_")
                        && !target.split('.').next().unwrap().ends_with("_permitted"))
                        || target.starts_with("sat_weather_line.")
                })
                .map(|edge| {
                    let mut copy=edge.clone();
                    if copy["from"] == "sat_output.y" { copy["from"] = json!("reset_max"); }
                    copy
                }).collect::<Vec<_>>()
        });
        branch["connections"]
            .as_array_mut()
            .unwrap()
            .push(json!({"from":"sat_mode_fan_select.y","to":"supply_air_temp"}));
        let branch: Block = serde_json::from_value(branch).unwrap();
        assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        for (mode, expected) in [
            (0, 26.0),
            (1, 17.5),
            (2, 17.5),
            (3, 12.0),
            (4, 35.0),
            (5, 35.0),
            (6, 26.0),
            (7, 26.0),
        ] {
            for fan in [false, true] {
                let inputs = HashMap::from([
                    ("operation_mode".into(), json!(mode)),
                    ("supply_fan_proven".into(), json!(fan)),
                    ("reset_max".into(), json!(17.5)),
                    ("outdoor_c".into(), json!(16.0)),
                ]);
                let expected = if fan { expected } else { 26.0 };
                assert_eq!(
                    dynamic.execute(&branch, &inputs, 60.0, "").unwrap()["supply_air_temp"],
                    json!(expected),
                    "mode {mode}, fan {fan}"
                );
                assert_eq!(
                    compiled.execute(&inputs, 60.0, false).unwrap().0["supply_air_temp"],
                    json!(expected),
                    "mode {mode}, fan {fan}"
                );
            }
        }
        let source = include_str!("../../verification/g36_supply_temperature_dymola.txt");
        let series = |name: &str| -> Vec<f64> {
            source
                .lines()
                .find(|line| line.starts_with(name))
                .unwrap()
                .split_once('=')
                .unwrap()
                .1
                .trim_matches(['[', ']'])
                .split(',')
                .map(|field| field.trim().parse::<f64>().unwrap())
                .collect()
        };
        let modes = series("conTSupSet.uOpeMod=");
        let fans = series("conTSupSet.u1SupFan=");
        let outdoor = series("conTSupSet.TOut=");
        let reset_max = series("conTSupSet.maxSupTemRes.y=");
        let expected = series("conTSupSet.TAirSupSet=");
        assert_eq!(expected.len(), 101);
        for sample in 0..expected.len() {
            // Feed the archived trim output to the actual editable Line block.
            let inputs = HashMap::from([
                ("operation_mode".into(), json!(modes[sample] as i64)),
                ("supply_fan_proven".into(), json!(fans[sample] > 0.5)),
                ("reset_max".into(), json!(reset_max[sample] - 273.15)),
                ("outdoor_c".into(), json!(outdoor[sample] - 273.15)),
            ]);
            for output in [
                dynamic.execute(&branch, &inputs, 864.0, "").unwrap(),
                compiled.execute(&inputs, 864.0, false).unwrap().0,
            ] {
                let actual = output["supply_air_temp"].as_f64().unwrap();
                assert!(
                    (actual - (expected[sample] - 273.15)).abs() < 5e-4,
                    "OBC supply mode sample {sample}: {actual} vs {}",
                    expected[sample] - 273.15
                );
            }
        }
    }
    fn g36_pressure_reset_branch() -> Block {
        let building: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["name"] == "ahu_1")
            .unwrap();
        let names: std::collections::HashSet<&str> = [
            "ignored_request_count",
            "request_count_real",
            "pressure_request_sample",
            "ignored_request_real",
            "excess_request_count",
            "zero_requests",
            "positive_request_count",
            "response_per_request",
            "maximum_response",
            "bounded_response",
            "trim_amount",
            "net_pressure_reset",
            "request_delay",
            "initial_pressure_target",
            "low_pressure_target",
            "high_pressure_target",
            "pressure_previous",
            "pressure_target_sum",
            "pressure_above_minimum",
            "pressure_below_maximum",
            "pressure_candidate",
            "pressure_target_output",
        ]
        .into_iter()
        .collect();
        let blocks: Vec<Value> = ahu["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|block| names.contains(block["name"].as_str().unwrap()))
            .cloned()
            .collect();
        assert_eq!(blocks.len(), names.len());
        let mut connections: Vec<Value> = ahu["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|edge| {
                let source = edge["from"].as_str().unwrap().split('.').next().unwrap();
                let target = edge["to"].as_str().unwrap().split('.').next().unwrap();
                names.contains(source) && names.contains(target)
            })
            .cloned()
            .collect();
        connections.extend([
            json!({"from":"request_count","to":"request_count_real.u"}),
            json!({"from":"fan_requested","to":"request_delay.u"}),
            json!({"from":"pressure_target_output.y","to":"setpoint"}),
        ]);
        let graph: Block = serde_json::from_value(json!({
            "name":"PressureResetReference","block_type":"Composite","category":"composite",
            "inputs":[{"name":"request_count","type":"Integer"},
                      {"name":"fan_requested","type":"Boolean"}],
            "outputs":[{"name":"setpoint","type":"Real"}],
            "blocks":blocks,"connections":connections
        }))
        .unwrap();
        assert!(validate(&graph).is_empty(), "{:?}", validate(&graph));
        graph
    }

    #[test]
    fn g36_pressure_trim_waits_for_startup_and_one_sample_in_editable_cdl() {
        let graph = g36_pressure_reset_branch();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        for tick in 0..=16 {
            let inputs = HashMap::from([
                ("request_count".into(), json!(5)),
                ("fan_requested".into(), json!(true)),
            ]);
            let ordinary = dynamic.execute(&graph, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            let value = ordinary["setpoint"].as_f64().unwrap();
            assert!(
                (value - fast["setpoint"].as_f64().unwrap()).abs() < 1e-9,
                "executors disagree at tick {tick}: dynamic {value}, compiled {}",
                fast["setpoint"]
            );
            if tick * 60 < 720 {
                assert!((value - 0.35).abs() < 1e-9, "tick {tick}: {value}");
            }
            if tick == 16 {
                assert!(value > 0.35, "pressure reset never started: {value}");
            }
        }
    }

    #[test]
    fn g36_pressure_reset_replays_pinned_obc_supply_fan_requests() {
        let mut graph = g36_pressure_reset_branch();
        for (name, parameter, value) in [
            ("initial_pressure_target", "k", 0.24),
            ("low_pressure_target", "k", 0.05),
            ("high_pressure_target", "k", 0.8),
            ("pressure_previous", "y_start", 0.24),
            ("trim_amount", "k", -0.024),
            ("response_per_request", "k", 0.03),
            ("maximum_response", "k", 0.064),
        ] {
            graph
                .blocks
                .iter_mut()
                .find(|block| block.name == name)
                .unwrap()
                .parameters
                .iter_mut()
                .find(|item| item.name == parameter)
                .unwrap()
                .value = json!(value);
        }
        let archive = include_str!("../../verification/g36_supply_fan_dymola.txt");
        let expected: Vec<f64> = archive
            .lines()
            .find(|line| line.starts_with("conSupFan.staPreSetRes.y="))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|field| field.trim().parse::<f64>().unwrap() / 500.0)
            .collect();
        assert_eq!(expected.len(), 101);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        let mut prior_value = None;
        for tick in 0..=28_800 {
            let seconds = tick as f64;
            let phase = std::f64::consts::TAU * seconds / 14_400.0;
            let count = (3.0 + 2.0 * phase.sin()).abs().round() as i64;
            let mode = (6.0 * seconds / 28_800.0).abs().round() as i64;
            let inputs = HashMap::from([
                ("request_count".into(), json!(count)),
                ("fan_requested".into(), json!((1..=3).contains(&mode))),
            ]);
            let ordinary = dynamic.execute(&graph, &inputs, 1.0, "").unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
            let value = ordinary["setpoint"].as_f64().unwrap();
            assert!((value - fast["setpoint"].as_f64().unwrap()).abs() < 1e-9);
            if tick % 288 == 0 {
                let index = tick / 288;
                // The Dymola archive records the left limit when its 288 s
                // observation lands on the 120 s reset event. At other
                // observations, compare the settled value at that instant.
                let observed = if tick > 0 && tick % 120 == 0 {
                    prior_value.unwrap_or(value)
                } else {
                    value
                };
                let difference = (observed - expected[index]).abs();
                assert!(
                    difference < 1e-9,
                    "OBC pressure reset sample {index} at {tick}s: runtime {} Pa, archived {} Pa",
                    observed * 500.0,
                    expected[index] * 500.0
                );
                if index == 11 {
                    assert!(difference < 1e-9, "first OBC pressure response: {value}");
                }
            }
            prior_value = Some(value);
        }
    }

    #[test]
    fn g36_sampled_supply_reset_tracks_pinned_obc_day() {
        let building: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["name"] == "ahu_1")
            .unwrap();
        let branch_blocks: Vec<Value> = ahu["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|block| {
                let name = block["name"].as_str().unwrap();
                name.starts_with("sat_")
                    && !name.starts_with("sat_request_sum_")
                    && name != "sat_request_real"
                    && !name.ends_with("_permitted")
            })
            .cloned()
            .collect();
        let names: std::collections::HashSet<&str> = branch_blocks
            .iter()
            .map(|block| block["name"].as_str().unwrap())
            .collect();
        let mut connections: Vec<Value> = ahu["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|edge| {
                let from = edge["from"].as_str().unwrap();
                let to = edge["to"].as_str().unwrap();
                let source = from.split('.').next().unwrap();
                let target = to.split('.').next().unwrap();
                if !names.contains(target)
                    || (!names.contains(source)
                        && !["operation_mode", "supply_fan_proven", "outdoor_c"].contains(&source))
                {
                    return None;
                }
                Some(edge.clone())
            })
            .collect();
        connections.push(json!({"from":"request_count","to":"sat_request_sample.u"}));
        connections.push(json!({"from":"sat_output.y","to":"reset_max"}));
        connections.push(json!({"from":"sat_mode_fan_select.y","to":"supply_air_temp"}));
        let branch: Block = serde_json::from_value(json!({
            "name":"SupplyResetReference","block_type":"Composite","category":"composite",
            "inputs":[{"name":"request_count","type":"Real"},
                      {"name":"operation_mode","type":"Integer"},
                      {"name":"supply_fan_proven","type":"Boolean"},
                      {"name":"outdoor_c","type":"Real"}],
            "outputs":[{"name":"reset_max","type":"Real"},
                       {"name":"supply_air_temp","type":"Real"}],
            "blocks":branch_blocks,"connections":connections
        }))
        .unwrap();
        assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
        let source = include_str!("../../verification/g36_supply_temperature_dymola.txt");
        let series = |name: &str| -> Vec<f64> {
            source
                .lines()
                .find(|line| line.starts_with(name))
                .unwrap()
                .split_once('=')
                .unwrap()
                .1
                .trim_matches(['[', ']'])
                .split(',')
                .map(|field| field.trim().parse::<f64>().unwrap())
                .collect()
        };
        let expected_max = series("conTSupSet.maxSupTemRes.y=");
        let expected_supply = series("conTSupSet.TAirSupSet=");
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        let mut excess_max = 0.0_f64;
        let mut excess_supply = 0.0_f64;
        let mut previous_max = 18.0_f64;
        let mut previous_supply = 15.6_f64;
        // One-second input resolution catches request threshold crossings
        // close to a control sample without changing the 120-second clock.
        for tick in 0..=86_400 {
            let seconds = tick as f64;
            let phase = std::f64::consts::TAU * seconds / 86_400.0;
            let request_count = (6.0 * phase.sin()).abs().round();
            let fan = tick == 0 || (seconds - 1e-9).rem_euclid(43_200.0) < 21_600.0;
            let mode = (1.0 + seconds / 90_000.0).round() as i64;
            let inputs = HashMap::from([
                ("request_count".into(), json!(request_count)),
                ("operation_mode".into(), json!(mode)),
                ("supply_fan_proven".into(), json!(fan)),
                ("outdoor_c".into(), json!(18.0 + 5.0 * phase.sin())),
            ]);
            let ordinary = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
            for name in ["reset_max", "supply_air_temp"] {
                assert!(
                    (ordinary[name].as_f64().unwrap() - fast[name].as_f64().unwrap()).abs() < 1e-8,
                    "tick {tick}, {name}"
                );
            }
            if tick % 864 == 0 {
                let index = tick / 864;
                // At a 120-second event, the archive records the left limit.
                let at_event = tick > 0 && tick % 120 == 0;
                let max_value = if at_event {
                    previous_max
                } else {
                    ordinary["reset_max"].as_f64().unwrap()
                };
                let supply_value = if at_event {
                    previous_supply
                } else {
                    ordinary["supply_air_temp"].as_f64().unwrap()
                };
                excess_max = excess_max.max((max_value - (expected_max[index] - 273.15)).abs());
                excess_supply =
                    excess_supply.max((supply_value - (expected_supply[index] - 273.15)).abs());
            }
            previous_max = ordinary["reset_max"].as_f64().unwrap();
            previous_supply = ordinary["supply_air_temp"].as_f64().unwrap();
        }
        // The finer input drive resolves the validation sine's rounded
        // request transitions before the 120-second sampler consumes them.
        assert!(
            excess_max < 0.0005 && excess_supply < 0.0005,
            "OBC day excess: max {excess_max} K, supply {excess_supply} K"
        );
    }
    #[test]
    fn g36_normal_coil_commands_require_fan_proof_in_editable_cdl() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        for mixed in [0.0, 30.0] {
            let mut inputs = HashMap::from([
                ("outdoor_c".into(), json!(0.0)),
                ("flow_request".into(), json!(5.0)),
                ("return_air_temp".into(), json!(24.0)),
                ("mixed_air_temp".into(), json!(mixed)),
                ("actual_supply_air_temp".into(), json!(16.0)),
                ("duct_pressure".into(), json!(0.4)),
                ("outdoor_airflow".into(), json!(0.0)),
                ("freeze_stat_trip".into(), json!(false)),
                ("supply_fan_proven".into(), json!(false)),
                ("actual_fan_speed".into(), json!(0.0)),
                ("actual_outdoor_damper_position".into(), json!(0.2)),
                ("actual_return_damper_position".into(), json!(0.8)),
                ("occupied_mode".into(), json!(true)),
                ("freeze_reset".into(), json!(false)),
            ]);
            add_ahu_ventilation_inputs(&mut inputs);
            for zone in 1..=5 {
                inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
                inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
            }
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
            let normal = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            for key in ["heating_coil_valve_position", "cooling_coil_valve_position"] {
                assert_eq!(normal[key], json!(0.0), "mixed {mixed}, {key}");
                assert_eq!(fast[key], normal[key], "mixed {mixed}, {key}");
            }
            inputs.insert("supply_fan_proven".into(), json!(true));
            let normal = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            let active = if mixed < 10.0 {
                "heating_coil_valve_position"
            } else {
                "cooling_coil_valve_position"
            };
            assert!(
                normal[active].as_f64().unwrap() > 0.0,
                "mixed {mixed}, {active}"
            );
            assert_eq!(fast[active], normal[active]);
        }
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
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
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
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(3));
        }
        let mut high = HashMap::new();
        for _ in 0..90 {
            high = dynamic.execute(ahu, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(fast["sat_request_count"], high["sat_request_count"]);
            assert_eq!(fast["supply_air_temp"], high["supply_air_temp"]);
        }
        assert_eq!(high["sat_request_count"], json!(15));
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
        assert!(
            minimum(&reset_rows, "ahu_1_supply_setpoint_c")
                < minimum(&neutral_rows, "ahu_1_supply_setpoint_c") - 0.1,
            "request response must lower the outdoor-reset target"
        );
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
        // The first call is t=0; the reset clock reaches t=600 on call 11.
        for _ in 0..11 {
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
            inputs.insert(format!("zone_{zone:02}_hot_water_reset_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_hot_water_plant_request"), json!(0));
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
    fn g36_ahu_plant_request_enables_water_loop_in_cdl() {
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
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(plant).unwrap().unwrap();
        for valve in [0.0, 1.0] {
            inputs.insert("ahu_1_heating_valve".into(), json!(valve));
            inputs.insert("ahu_1_cooling_valve".into(), json!(valve));
            let ordinary = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            for result in [&ordinary, &fast] {
                let enabled = valve > 0.95;
                assert_eq!(result["ahu_1_hot_plant_request"], json!(enabled as i64));
                assert_eq!(result["ahu_1_chilled_plant_request"], json!(enabled as i64));
                assert_eq!(result["hot_water_enable"], json!(enabled));
                assert_eq!(result["chilled_water_enable"], json!(enabled));
            }
        }
    }

    #[test]
    fn g36_ahu_plant_requests_match_pinned_obc_trace() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let plant = building
            .blocks
            .iter()
            .find(|block| block.name == "plant")
            .unwrap();
        let mut value = serde_json::to_value(plant).unwrap();
        value["name"] = json!("AHUPlantRequests");
        let inputs = [
            "ahu_1_supply_actual_c",
            "ahu_1_supply_setpoint_c",
            "ahu_1_heating_valve",
            "ahu_1_cooling_valve",
        ];
        value["inputs"]
            .as_array_mut()
            .unwrap()
            .retain(|port| inputs.contains(&port["name"].as_str().unwrap()));
        value["outputs"] = json!([
            {"name":"hot_reset","type":"Integer","unit":"count"},
            {"name":"chilled_reset","type":"Integer","unit":"count"},
            {"name":"ahu_1_hot_plant_request","type":"Integer","unit":"count"},
            {"name":"ahu_1_chilled_plant_request","type":"Integer","unit":"count"}
        ]);
        value["blocks"]
            .as_array_mut()
            .unwrap()
            .retain(|block| block["name"].as_str().unwrap().starts_with("ahu_1_"));
        value["connections"].as_array_mut().unwrap().retain(|edge| {
            let from = edge["from"].as_str().unwrap();
            let to = edge["to"].as_str().unwrap();
            (to.starts_with("ahu_1_")
                || to == "ahu_1_hot_plant_request"
                || to == "ahu_1_chilled_plant_request")
                && (from.starts_with("ahu_1_") || inputs.contains(&from))
        });
        value["connections"].as_array_mut().unwrap().extend([
            json!({"from":"ahu_1_hot_select_three_requests.y","to":"hot_reset"}),
            json!({"from":"ahu_1_select_three_requests.y","to":"chilled_reset"}),
        ]);
        let request: Block = serde_json::from_value(value).unwrap();
        assert!(validate(&request).is_empty(), "{:?}", validate(&request));
        let source = include_str!("../../verification/g36_ahu_plant_requests_dymola.txt");
        let series = |name: &str| -> Vec<f64> {
            let prefix = format!("{name}=");
            source
                .lines()
                .find(|line| line.starts_with(&prefix))
                .unwrap()
                .split_once('=')
                .unwrap()
                .1
                .trim_matches(['[', ']'])
                .split(',')
                .map(|field| field.trim().parse::<f64>().unwrap())
                .collect()
        };
        let hot_actual = series("plaReq.TAirSup");
        let hot_target = series("plaReq.TAirSupSet");
        let hot_valve = series("plaReq.uHeaCoiSet");
        let hot_reset = series("plaReq.yHotWatResReq");
        let hot_plant = series("plaReq.yHotWatPlaReq");
        let chilled_actual = series("plaReq1.TAirSup");
        let chilled_target = series("plaReq1.TAirSupSet");
        let chilled_valve = series("plaReq1.uCooCoiSet");
        let chilled_reset = series("plaReq1.yChiWatResReq");
        let chilled_plant = series("plaReq1.yChiPlaReq");
        assert_eq!(hot_actual.len(), 101);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&request).unwrap().unwrap();
        for tick in 0..=3_600 {
            let sample = tick / 36;
            let seconds = tick as f64;
            let hot_valve_input = 0.96 - 0.3 * ((seconds - 1_000.0) / 3_600.0).clamp(0.0, 1.0);
            let inputs = HashMap::from([
                (
                    "ahu_1_supply_actual_c".into(),
                    json!(12.0 + 8.0 * seconds / 3_600.0),
                ),
                (
                    "ahu_1_supply_setpoint_c".into(),
                    json!(20.0 + 15.0 * seconds / 3_600.0),
                ),
                ("ahu_1_heating_valve".into(), json!(hot_valve_input)),
                ("ahu_1_cooling_valve".into(), json!(0.0)),
            ]);
            // The OBC validation uses two separate AHU instances with different
            // supply-temperature inputs. Check the hot and chilled paths in
            // separate runs below rather than mixing their temperature signals.
            let actual = dynamic.execute(&request, &inputs, 1.0, "").unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
            if tick % 36 != 0 {
                continue;
            }
            assert!((hot_actual[sample] - 273.15 - (12.0 + 8.0 * seconds / 3_600.0)).abs() < 5e-5);
            assert!((hot_target[sample] - 273.15 - (20.0 + 15.0 * seconds / 3_600.0)).abs() < 5e-5);
            assert!((hot_valve[sample] - hot_valve_input).abs() < 5e-5);
            for result in [&actual, &fast] {
                let expected = json!(hot_reset[sample] as i64);
                assert_eq!(result["hot_reset"], expected, "hot reset sample {sample}");
                assert_eq!(
                    result["ahu_1_hot_plant_request"],
                    json!(hot_plant[sample] as i64),
                    "hot plant sample {sample}"
                );
            }
        }
        // Re-run with the second OBC AHU's chilled-water temperature trace.
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&request).unwrap().unwrap();
        for tick in 0..=3_600 {
            let sample = tick / 36;
            let seconds = tick as f64;
            let chilled_valve_input = 0.96 - 0.3 * ((seconds - 1_000.0) / 3_600.0).clamp(0.0, 1.0);
            let inputs = HashMap::from([
                (
                    "ahu_1_supply_actual_c".into(),
                    json!(15.0 + 8.0 * seconds / 3_600.0),
                ),
                (
                    "ahu_1_supply_setpoint_c".into(),
                    json!(14.5 + 6.0 * seconds / 3_600.0),
                ),
                ("ahu_1_heating_valve".into(), json!(0.0)),
                ("ahu_1_cooling_valve".into(), json!(chilled_valve_input)),
            ]);
            let actual = dynamic.execute(&request, &inputs, 1.0, "").unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
            if tick % 36 != 0 {
                continue;
            }
            assert!(
                (chilled_actual[sample] - 273.15 - (15.0 + 8.0 * seconds / 3_600.0)).abs() < 5e-5
            );
            assert!(
                (chilled_target[sample] - 273.15 - (14.5 + 6.0 * seconds / 3_600.0)).abs() < 5e-5
            );
            assert!((chilled_valve[sample] - chilled_valve_input).abs() < 5e-5);
            for result in [&actual, &fast] {
                let expected = json!(chilled_reset[sample] as i64);
                assert_eq!(
                    result["chilled_reset"], expected,
                    "chilled reset sample {sample}"
                );
                assert_eq!(
                    result["ahu_1_chilled_plant_request"],
                    json!(chilled_plant[sample] as i64),
                    "chilled plant sample {sample}"
                );
            }
        }
    }

    #[test]
    fn g36_terminal_hot_water_tiers_follow_discharge_and_valve_feedback() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut request = serde_json::to_value(zone).unwrap();
        request["name"] = json!("TerminalHotRequests");
        request["inputs"] = json!([
            {"name":"target","type":"Real","unit":"°C"},
            {"name":"discharge_temp","type":"Real","unit":"°C"},
            {"name":"valve","type":"Real","unit":"fraction (0–1)"}
        ]);
        request["outputs"] = json!([
            {"name":"hot_water_reset_request","type":"Integer","unit":"count"},
            {"name":"hot_water_plant_request","type":"Integer","unit":"count"}
        ]);
        let is_request =
            |name: &str| name.starts_with("hot_reset_") || name.starts_with("hot_plant_");
        request["blocks"]
            .as_array_mut()
            .unwrap()
            .retain(|block| is_request(block["name"].as_str().unwrap()));
        request["connections"]
            .as_array_mut()
            .unwrap()
            .retain_mut(|edge| {
                let from = edge["from"].as_str().unwrap().to_owned();
                let to = edge["to"].as_str().unwrap();
                if !is_request(to.split('.').next().unwrap())
                    && to != "hot_water_reset_request"
                    && to != "hot_water_plant_request"
                {
                    return false;
                }
                if from == "discharge_target.y" {
                    edge["from"] = json!("target");
                } else if from == "reheat_capped.y" {
                    edge["from"] = json!("valve");
                } else if !is_request(from.split('.').next().unwrap()) && from != "discharge_temp" {
                    return false;
                }
                true
            });
        let request: Block = serde_json::from_value(request).unwrap();
        assert!(validate(&request).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&request).unwrap().unwrap();
        let mut inputs = HashMap::from([
            ("target".into(), json!(35.0)),
            ("discharge_temp".into(), json!(20.0)),
            ("valve".into(), json!(0.96)),
        ]);
        let mut step = |inputs: &HashMap<String, Value>| {
            let ordinary = dynamic.execute(&request, inputs, 60.0, "").unwrap();
            let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
            assert_eq!(
                ordinary["hot_water_reset_request"],
                fast["hot_water_reset_request"]
            );
            assert_eq!(
                ordinary["hot_water_plant_request"],
                fast["hot_water_plant_request"]
            );
            ordinary
        };
        // OBC TrueDelay defaults to delayOnInit=false: an initial deficit is
        // immediately eligible, while a later rising deficit waits five minutes.
        assert_eq!(step(&inputs)["hot_water_reset_request"], json!(2));
        for _ in 0..6 {
            step(&inputs);
        }
        assert_eq!(step(&inputs)["hot_water_reset_request"], json!(2));
        inputs.insert("discharge_temp".into(), json!(15.0));
        for _ in 0..6 {
            step(&inputs);
        }
        assert_eq!(step(&inputs)["hot_water_reset_request"], json!(3));
        inputs.insert("discharge_temp".into(), json!(35.0));
        inputs.insert("valve".into(), json!(0.9));
        assert_eq!(step(&inputs)["hot_water_reset_request"], json!(1));
        assert_eq!(step(&inputs)["hot_water_plant_request"], json!(1));
        inputs.insert("valve".into(), json!(0.8));
        assert_eq!(step(&inputs)["hot_water_reset_request"], json!(0));
        assert_eq!(step(&inputs)["hot_water_plant_request"], json!(1));
        inputs.insert("valve".into(), json!(0.05));
        assert_eq!(step(&inputs)["hot_water_plant_request"], json!(0));

        // The archived OBC validation records 101 output samples at 72 s
        // spacing. Its discharge deficit stays in the two-request band and
        // its valve never reaches the plant-request threshold.
        let source = include_str!("../../verification/g36_system_requests_dymola.txt");
        let series = |name: &str| -> Vec<f64> {
            let line = source.lines().find(|line| line.starts_with(name)).unwrap();
            line.split_once('=')
                .unwrap()
                .1
                .trim_matches(['[', ']'])
                .split(',')
                .map(|field| field.trim().parse::<f64>().unwrap())
                .collect()
        };
        let discharge = series("sysReq.TDis=");
        let valve = series("sysReq.uVal=");
        let reset = series("sysReq.yHeaValResReq=");
        let plant = series("sysReq.yHotWatPlaReq=");
        assert_eq!(
            (discharge.len(), valve.len(), reset.len(), plant.len()),
            (101, 101, 101, 101)
        );
        let mut reference_dynamic = Executor::default();
        let mut reference_compiled = CompiledExecutor::try_new(&request).unwrap().unwrap();
        for sample in 0..101 {
            let values = HashMap::from([
                ("target".into(), json!(30.0)),
                ("discharge_temp".into(), json!(discharge[sample] - 273.15)),
                ("valve".into(), json!(valve[sample])),
            ]);
            let actual = reference_dynamic
                .execute(&request, &values, 72.0, "")
                .unwrap();
            let fast = reference_compiled.execute(&values, 72.0, false).unwrap().0;
            for output in [&actual, &fast] {
                assert_eq!(
                    output["hot_water_reset_request"],
                    json!(reset[sample] as i64),
                    "sample {sample}"
                );
                assert_eq!(
                    output["hot_water_plant_request"],
                    json!(plant[sample] as i64),
                    "sample {sample}"
                );
            }
        }
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
        for (target, source) in [
            (
                ("hot_reset_request_real", "u"),
                ("hot_reset_with_freeze_requests", "y"),
            ),
            (
                ("hot_reset_with_freeze_requests", "u1"),
                ("hot_reset_all_request_count", "y"),
            ),
        ] {
            let drivers: Vec<_> = plant
                .connections
                .iter()
                .filter(|edge| edge.target() == (target.0.to_owned(), target.1.to_owned()))
                .map(Connection::source)
                .collect();
            assert_eq!(drivers, vec![(source.0.to_owned(), source.1.to_owned())]);
        }
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
            inputs.insert(format!("zone_{zone:02}_hot_water_reset_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_hot_water_plant_request"), json!(0));
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
        assert_eq!(chilled_fast["chilled_water_enable"], json!(true));
        inputs.insert("ahu_1_freeze_chilled_water_request".into(), json!(false));
        inputs.insert("heat_request_kw".into(), json!(100.0));
        inputs.insert("ahu_1_freeze_hot_water_request".into(), json!(0));
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_hot_water_reset_request"), json!(1));
            inputs.insert(format!("zone_{zone:02}_hot_water_plant_request"), json!(1));
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
        assert_eq!(terminal["terminal_hot_water_plant_request_count"], json!(5));
        assert!(terminal["hot_water_supply_setpoint_c"].as_f64().unwrap() > low_target + 1.0);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_hot_water_reset_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_hot_water_plant_request"), json!(0));
        }
        let cleared = dynamic.execute(plant, &inputs, 60.0, "").unwrap();
        let cleared_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
        assert_eq!(cleared["terminal_hot_water_request_count"], json!(0));
        assert_eq!(cleared["terminal_hot_water_plant_request_count"], json!(0));
        assert_eq!(cleared_fast["terminal_hot_water_request_count"], json!(0));
        assert_eq!(
            cleared_fast["terminal_hot_water_plant_request_count"],
            json!(0)
        );
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
        for zone in graph["blocks"].as_array_mut().unwrap() {
            if zone["block_type"] != "VAVBoxController" {
                continue;
            }
            zone["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "ventilation_standby_permitted")
                .unwrap()["parameters"][0]["value"] = json!(false);
        }
        let scenario = r#"{"design_occupants_per_zone":0}"#;
        let mut reset = Simulation::new(&graph.to_string(), &weather, scenario).unwrap();
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
        let mut fixed = Simulation::new(&graph.to_string(), &weather, scenario).unwrap();
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
    fn g36_co2_feedback_changes_airflow_and_zone_concentration_from_cdl() {
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
        let scenario = r#"{"zone_air_volume_m3":100,"co2_generation_lps_per_person":0.02}"#;
        let mut active = Simulation::new(graph, &weather, scenario).unwrap();
        let active_rows = run_weather_intervals(&mut active, 55);
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        for zone in edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|block| block["block_type"] == "VAVBoxController")
        {
            zone["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "co2_cooling_max_flow")
                .unwrap()["parameters"][0]["value"] = json!(0.2);
        }
        let mut muted = Simulation::new(&edited.to_string(), &weather, scenario).unwrap();
        let muted_rows = run_weather_intervals(&mut muted, 55);
        let active_last = active_rows["rows"].as_array().unwrap().last().unwrap();
        let muted_last = muted_rows["rows"].as_array().unwrap().last().unwrap();
        let peak_airflow = |rows: &Value| {
            rows["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["zone_airflow"][0].as_f64().unwrap())
                .fold(0.0_f64, f64::max)
        };
        assert!(
            peak_airflow(&active_rows) > peak_airflow(&muted_rows) + 0.02,
            "active peak flow {}, muted peak flow {}, active CO2 {}, muted CO2 {}",
            peak_airflow(&active_rows),
            peak_airflow(&muted_rows),
            active_last["zone_co2_ppm"][0],
            muted_last["zone_co2_ppm"][0]
        );
        assert!(
            active_last["zone_co2_ppm"][0].as_f64().unwrap()
                < muted_last["zone_co2_ppm"][0].as_f64().unwrap() - 50.0
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
                .find(|block| block["name"] == "sat_minimum")
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
    fn g36_zone_pressure_request_levels_are_editable_cdl() {
        let graph: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["name"] == "zone_01")
            .unwrap();
        let mut needed =
            std::collections::HashSet::from(["pressure_importance_select".to_string()]);
        loop {
            let mut changed = false;
            for edge in zone["connections"].as_array().unwrap() {
                let target = edge["to"].as_str().unwrap().split('.').next().unwrap();
                let source = edge["from"].as_str().unwrap().split('.').next().unwrap();
                if needed.contains(target) && source.starts_with("pressure_") {
                    changed |= needed.insert(source.to_string());
                }
            }
            if !changed {
                break;
            }
        }
        let mut branch = json!({
            "name": "PressureRequestLevels",
            "block_type": "Composite",
            "category": "composite",
            "inputs": [
                {"name":"damper_position","type":"Real","unit":"fraction (0–1)"},
                {"name":"airflow_setpoint","type":"Real","unit":"normalized airflow"},
                {"name":"measured_airflow","type":"Real","unit":"normalized airflow"}
            ],
            "outputs": [zone["outputs"].as_array().unwrap().iter()
                .find(|port| port["name"] == "pressure_reset_request").unwrap().clone()],
            "blocks": zone["blocks"].as_array().unwrap().iter()
                .filter(|block| needed.contains(block["name"].as_str().unwrap()))
                .cloned().collect::<Vec<_>>(),
            "connections": []
        });
        branch["connections"] = Value::Array(
            zone["connections"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|edge| {
                    let target = edge["to"].as_str().unwrap();
                    let source = edge["from"].as_str().unwrap();
                    if !needed.contains(target.split('.').next().unwrap())
                        && target != "pressure_reset_request"
                    {
                        return None;
                    }
                    let mut copy = edge.clone();
                    copy["from"] = json!(match source {
                        "heating_damper_select.y" => "damper_position",
                        "flow_override_select.y" => "airflow_setpoint",
                        other => other,
                    });
                    Some(copy)
                })
                .collect(),
        );
        let branch: Block = serde_json::from_value(branch).unwrap();
        assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
        let cases = [
            (0.96, 1.0, 0.40, 3),
            (0.96, 1.0, 0.60, 2),
            (0.96, 1.0, 0.80, 1),
            (0.90, 1.0, 0.40, 1),
            (0.90, 1.0, 0.80, 1),
            (0.84, 1.0, 0.80, 0),
            (0.96, 0.0, 0.0, 1),
        ];
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        for (damper, setpoint, measured, expected) in cases {
            let inputs = HashMap::from([
                ("damper_position".into(), json!(damper)),
                ("airflow_setpoint".into(), json!(setpoint)),
                ("measured_airflow".into(), json!(measured)),
            ]);
            for _ in 0..2 {
                let result = dynamic.execute(&branch, &inputs, 60.0, "").unwrap();
                let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
                assert_eq!(
                    result["pressure_reset_request"], fast["pressure_reset_request"],
                    "damper={damper}, setpoint={setpoint}, measured={measured}"
                );
            }
            let ordinary = dynamic.execute(&branch, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                ordinary["pressure_reset_request"],
                fast["pressure_reset_request"]
            );
            assert_eq!(
                ordinary["pressure_reset_request"],
                json!(expected),
                "damper={damper}, setpoint={setpoint}, measured={measured}"
            );
        }
        // OBC samples damper and flow together at 120 s. Once the damper
        // qualifies, a newly sampled flow deficit raises the tier immediately;
        // it does not start a second one-minute flow-deficit timer.
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        let mut inputs = HashMap::from([
            ("damper_position".into(), json!(0.96)),
            ("airflow_setpoint".into(), json!(1.0)),
            ("measured_airflow".into(), json!(0.8)),
        ]);
        let mut step = |inputs: &HashMap<String, Value>, expected: i64| {
            let ordinary = dynamic.execute(&branch, inputs, 60.0, "").unwrap();
            let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
            assert_eq!(ordinary["pressure_reset_request"], json!(expected));
            assert_eq!(fast["pressure_reset_request"], json!(expected));
        };
        step(&inputs, 1); // initial sample
        step(&inputs, 1); // t=60 s: held between samples
        inputs.insert("measured_airflow".into(), json!(0.4));
        step(&inputs, 3); // t=120 s: coherent sample raises the tier
        step(&inputs, 3); // t=180 s: held between samples
        inputs.insert("damper_position".into(), json!(0.9));
        step(&inputs, 1); // t=240 s: high gate clears, one-request latch remains
        step(&inputs, 1); // t=300 s: held between samples
        let importance = branch
            .blocks
            .iter()
            .find(|block| block.name == "pressure_importance")
            .unwrap();
        assert_eq!(importance.parameters[0].value, json!(1.0));
        let mut disabled = branch.clone();
        disabled
            .blocks
            .iter_mut()
            .find(|block| block.name == "pressure_importance")
            .unwrap()
            .parameters[0]
            .value = json!(0.0);
        let inputs = HashMap::from([
            ("damper_position".into(), json!(1.0)),
            ("airflow_setpoint".into(), json!(1.0)),
            ("measured_airflow".into(), json!(0.0)),
        ]);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&disabled).unwrap().unwrap();
        for _ in 0..3 {
            assert_eq!(
                dynamic.execute(&disabled, &inputs, 60.0, "").unwrap()["pressure_reset_request"],
                json!(0)
            );
            assert_eq!(
                compiled.execute(&inputs, 60.0, false).unwrap().0["pressure_reset_request"],
                json!(0)
            );
        }
    }
    #[test]
    fn g36_zone_sat_requests_match_pinned_obc_trace() {
        let source = include_str!("../../verification/g36_system_requests_dymola.txt");
        let series = |name: &str| -> Vec<f64> {
            let line = source.lines().find(|line| line.starts_with(name)).unwrap();
            let values = line.split_once('=').unwrap().1.trim();
            values[1..values.len() - 1]
                .split(',')
                .map(|value| value.trim().parse::<f64>().unwrap())
                .collect()
        };
        let setpoint = series("sysReq.TCooSet=");
        let expected = series("sysReq.yZonTemResReq=");
        assert_eq!(setpoint.len(), 101);
        assert_eq!(expected.len(), 101);
        let building: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["name"] == "zone_01")
            .unwrap();
        let mut branch = json!({
            "name":"CoolingSatRequestsReference",
            "block_type":"Composite",
            "category":"composite",
            "inputs":[
                {"name":"room_temp","type":"Real","unit":"°C"},
                {"name":"cool_setpoint","type":"Real","unit":"°C"},
                {"name":"cooling_loop","type":"Real","unit":"fraction (0–1)"},
                {"name":"after_suppression","type":"Boolean"}
            ],
            "outputs":[{"name":"cooling_sat_reset_request","type":"Integer","unit":"count"}],
            "blocks":zone["blocks"].as_array().unwrap().iter()
                .filter(|block| block["name"].as_str().unwrap().starts_with("sat_"))
                .cloned().collect::<Vec<_>>(),
            "connections":zone["connections"].as_array().unwrap().iter()
                .filter(|edge| edge["to"].as_str().unwrap().starts_with("sat_")
                    || edge["to"] == "cooling_sat_reset_request")
                .map(|edge| {
                    let mut copy = edge.clone();
                    copy["from"] = json!(match edge["from"].as_str().unwrap() {
                        "window_cool_target.y" => "cool_setpoint",
                        "cool_pi.y" => "cooling_loop",
                        other => other,
                    });
                    if copy["to"] == "sat_two_eligible.u2" || copy["to"] == "sat_three_eligible.u2" {
                        copy["from"] = json!("after_suppression");
                    }
                    copy
                }).collect::<Vec<_>>()
        });
        for block in branch["blocks"].as_array_mut().unwrap() {
            if block["name"] == "sat_cooling_loop_high" {
                block["parameters"][0]["value"] = json!(0.94);
            }
        }
        let branch: Block = serde_json::from_value(branch).unwrap();
        assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        let mut actual = Vec::new();
        for tick in 0..=600 {
            let time = tick as f64 * 12.0;
            let recorded = tick / 6;
            let room = 299.15 + 2.0 * (std::f64::consts::TAU * time / 7200.0).sin();
            let cooling_loop = 1.0 - ((time - 1000.0) / 2000.0).clamp(0.0, 1.0);
            let inputs = HashMap::from([
                ("room_temp".into(), json!(room - 273.15)),
                ("cool_setpoint".into(), json!(setpoint[recorded] - 273.15)),
                ("cooling_loop".into(), json!(cooling_loop)),
                ("after_suppression".into(), json!(time > 720.0)),
            ]);
            let slow = dynamic.execute(&branch, &inputs, 12.0, "").unwrap();
            let fast = compiled.execute(&inputs, 12.0, false).unwrap().0;
            assert_eq!(
                slow["cooling_sat_reset_request"],
                fast["cooling_sat_reset_request"]
            );
            if tick % 6 == 0 {
                actual.push(slow["cooling_sat_reset_request"].as_i64().unwrap());
            }
        }
        let mismatches: Vec<_> = actual
            .iter()
            .zip(expected.iter())
            .enumerate()
            .filter(|(_, (actual, expected))| **actual as f64 != **expected)
            .map(|(index, (actual, expected))| (index, *actual, *expected as i64))
            .collect();
        assert!(
            mismatches.is_empty(),
            "SAT request mismatches: {mismatches:?}"
        );
    }

    #[test]
    fn g36_zone_sat_requests_and_setpoint_suppression_are_editable_cdl() {
        let graph: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["name"] == "zone_01")
            .unwrap();
        let mut branch = json!({
            "name": "CoolingSatRequests",
            "block_type": "Composite",
            "category": "composite",
            "inputs": [
                {"name":"room_temp","type":"Real","unit":"°C"},
                {"name":"cool_setpoint","type":"Real","unit":"°C"},
                {"name":"cooling_loop","type":"Real","unit":"fraction (0–1)"}
            ],
            "outputs": [zone["outputs"].as_array().unwrap().iter()
                .find(|port| port["name"] == "cooling_sat_reset_request").unwrap().clone()],
            "blocks": zone["blocks"].as_array().unwrap().iter()
                .filter(|block| block["name"].as_str().unwrap().starts_with("sat_"))
                .cloned().collect::<Vec<_>>(),
            "connections": []
        });
        branch["connections"] = Value::Array(
            zone["connections"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|edge| {
                    let target = edge["to"].as_str().unwrap();
                    if !target.starts_with("sat_") && target != "cooling_sat_reset_request" {
                        return None;
                    }
                    let source = edge["from"].as_str().unwrap();
                    let mut copy = edge.clone();
                    copy["from"] = json!(match source {
                        "window_cool_target.y" => "cool_setpoint",
                        "cool_pi.y" => "cooling_loop",
                        other => other,
                    });
                    Some(copy)
                })
                .collect(),
        );
        let branch: Block = serde_json::from_value(branch).unwrap();
        assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        let mut evaluate = |room: f64, setpoint: f64, loop_value: f64, steps: usize| -> Value {
            let inputs = HashMap::from([
                ("room_temp".into(), json!(room)),
                ("cool_setpoint".into(), json!(setpoint)),
                ("cooling_loop".into(), json!(loop_value)),
            ]);
            let mut output = Value::Null;
            for _ in 0..steps {
                let result = dynamic.execute(&branch, &inputs, 60.0, "").unwrap();
                let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
                assert_eq!(
                    result["cooling_sat_reset_request"],
                    fast["cooling_sat_reset_request"]
                );
                output = result["cooling_sat_reset_request"].clone();
            }
            output
        };
        assert_eq!(evaluate(24.0, 24.0, 0.0, 2), json!(0));
        assert_eq!(evaluate(27.5, 24.0, 0.85, 4), json!(3));
        assert_eq!(evaluate(26.5, 24.0, 0.85, 3), json!(2));
        assert_eq!(evaluate(24.0, 24.0, 0.85, 3), json!(1));
        assert_eq!(evaluate(24.0, 24.0, 0.79, 3), json!(1));
        assert_eq!(evaluate(24.0, 24.0, 0.70, 3), json!(0));
        assert_eq!(evaluate(27.5, 23.0, 0.0, 2), json!(0));
        assert_eq!(evaluate(27.5, 23.0, 0.0, 35), json!(3));
        // A 2.2 K error yields 1188 s, so Level 2 returns before the old
        // fixed 1800 s suppression would have elapsed.
        assert_eq!(evaluate(24.2, 22.0, 0.0, 2), json!(0));
        assert_eq!(evaluate(24.2, 22.0, 0.0, 15), json!(0));
        assert_eq!(evaluate(24.2, 22.0, 0.0, 8), json!(2));
    }
    #[test]
    fn periodic_sampler_holds_between_events_in_both_executors() {
        let graph: Block = serde_json::from_value(json!({
            "name":"SampledInput","block_type":"Composite","category":"composite",
            "inputs":[{"name":"u","type":"Real"}],
            "outputs":[{"name":"y","type":"Real"}],
            "blocks":[{"name":"sampled","block_type":"Buildings.Controls.OBC.CDL.Discrete.Sampler",
                "inputs":[{"name":"u","type":"Real"}],"outputs":[{"name":"y","type":"Real"}],
                "parameters":[{"name":"samplePeriod","value":120.0}]}],
            "connections":[{"from":"u","to":"sampled.u"},{"from":"sampled.y","to":"y"}]
        }))
        .unwrap();
        assert!(validate(&graph).is_empty());
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
        // OBC samples at t=0, 120, and 240 seconds.
        for (input, expected) in [(1.0, 1.0), (2.0, 1.0), (3.0, 3.0), (4.0, 3.0), (5.0, 5.0)] {
            let values = HashMap::from([("u".into(), json!(input))]);
            assert_eq!(
                dynamic.execute(&graph, &values, 60.0, "").unwrap()["y"],
                json!(expected)
            );
            assert_eq!(
                compiled.execute(&values, 60.0, false).unwrap().0["y"],
                json!(expected)
            );
        }
    }
    #[test]
    fn sample_trigger_uses_absolute_phase_in_both_executors() {
        for (period, shift, step, expected) in [
            (120.0, 0.0, 60.0, vec![true, false, true, false, true]),
            (
                60.0,
                30.0,
                15.0,
                vec![false, false, true, false, false, false, true],
            ),
        ] {
            let graph: Block = serde_json::from_value(json!({
                "name":"clock","block_type":"SampleTrigger",
                "outputs":[{"name":"y","type":"Boolean"}],
                "parameters":[{"name":"period","value":period},{"name":"shift","value":shift}]
            }))
            .unwrap();
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(&graph).unwrap().unwrap();
            for (tick, pulse) in expected.into_iter().enumerate() {
                let input = HashMap::new();
                let ordinary = dynamic.execute(&graph, &input, step, "").unwrap();
                let fast = compiled.execute(&input, step, false).unwrap().0;
                assert_eq!(
                    ordinary["y"],
                    json!(pulse),
                    "dynamic clock at t={}",
                    tick as f64 * step
                );
                assert_eq!(
                    fast["y"],
                    json!(pulse),
                    "compiled clock at t={}",
                    tick as f64 * step
                );
            }
        }
    }
    fn assert_g36_sat_suppression_trace(source: &str, start: f64, step_millis: usize) {
        let series = |name: &str| -> Vec<f64> {
            let line = source.lines().find(|line| line.starts_with(name)).unwrap();
            line.split_once('=')
                .unwrap()
                .1
                .trim_matches(['[', ']'])
                .split(',')
                .map(|field| field.trim().parse::<f64>().unwrap())
                .collect()
        };
        let expected_after = series("timSupCooReq.yAftSup=");
        let expected_duration = series("timSupCooReq.supTim.y=");
        assert_eq!(expected_after.len(), 101);
        let graph: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["name"] == "zone_01")
            .unwrap();
        let mut branch = json!({
            "name":"CoolingSuppressionReference","block_type":"Composite","category":"composite",
            "inputs":[{"name":"room_temp","type":"Real"},{"name":"cool_setpoint","type":"Real"},
                      {"name":"cooling_loop","type":"Real"}],
            "outputs":[{"name":"after","type":"Boolean"},{"name":"duration","type":"Real"}],
            "blocks":zone["blocks"].as_array().unwrap().iter()
                .filter(|block| block["name"].as_str().unwrap().starts_with("sat_"))
                .cloned().collect::<Vec<_>>(),
            "connections":zone["connections"].as_array().unwrap().iter()
                .filter(|edge| edge["to"].as_str().unwrap().starts_with("sat_"))
                .map(|edge| {
                    let mut copy=edge.clone();
                    copy["from"]=json!(match edge["from"].as_str().unwrap() {
                        "window_cool_target.y"=>"cool_setpoint",
                        "cool_pi.y"=>"cooling_loop",
                        other=>other,
                    });
                    copy
                }).collect::<Vec<_>>()
        });
        branch["connections"].as_array_mut().unwrap().extend([
            json!({"from":"sat_suppression_complete.y","to":"after"}),
            json!({"from":"sat_suppression_duration.y","to":"duration"}),
        ]);
        let branch: Block = serde_json::from_value(branch).unwrap();
        assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        let mut actual_after = Vec::new();
        let mut actual_duration = Vec::new();
        let steps = ((7200.0 - start) * 1000.0 / step_millis as f64).round() as usize;
        let sample_every = steps / 100;
        assert_eq!(sample_every * 100, steps);
        let dt = step_millis as f64 / 1000.0;
        for second in 0..=steps {
            let time = start + second as f64 * dt;
            // Source validation model: 22 °C to 27 °C ramp after 900 s,
            // and a 25 °C zone with a 2 K, 7200-second sinusoid.
            let setpoint = 22.0 + (5.0 * (time - 900.0) / 600.0).clamp(0.0, 5.0);
            let zone_temp = 25.0 + 2.0 * (2.0 * std::f64::consts::PI * time / 7200.0).sin();
            let inputs = HashMap::from([
                ("cool_setpoint".into(), json!(setpoint)),
                ("room_temp".into(), json!(zone_temp)),
                ("cooling_loop".into(), json!(0.0)),
            ]);
            let elapsed = if second == 0 { 0.0 } else { dt };
            let ordinary = dynamic.execute(&branch, &inputs, elapsed, "").unwrap();
            let fast = compiled.execute(&inputs, elapsed, false).unwrap().0;
            assert_eq!(ordinary["after"], fast["after"], "second {second}");
            assert_eq!(ordinary["duration"], fast["duration"], "second {second}");
            if second % sample_every == 0 {
                actual_after.push(if ordinary["after"].as_bool().unwrap() {
                    1.0
                } else {
                    0.0
                });
                actual_duration.push(ordinary["duration"].as_f64().unwrap());
            }
        }
        let mismatches: Vec<_> = expected_after
            .iter()
            .zip(&actual_after)
            .enumerate()
            .filter(|(_, (expected, actual))| (*expected - **actual).abs() > 0.0)
            .map(|(index, _)| index)
            .collect();
        assert!(
            mismatches.is_empty(),
            "OBC Boolean trace differs at samples {mismatches:?}"
        );
        let duration_mismatches: Vec<_> = expected_duration
            .iter()
            .zip(&actual_duration)
            .enumerate()
            .filter(|(_, (expected, actual))| (*expected - **actual).abs() > 1e-6)
            .map(|(index, _)| index)
            .collect();
        assert!(
            duration_mismatches.is_empty(),
            "OBC suppression duration differs at samples {duration_mismatches:?}"
        );
        // The archived OBC trace ends after one ramp. Extend the same branch
        // with two later changes to verify that a change during suppression
        // cannot replace the captured duration, while a later change can.
        let mut repeated = HashMap::new();
        for step in 1..=((1800.0 / dt).round() as usize) {
            let time = 7200.0 + step as f64 * dt;
            let setpoint = if time < 7300.0 {
                27.0
            } else if time < 7480.0 {
                24.0
            } else if time < 8200.0 {
                23.0
            } else {
                26.0
            };
            let inputs = HashMap::from([
                ("cool_setpoint".into(), json!(setpoint)),
                ("room_temp".into(), json!(25.0)),
                ("cooling_loop".into(), json!(0.0)),
            ]);
            let ordinary = dynamic.execute(&branch, &inputs, dt, "").unwrap();
            let fast = compiled.execute(&inputs, dt, false).unwrap().0;
            assert_eq!(ordinary["after"], fast["after"], "time {time}");
            assert_eq!(ordinary["duration"], fast["duration"], "time {time}");
            for target in [7440, 7680, 8040, 8400, 8940] {
                if (time - target as f64).abs() < dt / 2.0 {
                    repeated.insert(
                        target,
                        (
                            ordinary["after"].as_bool().unwrap(),
                            ordinary["duration"].as_f64().unwrap(),
                        ),
                    );
                }
            }
        }
        for target in [7440, 7680, 8400] {
            assert_eq!(repeated[&target].0, false, "time {target}");
            assert!((repeated[&target].1 - 540.0).abs() < 1e-6, "time {target}");
        }
        for target in [8040, 8940] {
            assert_eq!(repeated[&target].0, true, "time {target}");
        }
    }
    #[test]
    fn g36_sat_suppression_follows_pinned_obc_positive_start_trace() {
        assert_g36_sat_suppression_trace(
            include_str!("../../verification/g36_time_suppression_positive_dymola.txt"),
            0.0,
            1000,
        );
    }
    #[test]
    fn g36_sat_suppression_follows_pinned_obc_negative_start_trace() {
        assert_g36_sat_suppression_trace(
            include_str!("../../verification/g36_time_suppression_negative_dymola.txt"),
            -360.0,
            600,
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
            include_str!("../../web/public/examples/obc_zone_status.json"),
            include_str!("../../web/public/examples/obc_operation_mode.json"),
            include_str!("../../web/public/examples/obc_group_status.json"),
            include_str!("../../web/public/examples/obc_optimal_start_calculation.json"),
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
    fn obc_zone_status_unrecorded_outputs_follow_temperature_and_window() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/public/examples/obc_zone_status.json"
        ))
        .unwrap();
        for (zone_k, window_closed, expected) in [
            (280.0, true, (false, false, true, 1800.0)),
            (310.0, true, (true, true, false, 1800.0)),
            (280.0, false, (false, true, true, 0.0)),
        ] {
            let inputs = HashMap::from([
                ("cooDowTim".into(), json!(1800.0)),
                ("warUpTim".into(), json!(1800.0)),
                ("TZon".into(), json!(zone_k)),
                ("TOccHeaSet".into(), json!(293.15)),
                ("TOccCooSet".into(), json!(297.15)),
                ("TUnoHeaSet".into(), json!(285.15)),
                ("TUnoCooSet".into(), json!(303.15)),
                ("u1Win".into(), json!(window_closed)),
            ]);
            let dynamic = Executor::default()
                .execute(&graph, &inputs, 864.0, "")
                .unwrap();
            let compiled = CompiledExecutor::try_new(&graph)
                .unwrap()
                .unwrap()
                .execute(&inputs, 864.0, false)
                .unwrap()
                .0;
            for actual in [&dynamic, &compiled] {
                assert_eq!(actual["yHigUnoCoo"], json!(expected.0));
                assert_eq!(actual["yEndSetBac"], json!(expected.1));
                assert_eq!(actual["yEndSetUp"], json!(expected.2));
                assert_eq!(actual["yCooTim"], json!(expected.3));
                assert_eq!(actual["yWarTim"], json!(expected.3));
            }
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
        let graph = include_str!("../../web/tests/fixtures/g36_pre_obc_optimal_start.json");
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
        for (index, (a, b)) in fast["rows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(dynamic["rows"].as_array().unwrap())
            .enumerate()
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
                    "step {index} {point}: compiled={}, dynamic={}",
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
    fn conventional_return_damper_command_changes_physical_air_delivery() {
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
        let mut base = Simulation::new(graph, &weather, "{}").unwrap();
        let normal: Value = serde_json::from_str(&base.step_chunk(3).unwrap()).unwrap();
        let mut edited: Value = serde_json::from_str(graph).unwrap();
        for ahu in edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|block| block["block_type"] == "AHUController")
        {
            ahu["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "conventional_return_one")
                .unwrap()["parameters"][0]["value"] = json!(0.2);
        }
        let mut changed = Simulation::new(&edited.to_string(), &weather, "{}").unwrap();
        let restricted: Value = serde_json::from_str(&changed.step_chunk(3).unwrap()).unwrap();
        assert_eq!(
            normal["rows"][2]["ahu_1_actual_return_damper_position"],
            json!(0.8)
        );
        assert_eq!(
            restricted["rows"][2]["ahu_1_actual_return_damper_position"],
            json!(0.0)
        );
        assert!(
            restricted["rows"][2]["delivered_airflow"].as_f64().unwrap()
                < normal["rows"][2]["delivered_airflow"].as_f64().unwrap()
        );
        assert!(
            restricted["rows"][2]["ahu_1_outdoor_air_fraction"]
                .as_f64()
                .unwrap()
                > normal["rows"][2]["ahu_1_outdoor_air_fraction"]
                    .as_f64()
                    .unwrap()
        );

        edited["outputs"]
            .as_array_mut()
            .unwrap()
            .retain(|port| port["name"] != "ahu_1_return_air_damper_position");
        edited["connections"]
            .as_array_mut()
            .unwrap()
            .retain(|edge| edge["to"] != "ahu_1_return_air_damper_position");
        let missing: Block = serde_json::from_value(edited).unwrap();
        assert!(conventional_control_contract(&missing)
            .iter()
            .any(|error| error.contains("ahu_1_return_air_damper_position")));
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
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(27.0)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_air_setpoint".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("zone_presence_raw".into(), json!(true)),
            ("occupancy_count".into(), json!(8.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
            ("measured_airflow".into(), json!(0.0)),
            ("discharge_temp".into(), json!(40.0)),
        ]);
        for step in 0..36 {
            let outputs = executor.execute(zone, &inputs, 60.0, "").unwrap();
            // OBC TrueDelay qualifies a fan that was already running at initialization.
            assert_eq!(outputs["low_airflow_alarm"], json!(step >= 5));
            assert_eq!(outputs["high_discharge_alarm"], json!(step >= 5));
        }
        inputs.insert("measured_airflow".into(), json!(1.0));
        inputs.insert("discharge_temp".into(), json!(30.0));
        let recovered = executor.execute(zone, &inputs, 60.0, "").unwrap();
        assert_eq!(recovered["low_airflow_alarm"], false);
        assert_eq!(recovered["high_discharge_alarm"], false);
        let mut disabled = zone.clone();
        disabled
            .blocks
            .iter_mut()
            .find(|block| block.name == "pressure_importance")
            .unwrap()
            .parameters[0]
            .value = json!(0.0);
        inputs.insert("measured_airflow".into(), json!(0.0));
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&disabled).unwrap().unwrap();
        for _ in 0..36 {
            let result = dynamic.execute(&disabled, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            for point in [
                "pressure_reset_request",
                "low_airflow_alarm_level",
                "low_airflow_alarm",
            ] {
                assert_eq!(result[point], fast[point]);
                assert_eq!(
                    result[point],
                    if point == "low_airflow_alarm" {
                        json!(false)
                    } else {
                        json!(0)
                    },
                    "{point}"
                );
            }
        }
    }
    #[test]
    fn g36_minimum_discharge_reheat_requires_occupied_cdl_mode() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut inputs = HashMap::from([
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(22.0)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(8.0)),
            ("supply_air_setpoint".into(), json!(8.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("discharge_temp".into(), json!(8.0)),
            ("scheduled_occupied".into(), json!(false)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("zone_presence_raw".into(), json!(false)),
            ("occupancy_count".into(), json!(0.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("measured_airflow".into(), json!(0.95)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
        ]);
        for (occupied, expect_open) in [(false, false), (true, true)] {
            inputs.insert("scheduled_occupied".into(), json!(occupied));
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
            let first = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let first_fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(first["reheat_valve_position"], json!(0.0));
            assert_eq!(first_fast["reheat_valve_position"], json!(0.0));
            let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            for result in [&slow, &fast] {
                let valve = result["reheat_valve_position"].as_f64().unwrap();
                assert_eq!(
                    valve > 0.0,
                    expect_open,
                    "occupied={occupied}, valve={valve}"
                );
            }
        }
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
                ("flow_override_code".into(), json!(0)),
                ("room_temp".into(), json!(room)),
                ("co2_ppm".into(), json!(420.0)),
                ("supply_air_temp".into(), json!(16.0)),
                ("supply_air_setpoint".into(), json!(16.0)),
                ("supply_pressure".into(), json!(1.0)),
                ("scheduled_occupied".into(), json!(schedule)),
                ("group_warmup".into(), json!(warmup)),
                ("group_cooldown".into(), json!(cooldown)),
                ("group_setback".into(), json!(setback)),
                ("group_setup".into(), json!(setup)),
                ("group_freeze_setback".into(), json!(freeze)),
                ("window_closed".into(), json!(true)),
                ("local_override".into(), json!(false)),
                ("supply_fan_proven".into(), json!(false)),
                ("actual_fan_speed".into(), json!(0.0)),
                ("actual_outdoor_damper_position".into(), json!(0.2)),
                ("actual_return_damper_position".into(), json!(0.8)),
                ("zone_presence_raw".into(), json!(presence)),
                (
                    "occupancy_count".into(),
                    json!(if presence { 8.0 } else { 0.0 }),
                ),
                ("actual_reheat_valve_position".into(), json!(0.0)),
                ("hot_water_plant_proven".into(), json!(false)),
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
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(22.0)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_air_setpoint".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(false)),
            ("group_warmup".into(), json!(true)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("supply_fan_proven".into(), json!(false)),
            ("actual_fan_speed".into(), json!(0.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("zone_presence_raw".into(), json!(false)),
            ("occupancy_count".into(), json!(0.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
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
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
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
                assert_eq!(dynamic[name].as_bool(), compiled[name].as_bool());
            }
            dynamic
        };
        let warm = check(group, &inputs);
        assert_eq!(warm["warmup"], true);
        assert_eq!(warm["cooldown"], false);
        let mut status_tuned = group.clone();
        for zone in 1..=5 {
            let status = status_tuned
                .blocks
                .iter_mut()
                .find(|block| block.name == format!("zone_status_{zone}"))
                .unwrap();
            let hysteresis = status
                .blocks
                .iter_mut()
                .find(|block| block.name == "hys")
                .unwrap();
            hysteresis
                .parameters
                .iter_mut()
                .find(|parameter| parameter.name == "uHigh")
                .unwrap()
                .value = json!(50.0);
        }
        assert_eq!(check(&status_tuned, &inputs)["warmup"], false);
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
        let mode = tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == "obc_operation_mode")
            .unwrap();
        mode.blocks
            .iter_mut()
            .find(|block| block.name == "maxWarCooTime")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(600.0);
        inputs.insert("lookahead_allowed".into(), json!(true));
        assert_eq!(check(&tuned, &inputs)["preconditioning"], false);
    }
    #[test]
    fn g36_optimal_start_learns_from_measured_zone_recovery_in_both_executors() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
        .unwrap();
        let mut group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap()
            .clone();
        // Advance a configurable daily clock after the 30-minute recovery in this short trace.
        group
            .blocks
            .iter_mut()
            .find(|block| block.name == "optimal_daily_sample")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "period")
            .unwrap()
            .value = json!(2100.0);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&group).unwrap().unwrap();
        let mut inputs = HashMap::from([
            ("seconds_until_occupied".into(), json!(3500.0)),
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
        let mut step = |inputs: &HashMap<String, Value>| {
            let reference = dynamic.execute(&group, inputs, 60.0, "").unwrap();
            let optimized = compiled.execute(inputs, 60.0, false).unwrap().0;
            assert_eq!(reference["warmup"].as_bool(), optimized["warmup"].as_bool());
            reference["warmup"].as_bool().unwrap()
        };
        assert!(step(&inputs));
        for minute in 1..=36 {
            if minute == 10 {
                inputs.insert("scheduled_occupied".into(), json!(true));
            }
            inputs.insert(
                "seconds_until_occupied".into(),
                json!(if minute < 10 {
                    3500.0 - minute as f64 * 60.0
                } else {
                    86400.0
                }),
            );
            for zone in 1..=5 {
                inputs.insert(
                    format!("room_{zone}"),
                    json!(19.0 + 2.0 * (minute.min(30)) as f64 / 30.0),
                );
            }
            step(&inputs);
        }
        inputs.insert("seconds_until_occupied".into(), json!(86400.0));
        inputs.insert("lookahead_allowed".into(), json!(false));
        inputs.insert("scheduled_occupied".into(), json!(false));
        assert!(!step(&inputs));
        inputs.insert("seconds_until_occupied".into(), json!(3500.0));
        inputs.insert("lookahead_allowed".into(), json!(true));
        for zone in 1..=5 {
            inputs.insert(format!("room_{zone}"), json!(19.0));
        }
        assert!(
            !step(&inputs),
            "the measured faster recovery must shorten the next day's start"
        );
    }
    #[test]
    fn g36_recovery_observation_requires_an_optimal_start_episode() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
        .unwrap();
        let mut group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap()
            .clone();
        group
            .blocks
            .iter_mut()
            .find(|block| block.name == "optimal_warmup_horizon_hold")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "trueHoldDuration")
            .unwrap()
            .value = json!(180.0);
        let trace = "zone_group_1|optimal_warmup_active.y".to_string();
        let mut dynamic = Executor::default();
        dynamic.trace_targets = vec![trace.clone()];
        let mut compiled = CompiledExecutor::try_new(&group).unwrap().unwrap();
        compiled.set_traces(&[trace.clone()]).unwrap();
        let mut inputs = HashMap::from([
            ("seconds_until_occupied".into(), json!(3500.0)),
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
        let mut ordinary_inputs = inputs.clone();
        ordinary_inputs.insert("scheduled_occupied".into(), json!(true));
        ordinary_inputs.insert("seconds_until_occupied".into(), json!(86400.0));
        ordinary_inputs.insert("lookahead_allowed".into(), json!(false));
        for zone in 1..=5 {
            ordinary_inputs.insert(format!("room_{zone}"), json!(22.0));
        }
        let mut ordinary = Executor::default();
        ordinary.trace_targets = vec![trace.clone()];
        ordinary
            .execute(&group, &ordinary_inputs, 60.0, "")
            .unwrap();
        assert_eq!(
            ordinary.trace_values[&trace],
            json!(false),
            "ordinary occupied operation must not teach recovery"
        );
        let mut observed = |inputs: &HashMap<String, Value>| {
            dynamic.execute(&group, inputs, 60.0, "").unwrap();
            let (_, traces) = compiled.execute(inputs, 60.0, true).unwrap();
            let reference = dynamic.trace_values[&trace].as_bool().unwrap();
            assert_eq!(reference, traces[&trace].as_f64().unwrap() != 0.0);
            reference
        };
        assert!(observed(&inputs));
        inputs.insert("scheduled_occupied".into(), json!(true));
        inputs.insert("seconds_until_occupied".into(), json!(86400.0));
        for zone in 1..=5 {
            inputs.insert(format!("room_{zone}"), json!(21.0));
        }
        assert!(
            observed(&inputs),
            "the episode must continue into occupancy"
        );
        inputs.insert("scheduled_occupied".into(), json!(false));
        inputs.insert("lookahead_allowed".into(), json!(false));
        assert!(
            observed(&inputs),
            "the episode must survive the input transition until the hold expires"
        );
        assert!(
            !observed(&inputs),
            "the episode must clear when the hold expires"
        );
    }
    #[test]
    fn g36_optimal_start_uses_longest_closed_window_zone_prediction() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
        .unwrap();
        let mut group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap()
            .clone();
        group
            .blocks
            .iter_mut()
            .find(|block| block.name == "optimal_daily_sample")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "period")
            .unwrap()
            .value = json!(3900.0);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&group).unwrap().unwrap();
        let mut inputs = HashMap::from([
            ("seconds_until_occupied".into(), json!(3500.0)),
            ("lookahead_allowed".into(), json!(true)),
            ("scheduled_occupied".into(), json!(false)),
        ]);
        for zone in 1..=5 {
            inputs.insert(
                format!("room_{zone}"),
                json!(if zone <= 2 { 19.0 } else { 21.0 }),
            );
            inputs.insert(format!("heat_target_{zone}"), json!(21.0));
            inputs.insert(format!("cool_target_{zone}"), json!(24.0));
            inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
            inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
            inputs.insert(format!("window_closed_{zone}"), json!(true));
            inputs.insert(format!("override_{zone}"), json!(false));
        }
        let mut step = |inputs: &HashMap<String, Value>| {
            let reference = dynamic.execute(&group, inputs, 60.0, "").unwrap();
            let optimized = compiled.execute(inputs, 60.0, false).unwrap().0;
            assert_eq!(reference["warmup"].as_bool(), optimized["warmup"].as_bool());
            reference["warmup"].as_bool().unwrap()
        };
        assert!(step(&inputs));
        for minute in 1..=60 {
            inputs.insert(
                "seconds_until_occupied".into(),
                json!(100.0_f64.max(3500.0 - minute as f64 * 60.0)),
            );
            inputs.insert(
                "room_1".into(),
                json!(19.0 + 2.0 * minute.min(30) as f64 / 30.0),
            );
            inputs.insert("room_2".into(), json!(19.0 + 2.0 * minute as f64 / 60.0));
            step(&inputs);
        }
        for _ in 0..5 {
            step(&inputs);
        }
        inputs.insert("seconds_until_occupied".into(), json!(86400.0));
        inputs.insert("lookahead_allowed".into(), json!(false));
        assert!(!step(&inputs));
        inputs.insert("seconds_until_occupied".into(), json!(3500.0));
        inputs.insert("lookahead_allowed".into(), json!(true));
        inputs.insert("room_1".into(), json!(21.0));
        inputs.insert("room_2".into(), json!(19.0));
        assert!(
            step(&inputs),
            "the slower zone should set the group start time"
        );
        inputs.insert("window_closed_2".into(), json!(false));
        assert!(
            !step(&inputs),
            "an open window should remove the slow zone's request"
        );
        inputs.insert("window_closed_2".into(), json!(true));
        assert!(
            step(&inputs),
            "closing the window should resample that zone's need"
        );
    }
    #[test]
    fn g36_recovery_learner_records_one_rate_per_active_episode() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
        .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let learner = group
            .blocks
            .iter()
            .find(|block| block.name == "optimal_warmup_1_learner")
            .unwrap();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(learner).unwrap().unwrap();
        let mut inputs = HashMap::from([
            ("error".into(), json!(2.0)),
            ("active".into(), json!(true)),
            ("window_closed".into(), json!(true)),
            ("fallback_rate".into(), json!(1.0 / 3600.0)),
            ("midnight".into(), json!(false)),
            ("sampled_need".into(), json!(2.0)),
        ]);
        let mut step = |values: &HashMap<String, Value>| {
            let reference = dynamic.execute(learner, values, 60.0, "").unwrap()["rate"]
                .as_f64()
                .unwrap();
            let optimized = compiled.execute(values, 60.0, false).unwrap().0["rate"]
                .as_f64()
                .unwrap();
            assert!((reference - optimized).abs() < 1e-12);
            reference
        };
        step(&inputs);
        for minute in 1..=30 {
            inputs.insert("error".into(), json!(2.0 * (1.0 - minute as f64 / 30.0)));
            step(&inputs);
        }
        assert_eq!(step(&inputs), 1.0 / 3600.0);
        inputs.insert("midnight".into(), json!(true));
        let learned = step(&inputs);
        inputs.insert("midnight".into(), json!(false));
        assert!(learned > 1.0 / 3600.0);
        inputs.insert("error".into(), json!(0.2));
        assert_eq!(step(&inputs), learned);
        inputs.insert("error".into(), json!(0.0));
        assert_eq!(
            step(&inputs),
            learned,
            "re-crossing the setpoint must not add a second episode sample"
        );
        inputs.insert("midnight".into(), json!(true));
        inputs.insert("sampled_need".into(), json!(0.0));
        assert_eq!(
            step(&inputs),
            learned,
            "no sampled need must suppress the next daily sample"
        );
        inputs.insert("midnight".into(), json!(false));
        step(&inputs);
        inputs.insert("sampled_need".into(), json!(2.0));
        inputs.insert("midnight".into(), json!(true));
        assert!(
            step(&inputs) > learned,
            "OBC's positive sampled need can reuse the latest held recovery rate"
        );
    }
    #[test]
    fn g36_daily_recovery_sample_uses_start_need_and_window_state() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
        .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let learner = group
            .blocks
            .iter()
            .find(|block| block.name == "optimal_warmup_1_learner")
            .unwrap();
        let trace = format!("{}|daily_sample.y", learner.name);
        let mut dynamic = Executor::default();
        dynamic.trace_targets = vec![trace.clone()];
        let mut compiled = CompiledExecutor::try_new(learner).unwrap().unwrap();
        compiled.set_traces(&[trace.clone()]).unwrap();
        let mut inputs = HashMap::from([
            ("error".into(), json!(2.0)),
            ("active".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("fallback_rate".into(), json!(1.0 / 3600.0)),
            ("midnight".into(), json!(false)),
            ("sampled_need".into(), json!(0.0)),
        ]);
        for (midnight, need, window_closed, expected) in [
            (false, 0.0, true, false),
            (true, 0.0, true, false),
            (false, 2.0, false, false),
            (true, 2.0, false, false),
            (false, 2.0, true, false),
            (true, 2.0, true, true),
        ] {
            inputs.insert("midnight".into(), json!(midnight));
            inputs.insert("sampled_need".into(), json!(need));
            inputs.insert("window_closed".into(), json!(window_closed));
            dynamic.execute(learner, &inputs, 60.0, "").unwrap();
            let (_, optimized) = compiled.execute(&inputs, 60.0, true).unwrap();
            assert_eq!(dynamic.trace_values[&trace].as_bool(), Some(expected));
            assert_eq!(
                optimized[&trace].as_f64(),
                Some(if expected { 1.0 } else { 0.0 })
            );
        }
    }
    #[test]
    fn g36_recovery_slope_keeps_the_obc_minimum_in_both_executors() {
        // OBC OptimalStartCalculation uses Max(sampled slope, 1/3600 K/s).
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
        .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let learner = group
            .blocks
            .iter()
            .find(|block| block.name == "optimal_warmup_1_learner")
            .unwrap();
        let fallback = 1.0 / 3600.0;
        let mut dynamic = Executor::default();
        let raw_rate_trace = format!("{}|rate_mean.y", learner.name);
        dynamic.trace_targets = vec![raw_rate_trace.clone()];
        let mut compiled = CompiledExecutor::try_new(learner).unwrap().unwrap();
        let mut inputs = HashMap::from([
            ("error".into(), json!(2.0)),
            ("active".into(), json!(true)),
            ("window_closed".into(), json!(true)),
            ("fallback_rate".into(), json!(fallback)),
            ("midnight".into(), json!(false)),
            ("sampled_need".into(), json!(2.0)),
        ]);
        for minute in 0..=180 {
            inputs.insert("error".into(), json!(2.0 * (1.0 - minute as f64 / 180.0)));
            let actual = dynamic.execute(learner, &inputs, 60.0, "").unwrap()["rate"]
                .as_f64()
                .unwrap();
            let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0["rate"]
                .as_f64()
                .unwrap();
            assert!((actual - optimized).abs() < 1e-12);
            assert!(
                (actual - fallback).abs() < 1e-12,
                "a slower recovery cannot lower the OBC minimum slope"
            );
        }
        inputs.insert("midnight".into(), json!(true));
        let actual = dynamic.execute(learner, &inputs, 60.0, "").unwrap()["rate"]
            .as_f64()
            .unwrap();
        let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0["rate"]
            .as_f64()
            .unwrap();
        assert!((actual - optimized).abs() < 1e-12);
        assert!((actual - fallback).abs() < 1e-12);
        assert!(
            dynamic.trace_values[&raw_rate_trace].as_f64().unwrap() < fallback,
            "the measured mean must actually be slower than the fallback"
        );
    }
    #[test]
    fn g36_recovery_learner_does_not_teach_from_timer_expiry() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
        .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let learner = group
            .blocks
            .iter()
            .find(|block| block.name == "optimal_warmup_1_learner")
            .unwrap();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(learner).unwrap().unwrap();
        let mut inputs = HashMap::from([
            ("error".into(), json!(2.0)),
            ("active".into(), json!(true)),
            ("window_closed".into(), json!(true)),
            ("fallback_rate".into(), json!(1.0 / 36000.0)),
            ("midnight".into(), json!(false)),
            ("sampled_need".into(), json!(2.0)),
        ]);
        for minute in 0..=185 {
            let reference = dynamic.execute(learner, &inputs, 60.0, "").unwrap()["rate"]
                .as_f64()
                .unwrap();
            let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0["rate"]
                .as_f64()
                .unwrap();
            assert!((reference - optimized).abs() < 1e-12);
            assert!(
                (reference - 1.0 / 36000.0).abs() < 1e-12,
                "minute {minute}: timer expiry without setpoint attainment must not teach"
            );
        }
        inputs.insert("error".into(), json!(0.0));
        let reference = dynamic.execute(learner, &inputs, 60.0, "").unwrap()["rate"]
            .as_f64()
            .unwrap();
        let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0["rate"]
            .as_f64()
            .unwrap();
        assert!((reference - optimized).abs() < 1e-12);
        assert!(
            (reference - 1.0 / 36000.0).abs() < 1e-12,
            "attainment waits for the daily sample"
        );
        inputs.insert("midnight".into(), json!(true));
        let reference = dynamic.execute(learner, &inputs, 60.0, "").unwrap()["rate"]
            .as_f64()
            .unwrap();
        let optimized = compiled.execute(&inputs, 60.0, false).unwrap().0["rate"]
            .as_f64()
            .unwrap();
        assert!((reference - optimized).abs() < 1e-12);
        assert!(
            reference > 1.0 / 36000.0,
            "actual attainment within fourteen hours should teach on the daily event"
        );
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
        for (room, setback, setup, freeze, alarm, mode) in [
            (19.0, false, false, false, 0.0, 7),
            (17.0, true, false, false, 0.0, 5),
            (18.5, true, false, false, 0.0, 5),
            (19.2, false, false, false, 0.0, 7),
            (33.0, false, true, false, 0.0, 3),
            (31.5, false, true, false, 0.0, 3),
            (30.8, false, false, false, 0.0, 7),
            (3.0, false, false, true, 3.0, 6),
            (6.0, false, false, true, 3.0, 6),
            (7.2, true, false, false, 0.0, 5),
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
                assert_eq!(
                    result[name],
                    json!(expected),
                    "{name} at {room} °C; OBC mode {}",
                    result["operation_mode"]
                );
                assert_eq!(fast[name].as_bool(), Some(expected));
            }
            assert_eq!(result["freeze_alarm_level"].as_f64(), Some(alarm));
            assert_eq!(fast["freeze_alarm_level"].as_f64(), Some(alarm));
            assert_eq!(result["operation_mode"].as_f64(), Some(mode as f64));
            assert_eq!(fast["operation_mode"].as_f64(), Some(mode as f64));
        }
    }
    #[test]
    fn g36_mode_priority_resolves_simultaneous_recovery_requests() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
        .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_group_1")
            .unwrap();
        let mut inputs = HashMap::from([
            ("seconds_until_occupied".into(), json!(3500.0)),
            ("lookahead_allowed".into(), json!(true)),
            ("scheduled_occupied".into(), json!(false)),
        ]);
        for zone in 1..=5 {
            inputs.insert(
                format!("room_{zone}"),
                json!(if zone == 1 {
                    15.0
                } else if zone == 2 {
                    29.0
                } else {
                    22.0
                }),
            );
            inputs.insert(format!("heat_target_{zone}"), json!(21.0));
            inputs.insert(format!("cool_target_{zone}"), json!(24.0));
            inputs.insert(format!("unoccupied_heat_{zone}"), json!(18.0));
            inputs.insert(format!("unoccupied_cool_{zone}"), json!(32.0));
            inputs.insert(format!("window_closed_{zone}"), json!(true));
            inputs.insert(format!("override_{zone}"), json!(false));
        }
        let raw_trace = "zone_group_1|obc_operation_mode.yOpeMod".to_string();
        let mut dynamic = Executor::default();
        dynamic.trace_targets = vec![raw_trace.clone()];
        let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
        compiled.set_traces(&[raw_trace.clone()]).unwrap();
        let result = dynamic.execute(group, &inputs, 60.0, "").unwrap();
        let (fast, traces) = compiled.execute(&inputs, 60.0, true).unwrap();
        assert_eq!(
            dynamic.trace_values[&raw_trace].as_f64(),
            Some(6.0),
            "the pinned OBC block adds simultaneous cool-down and warm-up codes"
        );
        assert_eq!(traces[&raw_trace].as_f64(), Some(6.0));
        for output in [&result, &fast] {
            assert_eq!(
                output["operation_mode"].as_f64(),
                Some(2.0),
                "the editable priority tree chooses one recovery mode"
            );
            assert_eq!(output["cooldown"], json!(true));
            assert_eq!(output["warmup"], json!(false));
        }
    }
    #[test]
    fn g36_window_contact_changes_group_and_terminal_cdl_decisions() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
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
        assert_eq!(compiled["warmup"], false);
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
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_air_setpoint".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("zone_presence_raw".into(), json!(true)),
            ("occupancy_count".into(), json!(8.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
            ("measured_airflow".into(), json!(0.0)),
            ("discharge_temp".into(), json!(16.0)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("supply_fan_proven".into(), json!(false)),
            ("actual_fan_speed".into(), json!(0.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
        ]);
        let trace = "zone_01|window_primary_minimum.y".to_string();
        let mut dynamic = Executor::default();
        dynamic.trace_targets = vec![trace.clone()];
        let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
        compiled.set_traces(&[trace.clone()]).unwrap();
        for _ in 0..5 {
            dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            compiled.execute(&inputs, 60.0, true).unwrap();
        }
        let closed = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let closed_minimum = dynamic.trace_values[&trace].as_f64().unwrap();
        let (_, closed_traces) = compiled.execute(&inputs, 60.0, true).unwrap();
        assert!(closed_minimum > 0.0);
        assert_eq!(closed_traces[&trace].as_f64(), Some(closed_minimum));
        inputs.insert("window_closed".into(), json!(false));
        let open = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let (_, open_traces) = compiled.execute(&inputs, 60.0, true).unwrap();
        assert!(closed["cooling_demand"].as_f64().unwrap() > 0.0);
        assert_eq!(open["cooling_demand"], 0.0);
        assert_eq!(dynamic.trace_values[&trace], json!(0.0));
        assert_eq!(open_traces[&trace].as_f64(), Some(0.0));
        inputs.insert("room_temp".into(), json!(3.0));
        inputs.insert("group_freeze_setback".into(), json!(true));
        inputs.insert("scheduled_occupied".into(), json!(false));
        let freeze = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        assert!(freeze["heating_demand"].as_f64().unwrap() > 0.0);
        assert!(freeze["airflow_request"].as_f64().unwrap() > 0.0);

        inputs.insert("room_temp".into(), json!(22.0));
        inputs.insert("group_freeze_setback".into(), json!(false));
        inputs.insert("scheduled_occupied".into(), json!(true));
        inputs.insert("zone_presence_raw".into(), json!(false));
        inputs.insert("occupancy_count".into(), json!(0.0));
        inputs.insert("window_closed".into(), json!(true));
        let mut no_standby = zone.clone();
        no_standby
            .blocks
            .iter_mut()
            .find(|block| block.name == "ventilation_standby_permitted")
            .unwrap()
            .parameters[0]
            .value = json!(false);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&no_standby).unwrap().unwrap();
        let standby_trace = "zone_01|window_primary_minimum.y".to_string();
        let heat_trace = "zone_01|heat_setpoint_active.y".to_string();
        let cool_trace = "zone_01|cool_setpoint_active.y".to_string();
        let traces = vec![
            standby_trace.clone(),
            heat_trace.clone(),
            cool_trace.clone(),
        ];
        dynamic.trace_targets = traces.clone();
        compiled.set_traces(&traces).unwrap();
        for _ in 0..3 {
            dynamic.execute(&no_standby, &inputs, 60.0, "").unwrap();
            compiled.execute(&inputs, 60.0, true).unwrap();
        }
        let reference = dynamic.execute(&no_standby, &inputs, 60.0, "").unwrap();
        let (fast, fast_traces) = compiled.execute(&inputs, 60.0, true).unwrap();
        let mut default_dynamic = Executor::default();
        default_dynamic.trace_targets = vec![heat_trace.clone(), cool_trace.clone()];
        let default_standby = default_dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        let mut default_compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
        default_compiled
            .set_traces(&[heat_trace.clone(), cool_trace.clone()])
            .unwrap();
        let (default_fast, default_traces) = default_compiled.execute(&inputs, 60.0, true).unwrap();
        for name in [
            "conditioning_state",
            "airflow_request",
            "damper_position",
            "ventilation_area",
            "ventilation_population",
        ] {
            assert_eq!(default_fast[name], default_standby[name]);
        }
        for trace in [&heat_trace, &cool_trace] {
            assert_eq!(default_traces[trace], default_dynamic.trace_values[trace]);
        }
        assert_eq!(default_standby["conditioning_state"], json!(2));
        assert_eq!(default_standby["airflow_request"], json!(0.0));
        assert_eq!(default_standby["damper_position"], json!(0.0));
        assert_eq!(default_dynamic.trace_values[&heat_trace], json!(20.5));
        assert_eq!(default_dynamic.trace_values[&cool_trace], json!(24.5));
        assert_eq!(reference["conditioning_state"], json!(1));
        assert!(reference["airflow_request"].as_f64().unwrap() > 0.0);
        assert!(reference["damper_position"].as_f64().unwrap() > 0.0);
        assert_eq!(dynamic.trace_values[&heat_trace], json!(21.0));
        assert_eq!(dynamic.trace_values[&cool_trace], json!(24.0));
        assert_eq!(reference["ventilation_area"], json!(0.03));
        assert_eq!(reference["ventilation_population"], json!(0.0));
        assert!(dynamic.trace_values[&standby_trace].as_f64().unwrap() > 0.0);
        assert_eq!(fast["ventilation_area"], 0.03);
        assert_eq!(fast["ventilation_population"], 0.0);
        assert_eq!(fast["conditioning_state"], 1.0);
        assert_eq!(fast["airflow_request"], reference["airflow_request"]);
        for trace in &traces {
            assert_eq!(fast_traces[trace], dynamic.trace_values[trace]);
        }
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
    fn open_window_air_exchange_changes_heat_and_co2_without_changing_cdl_contact() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 32.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: false,
            };
            35_040
        ])
        .unwrap();
        let contact = r#""window_openings":[{"zone":1,"start_hour":0,"duration_hours":2}]"#;
        let mut no_exchange = Simulation::new(
            graph,
            &weather,
            &format!(r#"{{{contact},"open_window_additional_infiltration_m3_s":0}}"#),
        )
        .unwrap();
        let mut with_exchange = Simulation::new(
            graph,
            &weather,
            &format!(r#"{{{contact},"open_window_additional_infiltration_m3_s":0.2}}"#),
        )
        .unwrap();
        no_exchange.temps = [29.0; 10];
        with_exchange.temps = [29.0; 10];
        no_exchange.co2_ppm = [1000.0; 10];
        with_exchange.co2_ppm = [1000.0; 10];
        let still = run_weather_intervals(&mut no_exchange, 1);
        let exchanged = run_weather_intervals(&mut with_exchange, 1);
        let still_row = &still["rows"][0];
        let exchanged_row = &exchanged["rows"][0];
        assert!(
            exchanged_row["zone_c"][0].as_f64().unwrap() > still_row["zone_c"][0].as_f64().unwrap()
        );
        assert!(
            exchanged_row["zone_co2_ppm"][0].as_f64().unwrap()
                < still_row["zone_co2_ppm"][0].as_f64().unwrap()
        );
        assert_eq!(exchanged_row["zone_c"][1], still_row["zone_c"][1]);
        assert_eq!(
            exchanged_row["zone_co2_ppm"][1],
            still_row["zone_co2_ppm"][1]
        );
        assert_eq!(
            exchanged_row["zone_airflow"][0],
            still_row["zone_airflow"][0]
        );
    }
    #[test]
    fn g36_local_override_and_window_alarm_are_cdl_decisions() {
        let graph: Block = serde_json::from_str(include_str!(
            "../../web/tests/fixtures/g36_pre_obc_optimal_start.json"
        ))
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
        assert_eq!(compiled["occupied"], true);
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
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_air_setpoint".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(false)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("zone_presence_raw".into(), json!(false)),
            ("occupancy_count".into(), json!(0.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
            ("measured_airflow".into(), json!(0.0)),
            ("discharge_temp".into(), json!(16.0)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("supply_fan_proven".into(), json!(false)),
            ("actual_fan_speed".into(), json!(0.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
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
    fn g36_vav_low_flow_alarm_uses_measured_flow_and_occupied_gate() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut inputs = HashMap::from([
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(29.0)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(14.0)),
            ("supply_air_setpoint".into(), json!(14.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("discharge_temp".into(), json!(14.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("zone_presence_raw".into(), json!(true)),
            ("occupancy_count".into(), json!(8.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
            ("measured_airflow".into(), json!(0.0)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("supply_fan_proven".into(), json!(false)),
            ("actual_fan_speed".into(), json!(0.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
        ]);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
        for _ in 0..40 {
            let result = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert!(result["airflow_request"].as_f64().unwrap() > 0.1);
            assert_eq!(result["low_airflow_alarm_level"], json!(0));
            assert_eq!(fast["low_airflow_alarm_level"].as_f64(), Some(0.0));
        }
        inputs.insert("supply_fan_proven".into(), json!(true));
        for step in 0..36 {
            let result = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert!(result["airflow_request"].as_f64().unwrap() > 0.1);
            assert_eq!(
                result["low_airflow_alarm_level"],
                json!(if step == 35 { 2 } else { 0 })
            );
            assert_eq!(
                result["low_airflow_alarm_level"].as_f64().unwrap(),
                fast["low_airflow_alarm_level"].as_f64().unwrap()
            );
        }
        let request = dynamic.execute(zone, &inputs, 60.0, "").unwrap()["airflow_request"]
            .as_f64()
            .unwrap();
        inputs.insert("measured_airflow".into(), json!(request * 0.6));
        for _ in 0..6 {
            let result = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(result["low_airflow_alarm_level"], json!(3));
            assert_eq!(
                result["low_airflow_alarm_level"].as_f64().unwrap(),
                fast["low_airflow_alarm_level"].as_f64().unwrap()
            );
        }
        inputs.insert("supply_fan_proven".into(), json!(false));
        let lost_proof = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        assert_eq!(lost_proof["low_airflow_alarm_level"], json!(0));
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["low_airflow_alarm_level"],
            json!(0)
        );
        inputs.insert("supply_fan_proven".into(), json!(true));
        for _ in 0..35 {
            assert_eq!(
                dynamic.execute(zone, &inputs, 60.0, "").unwrap()["low_airflow_alarm_level"],
                json!(0)
            );
        }
        assert_eq!(
            dynamic.execute(zone, &inputs, 60.0, "").unwrap()["low_airflow_alarm_level"],
            json!(3)
        );
        inputs.insert("scheduled_occupied".into(), json!(false));
        inputs.insert("group_setup".into(), json!(true));
        inputs.insert("room_temp".into(), json!(35.0));
        inputs.insert("measured_airflow".into(), json!(0.0));
        let clear = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
        assert_eq!(clear["low_airflow_alarm_level"], json!(0));
        let fresh_setup = Executor::default()
            .execute(zone, &inputs, 60.0, "")
            .unwrap();
        assert!(fresh_setup["airflow_request"].as_f64().unwrap() > 0.1);
        assert_eq!(fresh_setup["low_airflow_alarm_level"], json!(0));
    }
    #[test]
    fn g36_low_flow_alarm_acknowledgement_is_cdl_latched_and_rearms() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1_alarm_acknowledgement")
            .unwrap();
        let mut inputs: HashMap<String, Value> = group
            .inputs
            .iter()
            .map(|port| {
                (
                    port.name.clone(),
                    if port.kind == "Boolean" {
                        json!(false)
                    } else {
                        json!(0)
                    },
                )
            })
            .collect();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
        let mut step = |inputs: &HashMap<String, Value>| {
            let slow = dynamic.execute(group, inputs, 60.0, "").unwrap();
            let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
            assert_eq!(slow["acknowledged_1"], fast["acknowledged_1"]);
            slow["acknowledged_1"].as_bool().unwrap()
        };
        assert!(!step(&inputs));
        inputs.insert("acknowledge_1".into(), json!(true));
        assert!(!step(&inputs));
        inputs.insert("alarm_level_1".into(), json!(2));
        assert!(!step(&inputs)); // A button already held before the fault is not a new press.
        inputs.insert("acknowledge_1".into(), json!(false));
        assert!(!step(&inputs));
        inputs.insert("acknowledge_1".into(), json!(true));
        assert!(step(&inputs));
        inputs.insert("acknowledge_1".into(), json!(false));
        inputs.insert("alarm_level_1".into(), json!(3));
        assert!(step(&inputs)); // A severity change does not discard acknowledgement.
        inputs.insert("alarm_level_1".into(), json!(0));
        assert!(!step(&inputs));
        inputs.insert("alarm_level_1".into(), json!(2));
        assert!(!step(&inputs)); // A new episode must be acknowledged separately.
    }
    #[test]
    fn g36_other_vav_alarm_acknowledgements_are_independent_cdl_state() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1_alarm_acknowledgement")
            .unwrap();
        for kind in [
            "airflow_sensor_alarm",
            "low_discharge_temperature_alarm",
            "leaking_damper_alarm",
            "leaking_reheat_valve_alarm",
            "high_discharge_alarm",
        ] {
            let mut inputs: HashMap<String, Value> = group
                .inputs
                .iter()
                .map(|port| {
                    (
                        port.name.clone(),
                        if port.kind == "Boolean" {
                            json!(false)
                        } else {
                            json!(0)
                        },
                    )
                })
                .collect();
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
            let output = format!("{kind}_acknowledged_1");
            let alarm = format!("{kind}_level_1");
            let mut step = |inputs: &HashMap<String, Value>| {
                let slow = dynamic.execute(group, inputs, 60.0, "").unwrap();
                let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
                assert_eq!(slow[&output], fast[&output], "{kind} executor mismatch");
                assert_eq!(
                    slow["acknowledged_1"],
                    json!(false),
                    "inactive low-flow alarm must not be acknowledged by another alarm"
                );
                slow[&output].as_bool().unwrap()
            };
            assert!(!step(&inputs));
            inputs.insert(alarm.clone(), json!(3));
            assert!(!step(&inputs));
            inputs.insert("acknowledge_1".into(), json!(true));
            assert!(step(&inputs), "{kind} should accept an active-alarm press");
            inputs.insert("acknowledge_1".into(), json!(false));
            assert!(step(&inputs));
            inputs.insert(alarm, json!(0));
            assert!(!step(&inputs), "{kind} should clear with its alarm");
        }
    }
    #[test]
    fn g36_ahu_alarm_acknowledgements_are_independent_cdl_state() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1_equipment_alarm_acknowledgement")
            .unwrap();
        for alarm in [
            "freeze_ventilation_alarm",
            "freeze_alarm",
            "supply_fan_alarm",
            "fan_speed_tracking_alarm",
            "outdoor_damper_tracking_alarm",
            "return_damper_tracking_alarm",
            "group_freeze_alarm",
            "group_window_open_alarm",
        ] {
            let mut inputs: HashMap<String, Value> = group
                .inputs
                .iter()
                .map(|port| {
                    (
                        port.name.clone(),
                        if port.kind == "Boolean" {
                            json!(false)
                        } else {
                            json!(0)
                        },
                    )
                })
                .collect();
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
            let level = format!("ahu_1_{alarm}_level");
            let status = format!("ahu_1_{alarm}_acknowledged");
            let mut step = |inputs: &HashMap<String, Value>| {
                let slow = dynamic.execute(group, inputs, 60.0, "").unwrap();
                let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
                assert_eq!(slow[&status], fast[&status], "{alarm} executor mismatch");
                assert_eq!(
                    slow["ahu_1_supply_fan_alarm_acknowledged"],
                    if alarm == "supply_fan_alarm" {
                        slow[&status].clone()
                    } else {
                        json!(false)
                    }
                );
                slow[&status].as_bool().unwrap()
            };
            assert!(!step(&inputs));
            inputs.insert(level.clone(), json!(3));
            assert!(!step(&inputs));
            inputs.insert("acknowledge".into(), json!(true));
            assert!(step(&inputs), "{alarm} should accept an active-alarm press");
            inputs.insert("acknowledge".into(), json!(false));
            assert!(step(&inputs));
            inputs.insert(level, json!(0));
            assert!(!step(&inputs), "{alarm} should clear with its alarm");
        }
    }
    #[test]
    fn g36_plant_start_fault_acknowledgements_are_independent_cdl_state() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let group = graph
            .blocks
            .iter()
            .find(|block| block.name == "plant_alarm_acknowledgement")
            .unwrap();
        for equipment in ["heat_pump", "boiler", "chiller"] {
            let mut inputs: HashMap<String, Value> = group
                .inputs
                .iter()
                .map(|port| (port.name.clone(), json!(false)))
                .collect();
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(group).unwrap().unwrap();
            let fault = format!("{equipment}_start_fault");
            let status = format!("{fault}_acknowledged");
            let mut step = |inputs: &HashMap<String, Value>| {
                let slow = dynamic.execute(group, inputs, 60.0, "").unwrap();
                let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
                assert_eq!(
                    slow[&status], fast[&status],
                    "{equipment} executor mismatch"
                );
                for other in ["heat_pump", "boiler", "chiller"] {
                    if other != equipment {
                        assert_eq!(
                            slow[&format!("{other}_start_fault_acknowledged")],
                            json!(false)
                        );
                    }
                }
                slow[&status].as_bool().unwrap()
            };
            assert!(!step(&inputs));
            inputs.insert("acknowledge".into(), json!(true));
            assert!(
                !step(&inputs),
                "press before fault must not acknowledge {equipment}"
            );
            inputs.insert(fault.clone(), json!(true));
            assert!(
                !step(&inputs),
                "held press must not acknowledge new {equipment} fault"
            );
            inputs.insert("acknowledge".into(), json!(false));
            assert!(!step(&inputs));
            inputs.insert("acknowledge".into(), json!(true));
            assert!(step(&inputs));
            inputs.insert("acknowledge".into(), json!(false));
            assert!(step(&inputs));
            inputs.insert(fault, json!(false));
            assert!(!step(&inputs), "{equipment} acknowledgment clears on reset");
        }
    }
    #[test]
    fn scheduled_plant_operator_contact_reaches_cdl_start_fault_acknowledgement() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: -5.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let scenario = json!({
            "step_seconds": 60,
            "equipment_start_failures": [{"equipment": "heat_pump", "start_hour": 7.75, "duration_hours": 0.5}],
            "heat_pump_fault_reset_hour": 8.25,
            "plant_alarm_acknowledgements": [{"start_hour": 8.0, "duration_hours": 0.25}]
        });
        let mut sim = Simulation::new(graph, &weather, &scenario.to_string()).unwrap();
        start_present_window(&mut sim);
        sim.temps = [16.0; 10];
        sim.last_heat_request = 100.0;
        sim.last_water_flow_kg_s = [2.0, 2.0];
        let fault = "FourPipeG36Controls|output.plant_heat_pump_start_fault";
        let acknowledged = "FourPipeG36Controls|output.plant_heat_pump_start_fault_acknowledged";
        sim.set_traces(&json!([fault, acknowledged]).to_string())
            .unwrap();
        let rows = run_weather_intervals(&mut sim, 3);
        let rows = rows["rows"].as_array().unwrap();
        assert_eq!(rows[0]["traces"][fault], json!(1.0));
        assert_eq!(rows[0]["traces"][acknowledged], json!(0.0));
        assert_eq!(rows[1]["traces"][fault], json!(1.0));
        assert_eq!(rows[1]["traces"][acknowledged], json!(1.0));
        assert_eq!(rows[2]["traces"][fault], json!(0.0));
        assert_eq!(rows[2]["traces"][acknowledged], json!(0.0));
    }
    #[test]
    fn scheduled_ahu_operator_contact_reaches_cdl_alarm_acknowledgement() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 28.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let scenario = json!({
            "step_seconds": 60,
            "fan_faults": [{"equipment": "ahu_1_stuck_off", "start_hour": 0, "duration_hours": 2}],
            "ahu_alarm_acknowledgements": [{"ahu": 1, "start_hour": 0.5, "duration_hours": 0.25}]
        });
        let mut sim = Simulation::new(graph, &weather, &scenario.to_string()).unwrap();
        sim.temps = [29.0; 10];
        let level = "FourPipeG36Controls|output.ahu_1_supply_fan_alarm_level";
        let acknowledged = "FourPipeG36Controls|output.ahu_1_supply_fan_alarm_acknowledged";
        sim.set_traces(&json!([level, acknowledged]).to_string())
            .unwrap();
        let rows = run_weather_intervals(&mut sim, 4);
        let rows = rows["rows"].as_array().unwrap();
        assert!(rows[1]["traces"][level].as_f64().unwrap() > 0.0);
        assert_eq!(rows[1]["traces"][acknowledged], json!(0.0));
        assert_eq!(rows[2]["traces"][acknowledged], json!(1.0));
        assert!(
            rows[2]["traces"][level].as_f64().unwrap() > 0.0,
            "acknowledgement must not suppress the active alarm"
        );
    }
    #[test]
    fn scheduled_group_window_alarm_reaches_ahu_cdl_acknowledgement() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
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
        let scenario = json!({
            "step_seconds": 60,
            "window_openings": [{"zone": 1, "start_hour": 0, "duration_hours": 2}],
            "ahu_alarm_acknowledgements": [{"ahu": 1, "start_hour": 0.5, "duration_hours": 0.25}]
        });
        let mut sim = Simulation::new(graph, &weather, &scenario.to_string()).unwrap();
        let level = "FourPipeG36Controls|output.ahu_1_group_window_open_alarm_level";
        let acknowledged = "FourPipeG36Controls|output.ahu_1_group_window_open_alarm_acknowledged";
        sim.set_traces(&json!([level, acknowledged]).to_string())
            .unwrap();
        let rows = run_weather_intervals(&mut sim, 4);
        let rows = rows["rows"].as_array().unwrap();
        assert!(rows[1]["traces"][level].as_f64().unwrap() > 0.0);
        assert_eq!(rows[1]["traces"][acknowledged], json!(0.0));
        assert_eq!(rows[2]["traces"][acknowledged], json!(1.0));
        assert!(rows[2]["traces"][level].as_f64().unwrap() > 0.0);
    }
    #[test]
    fn scheduled_operator_contact_reaches_cdl_alarm_acknowledgement() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 30.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true,
            };
            35_040
        ])
        .unwrap();
        let scenario = json!({
            "step_seconds": 60,
            "terminal_design_flow": [0.01, 0.95, 0.95, 0.95, 0.95, 0.95, 0.95, 0.95, 0.95, 0.95],
            "alarm_acknowledgements": [{"zone": 1, "start_hour": 9.0, "duration_hours": 0.25}]
        });
        let mut sim = Simulation::new(graph, &weather, &scenario.to_string()).unwrap();
        start_present_window(&mut sim);
        sim.temps[0] = 32.0;
        let level = "FourPipeG36Controls|output.zone_01_low_airflow_alarm_level";
        let acknowledged = "FourPipeG36Controls|output.zone_01_low_airflow_acknowledged";
        sim.set_traces(&json!([level, acknowledged]).to_string())
            .unwrap();
        let rows = run_weather_intervals(&mut sim, 9);
        let rows = rows["rows"].as_array().unwrap();
        assert!(
            rows[4]["traces"][level].as_f64().unwrap() > 0.0,
            "alarm should be active before scheduled contact: {:?}",
            rows.iter()
                .map(|row| &row["traces"][level])
                .collect::<Vec<_>>()
        );
        assert_eq!(rows[4]["traces"][acknowledged], json!(0.0));
        assert_eq!(rows[5]["traces"][acknowledged], json!(1.0));
        assert!(
            rows[5]["traces"][level].as_f64().unwrap() > 0.0,
            "acknowledgement must not suppress the active alarm"
        );
    }
    #[test]
    fn g36_high_discharge_alarm_requires_continuous_fan_and_occupied_proof() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut inputs = HashMap::from([
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(24.0)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(14.0)),
            ("supply_air_setpoint".into(), json!(14.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("discharge_temp".into(), json!(60.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("zone_presence_raw".into(), json!(true)),
            ("occupancy_count".into(), json!(8.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
            ("measured_airflow".into(), json!(0.2)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("supply_fan_proven".into(), json!(false)),
            ("actual_fan_speed".into(), json!(0.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
        ]);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
        let mut step = |inputs: &HashMap<String, Value>| {
            let slow = dynamic.execute(zone, inputs, 60.0, "").unwrap();
            let fast = compiled.execute(inputs, 60.0, false).unwrap().0;
            assert_eq!(slow["high_discharge_alarm"], fast["high_discharge_alarm"]);
            assert_eq!(
                slow["high_discharge_alarm_level"],
                fast["high_discharge_alarm_level"]
            );
            assert_eq!(
                slow["high_discharge_alarm_level"],
                json!(if slow["high_discharge_alarm"] == json!(true) {
                    3
                } else {
                    0
                })
            );
            slow["high_discharge_alarm"].as_bool().unwrap()
        };
        for _ in 0..8 {
            assert!(!step(&inputs));
        }
        inputs.insert("supply_fan_proven".into(), json!(true));
        for _ in 0..5 {
            assert!(!step(&inputs));
        }
        assert!(step(&inputs));
        inputs.insert("supply_fan_proven".into(), json!(false));
        assert!(!step(&inputs));
        inputs.insert("supply_fan_proven".into(), json!(true));
        assert!(!step(&inputs));
        inputs.insert("scheduled_occupied".into(), json!(false));
        assert!(!step(&inputs));
    }
    #[test]
    fn g36_airflow_sensor_alarm_uses_fan_off_proof_and_measured_flow() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut inputs = HashMap::from([
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(24.0)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(14.0)),
            ("supply_air_setpoint".into(), json!(14.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("discharge_temp".into(), json!(14.0)),
            ("scheduled_occupied".into(), json!(false)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("zone_presence_raw".into(), json!(false)),
            ("occupancy_count".into(), json!(0.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("measured_airflow".into(), json!(0.2)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
        ]);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
        for _ in 0..12 {
            let result = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(result["airflow_sensor_alarm_level"], json!(0));
            assert_eq!(fast["airflow_sensor_alarm_level"], json!(0));
        }
        inputs.insert("supply_fan_proven".into(), json!(false));
        for step in 0..11 {
            let result = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                result["airflow_sensor_alarm_level"],
                json!(if step >= 10 { 3 } else { 0 })
            );
            assert_eq!(
                result["airflow_sensor_alarm_level"],
                fast["airflow_sensor_alarm_level"]
            );
        }
        inputs.insert("measured_airflow".into(), json!(0.09));
        assert_eq!(
            dynamic.execute(zone, &inputs, 60.0, "").unwrap()["airflow_sensor_alarm_level"],
            json!(0)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["airflow_sensor_alarm_level"],
            json!(0)
        );
        inputs.insert("measured_airflow".into(), json!(0.2));
        inputs.insert("supply_fan_proven".into(), json!(true));
        assert_eq!(
            dynamic.execute(zone, &inputs, 60.0, "").unwrap()["airflow_sensor_alarm_level"],
            json!(0)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["airflow_sensor_alarm_level"],
            json!(0)
        );
    }
    #[test]
    fn g36_low_discharge_alarm_uses_physical_heating_proof_and_cdl_thresholds() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut inputs = HashMap::from([
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(24.0)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(14.0)),
            ("supply_air_setpoint".into(), json!(14.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("discharge_temp".into(), json!(10.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("zone_presence_raw".into(), json!(true)),
            ("occupancy_count".into(), json!(8.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("measured_airflow".into(), json!(0.2)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("hot_water_plant_proven".into(), json!(false)),
        ]);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
        let setpoint = dynamic.execute(zone, &inputs, 60.0, "").unwrap()
            ["discharge_air_setpoint_c"]
            .as_f64()
            .unwrap();
        compiled.execute(&inputs, 60.0, false).unwrap();
        inputs.insert("discharge_temp".into(), json!(setpoint - 10.0));
        for _ in 0..12 {
            assert_eq!(
                dynamic.execute(zone, &inputs, 60.0, "").unwrap()
                    ["low_discharge_temperature_alarm_level"],
                json!(0)
            );
            assert_eq!(
                compiled.execute(&inputs, 60.0, false).unwrap().0
                    ["low_discharge_temperature_alarm_level"],
                json!(0)
            );
        }
        inputs.insert("hot_water_plant_proven".into(), json!(true));
        for _ in 0..11 {
            let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(slow["low_discharge_temperature_alarm_level"], json!(3));
            assert_eq!(
                slow["low_discharge_temperature_alarm_level"],
                fast["low_discharge_temperature_alarm_level"]
            );
        }
        inputs.insert("discharge_temp".into(), json!(setpoint - 20.0));
        for step in 0..11 {
            let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(
                slow["low_discharge_temperature_alarm_level"],
                json!(if step >= 10 { 2 } else { 3 })
            );
            assert_eq!(
                slow["low_discharge_temperature_alarm_level"],
                fast["low_discharge_temperature_alarm_level"]
            );
        }
        inputs.insert("hot_water_plant_proven".into(), json!(false));
        assert_eq!(
            dynamic.execute(zone, &inputs, 60.0, "").unwrap()
                ["low_discharge_temperature_alarm_level"],
            json!(0)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0
                ["low_discharge_temperature_alarm_level"],
            json!(0)
        );
        inputs.insert("hot_water_plant_proven".into(), json!(true));
        assert_eq!(
            dynamic.execute(zone, &inputs, 60.0, "").unwrap()
                ["low_discharge_temperature_alarm_level"],
            json!(2)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0
                ["low_discharge_temperature_alarm_level"],
            json!(2)
        );
        let mut suppressed = zone.clone();
        suppressed
            .blocks
            .iter_mut()
            .find(|block| block.name == "low_discharge_importance")
            .unwrap()
            .parameters[0]
            .value = json!(0.0);
        let mut suppression_engine = Executor::default();
        for _ in 0..12 {
            assert_eq!(
                suppression_engine
                    .execute(&suppressed, &inputs, 60.0, "")
                    .unwrap()["low_discharge_temperature_alarm_level"],
                json!(0)
            );
        }
    }
    #[test]
    fn g36_low_discharge_alarm_matches_pinned_obc_validation_trace() {
        let building: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["name"] == "zone_01")
            .unwrap();
        let blocks: Vec<Value> = zone["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|block| {
                block["name"]
                    .as_str()
                    .unwrap()
                    .starts_with("low_discharge_")
            })
            .cloned()
            .collect();
        let connections: Vec<Value> = zone["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|edge| {
                let target = edge["to"].as_str().unwrap();
                if !target.starts_with("low_discharge_") {
                    return None;
                }
                let mut edge = edge.clone();
                edge["from"] = match edge["from"].as_str().unwrap() {
                    "local_occupied.y" => json!("occupied"),
                    "discharge_target.y" => json!("target"),
                    source => json!(source),
                };
                if target == "low_discharge_temperature_alarm_level" {
                    edge["to"] = json!("alarm");
                }
                Some(edge)
            })
            .collect();
        let branch: Block = serde_json::from_value(json!({
            "name":"OBCReheatLowDischargeAlarm",
            "block_type":"Composite",
            "category":"composite",
            "inputs":[
                {"name":"occupied","type":"Boolean"},
                {"name":"supply_fan_proven","type":"Boolean"},
                {"name":"hot_water_plant_proven","type":"Boolean"},
                {"name":"target","type":"Real","unit":"°C"},
                {"name":"discharge_temp","type":"Real","unit":"°C"}
            ],
            "outputs":[{"name":"alarm","type":"Integer"}],
            "blocks":blocks,
            "connections":connections
        }))
        .unwrap();
        assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
        let archive = include_str!("../../verification/g36_reheat_alarms_dymola.txt");
        let expected: Vec<i64> = archive
            .lines()
            .find(|line| line.starts_with("ala.yLowTemAla="))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|value| value.trim().parse::<f64>().unwrap() as i64)
            .collect();
        assert_eq!(expected.len(), 101);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        for tick in 0..=7_200 {
            let seconds = tick as f64;
            let inputs = HashMap::from([
                ("occupied".into(), json!(tick >= 850)),
                ("supply_fan_proven".into(), json!(tick < 5_625)),
                ("hot_water_plant_proven".into(), json!(tick < 6_750)),
                ("target".into(), json!(30.0)),
                (
                    "discharge_temp".into(),
                    json!(20.0 - 5.0 * (seconds / 3_600.0).min(1.0)),
                ),
            ]);
            let ordinary = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
            if tick % 72 == 0 {
                let sample = tick / 72;
                for result in [&ordinary, &fast] {
                    assert_eq!(result["alarm"], json!(expected[sample]), "sample {sample}");
                }
            }
        }
    }
    #[test]
    fn g36_airflow_sensor_alarm_matches_pinned_obc_validation_trace() {
        let building: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["name"] == "zone_01")
            .unwrap();
        let mut blocks: Vec<Value> = zone["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|block| block["name"].as_str().unwrap().starts_with("sensor_"))
            .cloned()
            .collect();
        blocks
            .iter_mut()
            .find(|block| block["name"] == "sensor_cooling_design_flow")
            .unwrap()["parameters"][0]["value"] = json!(1.0);
        let connections: Vec<Value> = zone["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|edge| {
                let target = edge["to"].as_str().unwrap();
                if !target.starts_with("sensor_") && target != "airflow_sensor_alarm_level" {
                    return None;
                }
                let mut edge = edge.clone();
                if target == "airflow_sensor_alarm_level" {
                    edge["to"] = json!("alarm");
                }
                Some(edge)
            })
            .collect();
        let branch: Block = serde_json::from_value(json!({
            "name":"OBCReheatAirflowSensorAlarm",
            "block_type":"Composite",
            "category":"composite",
            "inputs":[
                {"name":"supply_fan_proven","type":"Boolean"},
                {"name":"measured_airflow","type":"Real","unit":"normalized airflow"}
            ],
            "outputs":[{"name":"alarm","type":"Integer"}],
            "blocks":blocks,
            "connections":connections
        }))
        .unwrap();
        assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
        let archive = include_str!("../../verification/g36_reheat_alarms_dymola.txt");
        let expected: Vec<i64> = archive
            .lines()
            .find(|line| line.starts_with("ala.yFloSenAla="))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|value| value.trim().parse::<f64>().unwrap() as i64)
            .collect();
        assert_eq!(expected.len(), 101);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        for tick in 0..=7_200 {
            let seconds = tick as f64;
            let inputs = HashMap::from([
                ("supply_fan_proven".into(), json!(tick < 5_625)),
                (
                    "measured_airflow".into(),
                    json!(0.1 + 0.3 * seconds / 7_200.0),
                ),
            ]);
            let ordinary = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
            if tick % 72 == 0 {
                let sample = tick / 72;
                for result in [&ordinary, &fast] {
                    assert_eq!(result["alarm"], json!(expected[sample]), "sample {sample}");
                }
            }
        }
    }
    fn g36_low_airflow_alarm_branch() -> Block {
        let building: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = building["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|block| block["name"] == "zone_01")
            .unwrap();
        let is_alarm_block = |name: &str| {
            name.starts_with("low_flow_")
                || name == "flow_requested"
                || name == "flow_alarm_enable_threshold"
                || matches!(
                    name,
                    "pressure_importance"
                        | "pressure_importance_zero"
                        | "pressure_importance_positive"
                )
        };
        let mut blocks: Vec<Value> = zone["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|block| is_alarm_block(block["name"].as_str().unwrap()))
            .cloned()
            .collect();
        blocks
            .iter_mut()
            .find(|block| block["name"] == "flow_alarm_enable_threshold")
            .unwrap()["parameters"][0]["value"] = json!(0.01);
        for (name, hysteresis) in [
            ("flow_requested", 0.005),
            ("low_flow_detected", 0.01),
            ("low_flow_below_seventy", 0.01),
        ] {
            blocks
                .iter_mut()
                .find(|block| block["name"] == name)
                .unwrap()["parameters"][0]["value"] = json!(hysteresis);
        }
        let connections: Vec<Value> = zone["connections"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|edge| {
                let target = edge["to"].as_str().unwrap();
                if !is_alarm_block(target.split('.').next().unwrap())
                    && target != "low_airflow_alarm_level"
                {
                    return None;
                }
                let mut edge = edge.clone();
                edge["from"] = match edge["from"].as_str().unwrap() {
                    "local_occupied.y" => json!("occupied"),
                    "flow_override_select.y" => json!("setpoint"),
                    source => json!(source),
                };
                if target == "low_airflow_alarm_level" {
                    edge["to"] = json!("alarm");
                }
                Some(edge)
            })
            .collect();
        let branch: Block = serde_json::from_value(json!({
            "name":"OBCReheatLowAirflowAlarm",
            "block_type":"Composite",
            "category":"composite",
            "inputs":[
                {"name":"occupied","type":"Boolean"},
                {"name":"supply_fan_proven","type":"Boolean"},
                {"name":"measured_airflow","type":"Real","unit":"normalized airflow"},
                {"name":"setpoint","type":"Real","unit":"normalized airflow"}
            ],
            "outputs":[{"name":"alarm","type":"Integer"}],
            "blocks":blocks,
            "connections":connections
        }))
        .unwrap();
        assert!(validate(&branch).is_empty(), "{:?}", validate(&branch));
        branch
    }

    #[test]
    fn g36_low_airflow_alarm_matches_pinned_obc_validation_trace() {
        let branch = g36_low_airflow_alarm_branch();
        let archive = include_str!("../../verification/g36_reheat_alarms_dymola.txt");
        let expected: Vec<i64> = archive
            .lines()
            .find(|line| line.starts_with("ala.yLowFloAla="))
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .trim_matches(['[', ']'])
            .split(',')
            .map(|value| value.trim().parse::<f64>().unwrap() as i64)
            .collect();
        assert_eq!(expected.len(), 101);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        for tick in 0..=7_200 {
            let seconds = tick as f64;
            let inputs = HashMap::from([
                ("occupied".into(), json!(tick >= 850)),
                ("supply_fan_proven".into(), json!(tick < 5_625)),
                ("setpoint".into(), json!(0.1 + 0.9 * seconds / 7_200.0)),
                (
                    "measured_airflow".into(),
                    json!(0.1 + 0.3 * seconds / 7_200.0),
                ),
            ]);
            let ordinary = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
            if tick % 72 == 0 {
                let sample = tick / 72;
                for result in [&ordinary, &fast] {
                    assert_eq!(result["alarm"], json!(expected[sample]), "sample {sample}");
                }
            }
        }
    }
    #[test]
    fn g36_low_airflow_alarm_requires_sustained_new_flow_request() {
        let branch = g36_low_airflow_alarm_branch();
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&branch).unwrap().unwrap();
        // The fan and flow deficit have been steady for five minutes while
        // the setpoint stayed below the OBC near-zero guard. A new request
        // must still be present for five minutes before either level appears.
        for tick in 0..=900 {
            let setpoint = if (301..=420).contains(&tick) || tick >= 481 {
                0.1
            } else {
                0.005
            };
            let inputs = HashMap::from([
                ("occupied".into(), json!(true)),
                ("supply_fan_proven".into(), json!(true)),
                ("setpoint".into(), json!(setpoint)),
                ("measured_airflow".into(), json!(0.0)),
            ]);
            let ordinary = dynamic.execute(&branch, &inputs, 1.0, "").unwrap();
            let fast = compiled.execute(&inputs, 1.0, false).unwrap().0;
            for result in [&ordinary, &fast] {
                assert_eq!(
                    result["alarm"],
                    json!(if tick >= 781 { 2 } else { 0 }),
                    "tick {tick}"
                );
            }
        }
    }
    #[test]
    fn hot_water_plant_proof_reports_physical_heating_not_cooling_only() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        for (outdoor, zone_temp, heat_request, cool_request, expected) in
            [(-5.0, 16.0, 100.0, 0.0, 1.0), (35.0, 32.0, 0.0, 100.0, 0.0)]
        {
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
            let mut sim = Simulation::new(graph, &weather, r#"{"step_seconds":60}"#).unwrap();
            start_present_window(&mut sim);
            sim.temps = [zone_temp; 10];
            sim.last_heat_request = heat_request;
            sim.last_cool_request = cool_request;
            sim.last_water_flow_kg_s = [2.0, 2.0];
            let chunk = run_weather_intervals(&mut sim, 1);
            let row = &chunk["rows"][0];
            assert_eq!(row["hot_water_plant_proven"], json!(expected));
            if expected == 0.0 {
                assert_eq!(row["heat_pump_heating_used_kw"], json!(0.0));
                assert!(row["heat_pump_cooling_used_kw"].as_f64().unwrap() > 0.0);
            } else {
                assert!(row["heat_pump_heating_used_kw"].as_f64().unwrap() > 0.0);
            }
        }
    }
    #[test]
    fn g36_terminal_leak_alarms_use_measured_feedback_and_cdl_timers() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        let mut inputs = HashMap::from([
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(24.0)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(14.0)),
            ("supply_air_setpoint".into(), json!(14.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("discharge_temp".into(), json!(20.0)),
            ("scheduled_occupied".into(), json!(false)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("zone_presence_raw".into(), json!(false)),
            ("occupancy_count".into(), json!(0.0)),
            ("window_closed".into(), json!(false)),
            ("local_override".into(), json!(false)),
            ("measured_airflow".into(), json!(0.0)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("hot_water_plant_proven".into(), json!(false)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
        ]);
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(zone).unwrap().unwrap();
        for step in 0..16 {
            let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(slow["damper_position"], json!(0.0));
            assert_eq!(
                slow["leaking_reheat_valve_alarm_level"],
                json!(if step >= 15 { 4 } else { 0 })
            );
            assert_eq!(
                slow["leaking_reheat_valve_alarm_level"],
                fast["leaking_reheat_valve_alarm_level"]
            );
            assert_eq!(slow["leaking_damper_alarm_level"], json!(0));
        }
        inputs.insert("discharge_temp".into(), json!(14.0));
        inputs.insert("measured_airflow".into(), json!(0.2));
        for step in 0..11 {
            let slow = dynamic.execute(zone, &inputs, 60.0, "").unwrap();
            let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
            assert_eq!(slow["leaking_reheat_valve_alarm_level"], json!(0));
            assert_eq!(
                slow["leaking_damper_alarm_level"],
                json!(if step >= 10 { 4 } else { 0 })
            );
            assert_eq!(
                slow["leaking_damper_alarm_level"],
                fast["leaking_damper_alarm_level"]
            );
        }
        inputs.insert("supply_fan_proven".into(), json!(false));
        assert_eq!(
            dynamic.execute(zone, &inputs, 60.0, "").unwrap()["leaking_damper_alarm_level"],
            json!(0)
        );
        assert_eq!(
            compiled.execute(&inputs, 60.0, false).unwrap().0["leaking_damper_alarm_level"],
            json!(0)
        );
    }
    #[test]
    fn g36_terminal_leak_closed_thresholds_follow_obc_defaults() {
        let graph: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let zone = graph
            .blocks
            .iter()
            .find(|block| block.name == "zone_01")
            .unwrap();
        for (constant_name, comparator_name) in [
            ("leak_damper_closed_limit", "leak_damper_command_closed"),
            ("leak_valve_closed_limit", "leak_valve_position_closed"),
        ] {
            let constant = zone
                .blocks
                .iter()
                .find(|block| block.name == constant_name)
                .unwrap();
            assert_eq!(param(constant, "k", 0.0), 0.05);
            let comparator = zone
                .blocks
                .iter()
                .find(|block| block.name == comparator_name)
                .unwrap();
            assert_eq!(param(comparator, "h", 0.0), 0.025);
            let mut dynamic = Executor::default();
            let mut compiled = CompiledExecutor::try_new(comparator).unwrap().unwrap();
            for (position, expected) in [(0.06, false), (0.04, true), (0.06, true), (0.08, false)] {
                let inputs =
                    HashMap::from([("u1".into(), json!(position)), ("u2".into(), json!(0.05))]);
                let slow = dynamic.execute(comparator, &inputs, 60.0, "").unwrap();
                let fast = compiled.execute(&inputs, 60.0, false).unwrap().0;
                assert_eq!(
                    slow["y"],
                    json!(expected),
                    "{comparator_name} at {position}"
                );
                assert_eq!(slow["y"], fast["y"]);
            }
        }
    }
    #[test]
    fn physical_vav_faults_change_delivery_and_cdl_reports_leakage() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let warm_weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 30.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let window = r#"{"window_openings":[{"zone":1,"start_hour":0,"duration_hours":24}]}"#;
        let stuck = r#"{"window_openings":[{"zone":1,"start_hour":0,"duration_hours":24}],"terminal_faults":[{"zone":1,"kind":"damper_stuck_open","start_hour":0,"duration_hours":24}]}"#;
        let mut normal = Simulation::new(graph, &warm_weather, window).unwrap();
        let mut fault = Simulation::new(graph, &warm_weather, stuck).unwrap();
        for sim in [&mut normal, &mut fault] {
            start_present_window(sim);
            sim.temps = [29.0; 10];
        }
        let damper_alarm = "FourPipeG36Controls|output.zone_01_leaking_damper_alarm_level";
        let damper_command = "FourPipeG36Controls|output.zone_01_damper_position";
        fault
            .set_traces(&json!([damper_alarm, damper_command]).to_string())
            .unwrap();
        let base = run_weather_intervals(&mut normal, 2);
        let failed = run_weather_intervals(&mut fault, 2);
        let base_row = &base["rows"][1];
        let fault_row = &failed["rows"][1];
        assert_eq!(fault_row["traces"][damper_command], json!(0.0));
        assert!(base_row["zone_actual_damper_position"][0].as_f64().unwrap() < 1.0);
        assert_eq!(fault_row["zone_actual_damper_position"][0], json!(1.0));
        assert!(
            fault_row["zone_airflow"][0].as_f64().unwrap()
                > base_row["zone_airflow"][0].as_f64().unwrap() + 0.09
        );
        assert_eq!(fault_row["traces"][damper_alarm], json!(4.0));

        let cold_weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: -5.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let leak_scenario = r#"{"terminal_faults":[{"zone":1,"kind":"reheat_valve_leak","start_hour":0,"duration_hours":24}],"alarm_acknowledgements":[{"zone":1,"start_hour":8.5,"duration_hours":0.25}]}"#;
        let mut normal = Simulation::new(graph, &cold_weather, "{}").unwrap();
        let mut leak = Simulation::new(graph, &cold_weather, leak_scenario).unwrap();
        for sim in [&mut normal, &mut leak] {
            start_present_window(sim);
            sim.temps = [19.0; 10];
            sim.temps[0] = 24.0;
            sim.last_heat_request = 100.0;
            sim.last_water_flow_kg_s = [2.0, 2.0];
        }
        let valve_alarm = "FourPipeG36Controls|output.zone_01_leaking_reheat_valve_alarm_level";
        let valve_acknowledged =
            "FourPipeG36Controls|output.zone_01_leaking_reheat_valve_alarm_acknowledged";
        let valve_command = "FourPipeG36Controls|output.zone_01_reheat_valve_position";
        leak.set_traces(&json!([valve_alarm, valve_acknowledged, valve_command]).to_string())
            .unwrap();
        let base = run_weather_intervals(&mut normal, 3);
        let failed = run_weather_intervals(&mut leak, 4);
        let base_row = &base["rows"][2];
        let fault_row = &failed["rows"][2];
        assert_eq!(fault_row["traces"][valve_command], json!(0.0));
        assert_eq!(
            fault_row["zone_actual_reheat_valve_position"][0],
            json!(0.0)
        );
        assert!(
            fault_row["zone_discharge_c"][0].as_f64().unwrap()
                > base_row["zone_discharge_c"][0].as_f64().unwrap() + 3.0
        );
        assert_eq!(fault_row["traces"][valve_alarm], json!(4.0));
        assert_eq!(fault_row["traces"][valve_acknowledged], json!(0.0));
        assert_eq!(failed["rows"][3]["traces"][valve_acknowledged], json!(1.0));
        assert_eq!(failed["rows"][3]["traces"][valve_alarm], json!(4.0));
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
                > idle["rows"][2]["zone_airflow"][0].as_f64().unwrap(),
            "timed local override should deliver primary air while the raw presence sensor is empty"
        );
        assert!(
            active["rows"][2]["zone_c"][0].as_f64().unwrap()
                < idle["rows"][2]["zone_c"][0].as_f64().unwrap()
        );
    }
    #[test]
    fn local_override_button_is_timed_by_editable_cdl() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 22.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: false
            };
            35_040
        ])
        .unwrap();
        let active = "FourPipeG36Controls|output.zone_01_override_active";
        let group = "FourPipeG36Controls|output.ahu_1_group_occupied";
        let run = |graph_json: &str, duration_hours: f64| {
            let scenario = json!({"occupancy_overrides":[{"zone":1,"start_hour":0,"duration_hours":duration_hours}]}).to_string();
            let mut sim = Simulation::new(graph_json, &weather, &scenario).unwrap();
            sim.set_traces(&json!([active, group]).to_string()).unwrap();
            run_weather_intervals(&mut sim, 10)["rows"]
                .as_array()
                .unwrap()
                .clone()
        };
        let short_press = run(graph, 0.25);
        assert_eq!(short_press[0]["traces"][active], json!(1.0));
        assert_eq!(
            short_press[1]["traces"][active],
            json!(1.0),
            "a released button retains the timed occupied request"
        );
        assert_eq!(short_press[7]["traces"][active], json!(1.0));
        assert_eq!(short_press[8]["traces"][active], json!(0.0));
        assert_eq!(short_press[8]["traces"][group], json!(0.0));
        let held_button = run(graph, 3.0);
        assert_eq!(
            held_button[8]["traces"][active],
            json!(0.0),
            "a held button cannot extend one episode past two hours"
        );
        let mut tuned: Value = serde_json::from_str(graph).unwrap();
        tuned["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "zone_01_override_hold")
            .unwrap()["parameters"][0]["value"] = json!(3600.0);
        let tuned_rows = run(&tuned.to_string(), 0.25);
        assert_eq!(tuned_rows[3]["traces"][active], json!(1.0));
        assert_eq!(
            tuned_rows[4]["traces"][active],
            json!(0.0),
            "editing the CDL duration shortens the override"
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
            run_weather_intervals(sim, 80);
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
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(27.0)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_air_setpoint".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("supply_fan_proven".into(), json!(false)),
            ("actual_fan_speed".into(), json!(0.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("zone_presence_raw".into(), json!(true)),
            ("occupancy_count".into(), json!(8.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
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
            ("flow_override_code".into(), json!(0)),
            ("room_temp".into(), json!(15.0)),
            ("co2_ppm".into(), json!(420.0)),
            ("supply_air_temp".into(), json!(16.0)),
            ("supply_air_setpoint".into(), json!(16.0)),
            ("supply_pressure".into(), json!(1.0)),
            ("scheduled_occupied".into(), json!(true)),
            ("group_warmup".into(), json!(false)),
            ("group_cooldown".into(), json!(false)),
            ("group_setback".into(), json!(false)),
            ("group_setup".into(), json!(false)),
            ("group_freeze_setback".into(), json!(false)),
            ("window_closed".into(), json!(true)),
            ("local_override".into(), json!(false)),
            ("supply_fan_proven".into(), json!(false)),
            ("actual_fan_speed".into(), json!(0.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("zone_presence_raw".into(), json!(true)),
            ("occupancy_count".into(), json!(8.0)),
            ("actual_reheat_valve_position".into(), json!(0.0)),
            ("hot_water_plant_proven".into(), json!(false)),
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
                > cold["damper_position"].as_f64().unwrap() + 0.2,
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
                > cold["damper_position"].as_f64().unwrap() + 0.2,
            "warmup damper {}, cold damper {}",
            warmup["damper_position"],
            cold["damper_position"]
        );
    }
    #[test]
    fn g36_discharge_limit_reduces_reheat_energy_with_airflow_feedback() {
        let mut graph: Value =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        for zone in graph["blocks"].as_array_mut().unwrap() {
            if zone["block_type"] != "VAVBoxController" {
                continue;
            }
            zone["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "ventilation_standby_permitted")
                .unwrap()["parameters"][0]["value"] = json!(false);
        }
        let graph = graph.to_string();
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
        let mut normal = Simulation::new(&graph, &weather, scenario).unwrap();
        normal.temps = [15.0; 10];
        normal.last_zone_airflow = [0.4; 10];
        normal.last_heat_request = 200.0;
        let base = run_weather_intervals(&mut normal, 10);
        let mut dynamic = Simulation::new(&graph, &weather, scenario).unwrap();
        dynamic.compiled = None;
        dynamic.temps = [15.0; 10];
        dynamic.last_zone_airflow = [0.4; 10];
        dynamic.last_heat_request = 200.0;
        let dynamic_rows = run_weather_intervals(&mut dynamic, 10);
        assert_eq!(
            base["rows"][9]["zone_discharge_c"],
            dynamic_rows["rows"][9]["zone_discharge_c"]
        );
        let mut edited: Value = serde_json::from_str(&graph).unwrap();
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
        let reheat_energy = |row: &Value| {
            (row["zone_discharge_c"][0].as_f64().unwrap()
                - row["ahu_1_supply_actual_c"].as_f64().unwrap())
                * 1.2
                * row["zone_airflow"][0].as_f64().unwrap()
        };
        // Discharge temperature alone is not monotonic when the edited valve
        // also changes airflow. Compare the delivered sensible heat instead.
        assert!(
            low_row["zone_actual_reheat_valve_position"][0]
                .as_f64()
                .unwrap()
                < base_row["zone_actual_reheat_valve_position"][0]
                    .as_f64()
                    .unwrap()
                    - 0.2
        );
        assert!(
            reheat_energy(base_row) > reheat_energy(low_row) + 2.0,
            "base reheat={} kW, limited reheat={} kW",
            reheat_energy(base_row),
            reheat_energy(low_row)
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
                .unwrap()["parameters"][0]["value"] = json!(1e-6);
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
                    + 1.0,
            "base discharge {}, zero-gain discharge {}, base flow {}, zero-gain flow {}",
            base["rows"][9]["zone_discharge_c"][0],
            no_feedback["rows"][9]["zone_discharge_c"][0],
            base["rows"][9]["zone_airflow"][0],
            no_feedback["rows"][9]["zone_airflow"][0]
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
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
            ("freeze_reset".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
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
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        for (outdoor, fan, stat, expected) in [
            (22.0, true, false, true),
            (23.5, true, false, true),
            (24.1, true, false, true),
            (23.5, true, false, true),
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
            assert_eq!(fast["economizer_mode_enabled"], json!(expected));
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
        for parameter in &mut tuned
            .blocks
            .iter_mut()
            .find(|block| block.name == "economizer_high_limit_hold")
            .unwrap()
            .parameters
        {
            parameter.value = json!(0.0);
        }
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
        // OBC's economizer high-limit switch holds each state for 600 s;
        // fan and freeze proof remain separate immediate safety gates.
        let mut held_dynamic = Executor::default();
        let mut held_compiled = CompiledExecutor::try_new(ahu).unwrap().unwrap();
        inputs.insert("outdoor_c".into(), json!(22.0));
        for _ in 0..4 {
            assert_eq!(
                held_dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
                json!(true)
            );
            assert_eq!(
                held_compiled.execute(&inputs, 60.0, false).unwrap().0["economizer_mode_enabled"],
                json!(true)
            );
        }
        inputs.insert("outdoor_c".into(), json!(24.1));
        for step in 0..7 {
            let expected = step < 6;
            assert_eq!(
                held_dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
                json!(expected)
            );
            assert_eq!(
                held_compiled.execute(&inputs, 60.0, false).unwrap().0["economizer_mode_enabled"],
                json!(expected)
            );
        }
        inputs.insert("outdoor_c".into(), json!(22.0));
        for step in 0..10 {
            let expected = step == 9;
            assert_eq!(
                held_dynamic.execute(ahu, &inputs, 60.0, "").unwrap()["economizer_mode_enabled"],
                json!(expected)
            );
            assert_eq!(
                held_compiled.execute(&inputs, 60.0, false).unwrap().0["economizer_mode_enabled"],
                json!(expected)
            );
        }
    }
    #[test]
    fn g36_fixed_dry_bulb_selection_is_a_typed_cdl_decision() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap();
        let mut differential = ahu.clone();
        let hold = differential
            .blocks
            .iter_mut()
            .find(|block| block.name == "economizer_high_limit_hold")
            .unwrap();
        for parameter in &mut hold.parameters {
            parameter.value = json!(0.0);
        }
        let mut fixed = differential.clone();
        fixed
            .blocks
            .iter_mut()
            .find(|block| block.name == "economizer_use_fixed_dry_bulb")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "k")
            .unwrap()
            .value = json!(true);
        let mut inputs = HashMap::from([
            ("outdoor_c".into(), json!(24.5)),
            ("flow_request".into(), json!(5.0)),
            ("return_air_temp".into(), json!(29.0)),
            ("mixed_air_temp".into(), json!(25.0)),
            ("actual_supply_air_temp".into(), json!(16.0)),
            ("duct_pressure".into(), json!(0.4)),
            ("outdoor_airflow".into(), json!(0.3)),
            ("freeze_stat_trip".into(), json!(false)),
            ("freeze_reset".into(), json!(false)),
            ("supply_fan_proven".into(), json!(true)),
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut outdoor_commands = Vec::new();
        for (graph, expected) in [(&differential, true), (&fixed, false)] {
            let slow = Executor::default()
                .execute(graph, &inputs, 15.0, "")
                .unwrap();
            let fast = CompiledExecutor::try_new(graph)
                .unwrap()
                .unwrap()
                .execute(&inputs, 15.0, false)
                .unwrap()
                .0;
            assert_eq!(slow["economizer_mode_enabled"], json!(expected));
            assert_eq!(fast["economizer_mode_enabled"], json!(expected));
            outdoor_commands.push(slow["outdoor_air_damper_position"].as_f64().unwrap());
        }
        assert!(outdoor_commands[0] > outdoor_commands[1]);
    }
    #[test]
    fn g36_economizer_disable_damper_timers_are_compiled_cdl() {
        let building: Block =
            serde_json::from_str(include_str!("../../web/public/examples/four_pipe_g36.json"))
                .unwrap();
        let mut ahu = building
            .blocks
            .iter()
            .find(|block| block.name == "ahu_1")
            .unwrap()
            .clone();
        let hold = ahu
            .blocks
            .iter_mut()
            .find(|block| block.name == "economizer_high_limit_hold")
            .unwrap();
        for parameter in &mut hold.parameters {
            parameter.value = json!(0.0);
        }
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
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
        }
        let mut dynamic = Executor::default();
        let mut compiled = CompiledExecutor::try_new(&ahu).unwrap().unwrap();
        let mut no_outdoor_delay = ahu.clone();
        no_outdoor_delay
            .blocks
            .iter_mut()
            .find(|block| block.name == "economizer_outdoor_close_delay")
            .unwrap()
            .parameters
            .iter_mut()
            .find(|parameter| parameter.name == "delayTime")
            .unwrap()
            .value = json!(0.0);
        let mut immediate = Executor::default();
        for _ in 0..4 {
            let actual = dynamic.execute(&ahu, &inputs, 15.0, "").unwrap();
            assert_eq!(actual["economizer_mode_enabled"], json!(true));
            compiled.execute(&inputs, 15.0, false).unwrap();
            immediate
                .execute(&no_outdoor_delay, &inputs, 15.0, "")
                .unwrap();
        }
        inputs.insert("outdoor_c".into(), json!(24.1));
        for step in 0..14 {
            let actual = dynamic.execute(&ahu, &inputs, 15.0, "").unwrap();
            let fast = compiled.execute(&inputs, 15.0, false).unwrap().0;
            let without_delay = immediate
                .execute(&no_outdoor_delay, &inputs, 15.0, "")
                .unwrap();
            assert_eq!(actual["economizer_mode_enabled"], json!(false));
            if step == 0 {
                assert!(
                    actual["outdoor_air_damper_position"].as_f64().unwrap()
                        > without_delay["outdoor_air_damper_position"]
                            .as_f64()
                            .unwrap()
                );
            } else {
                assert_eq!(
                    actual["outdoor_air_damper_position"],
                    without_delay["outdoor_air_damper_position"]
                );
            }
            for signal in ["outdoor_air_damper_position", "return_air_damper_position"] {
                let slow = actual[signal].as_f64().unwrap();
                assert!(
                    (slow - fast[signal].as_f64().unwrap()).abs() < 1e-9,
                    "{signal}, step {step}"
                );
            }
            if step < 12 {
                assert_eq!(actual["return_air_damper_position"], json!(1.0));
            }
            if step == 12 {
                assert!(actual["return_air_damper_position"].as_f64().unwrap() < 1.0);
            }
        }
    }
    #[test]
    fn short_physical_outdoor_heat_pulse_exercises_editable_economizer_hold() {
        let graph = include_str!("../../web/public/examples/four_pipe_g36.json");
        let mut immediate: Value = serde_json::from_str(graph).unwrap();
        for ahu in immediate["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|block| block["block_type"] == "AHUController")
        {
            let hold = ahu["blocks"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|block| block["name"] == "economizer_high_limit_hold")
                .unwrap();
            for parameter in hold["parameters"].as_array_mut().unwrap() {
                parameter["value"] = json!(0.0);
            }
        }
        let weather = serde_json::to_string(&vec![
            Weather {
                outdoor_c: 22.0,
                solar_kw: 0.0,
                internal_kw: 0.0,
                occupied: true
            };
            35_040
        ])
        .unwrap();
        let scenario = r#"{"step_seconds":60,"outdoor_temperature_events":[{"delta_c":10,"start_hour":0.05,"duration_hours":0.05}]}"#;
        let mut held = Simulation::new(graph, &weather, scenario).unwrap();
        let mut direct = Simulation::new(&immediate.to_string(), &weather, scenario).unwrap();
        held.temps = [29.0; 10];
        direct.temps = [29.0; 10];
        let held_rows = run_weather_intervals(&mut held, 2);
        let direct_rows = run_weather_intervals(&mut direct, 2);
        let held_row = &held_rows["rows"][1];
        let direct_row = &direct_rows["rows"][1];
        assert!(
            (held_row["zone_c"][0].as_f64().unwrap() - direct_row["zone_c"][0].as_f64().unwrap())
                .abs()
                > 1e-5
        );
        assert!((held.electricity_kwh - direct.electricity_kwh).abs() > 1e-5);
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
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(false)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
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
                outdoor_c: 0.0,
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
        let ahu_air = "FourPipeG36Controls|output.ahu_1_ventilation_uncorrected";
        let ahu_setpoint = "FourPipeG36Controls|output.ahu_1_minimum_outdoor_airflow";
        let primary_minimum = "FourPipeG36Controls.zone_01|window_primary_minimum.y";
        let targets = json!([
            area,
            population,
            minimum,
            ahu_air,
            ahu_setpoint,
            primary_minimum
        ])
        .to_string();
        let scenario = r#"{"outdoor_co2_ppm":900}"#;
        let mut normal = Simulation::new(graph, &weather, scenario).unwrap();
        normal.set_traces(&targets).unwrap();
        let rows = run_weather_intervals(&mut normal, 35);
        assert_eq!(rows["rows"][30]["traces"][area], json!(0.0));
        assert_eq!(rows["rows"][30]["traces"][primary_minimum], json!(0.0));
        assert_eq!(rows["rows"][33]["traces"][area], json!(0.03));
        assert_eq!(rows["rows"][33]["traces"][population], json!(0.02));
        let baseline_minimum = rows["rows"][33]["traces"][minimum].as_f64().unwrap();
        assert!((0.05..=0.0625 + 1e-9).contains(&baseline_minimum));
        let mut low_co2 = Simulation::new(graph, &weather, "{}").unwrap();
        low_co2.set_traces(&targets).unwrap();
        let low_rows = run_weather_intervals(&mut low_co2, 35);
        assert_eq!(low_rows["rows"][33]["traces"][population], json!(0.0));
        let window_scenario = r#"{"outdoor_co2_ppm":900,"window_openings":[{"zone":1,"start_hour":8,"duration_hours":1}]}"#;
        let mut open_window = Simulation::new(graph, &weather, window_scenario).unwrap();
        open_window.set_traces(&targets).unwrap();
        let open_rows = run_weather_intervals(&mut open_window, 35);
        assert_eq!(open_rows["rows"][33]["traces"][area], json!(0.0));
        assert_eq!(open_rows["rows"][33]["traces"][population], json!(0.0));
        assert_eq!(open_rows["rows"][33]["traces"][primary_minimum], json!(0.0));
        let mut no_standby: Value = serde_json::from_str(graph).unwrap();
        no_standby["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "zone_01")
            .unwrap()["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "ventilation_standby_permitted")
            .unwrap()["parameters"][0]["value"] = json!(false);
        let mut retained = Simulation::new(&no_standby.to_string(), &weather, scenario).unwrap();
        retained.set_traces(&targets).unwrap();
        let retained_rows = run_weather_intervals(&mut retained, 35);
        assert_eq!(retained_rows["rows"][30]["traces"][area], json!(0.03));
        assert_eq!(retained_rows["rows"][30]["traces"][population], json!(0.0));
        assert!(
            retained_rows["rows"][30]["traces"][primary_minimum]
                .as_f64()
                .unwrap()
                > 0.0
        );
        let total_flow_difference: f64 = (30..35)
            .map(|index| {
                let retained = retained_rows["rows"][index]["zone_airflow"][0]
                    .as_f64()
                    .unwrap();
                let standby = rows["rows"][index]["zone_airflow"][0].as_f64().unwrap();
                (retained - standby).abs()
            })
            .sum();
        assert!(
            total_flow_difference > 0.02,
            "CDL standby permission should change physical flow across five intervals: total difference={total_flow_difference}"
        );
        let mut retained_window =
            Simulation::new(&no_standby.to_string(), &weather, window_scenario).unwrap();
        retained_window.set_traces(&targets).unwrap();
        let retained_window_rows = run_weather_intervals(&mut retained_window, 35);
        assert_eq!(retained_window_rows["rows"][33]["traces"][area], json!(0.0));
        assert_eq!(
            retained_window_rows["rows"][33]["traces"][primary_minimum],
            json!(0.0)
        );
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
            .find(|block| block["name"] == "ventilation_area_rate")
            .unwrap()["parameters"][0]["value"] = json!(0.0008);
        let mut tuned = Simulation::new(&edited.to_string(), &weather, scenario).unwrap();
        tuned.set_traces(&targets).unwrap();
        let tuned_rows = run_weather_intervals(&mut tuned, 37);
        assert_eq!(tuned_rows["rows"][33]["traces"][area], json!(0.08));
        assert!(tuned_rows["rows"][33]["traces"][minimum].as_f64().unwrap() > baseline_minimum);
        let person_rate = edited["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "zone_01")
            .unwrap()["blocks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|block| block["name"] == "ventilation_person_rate")
            .unwrap();
        person_rate["parameters"][0]["value"] = json!(0.02);
        let mut more_people_air = Simulation::new(&edited.to_string(), &weather, scenario).unwrap();
        more_people_air.set_traces(&targets).unwrap();
        let rate_rows = run_weather_intervals(&mut more_people_air, 37);
        assert_eq!(rate_rows["rows"][33]["traces"][population], json!(0.16));
        assert!(
            rate_rows["rows"][33]["traces"][ahu_air].as_f64().unwrap()
                > tuned_rows["rows"][33]["traces"][ahu_air].as_f64().unwrap()
        );
        assert!(
            rate_rows["rows"][36]["traces"][ahu_setpoint]
                .as_f64()
                .unwrap()
                > tuned_rows["rows"][36]["traces"][ahu_setpoint]
                    .as_f64()
                    .unwrap()
        );
        let greatest_flow_change = (0..37)
            .map(|index| {
                (rate_rows["rows"][index]["ahu_1_outdoor_airflow"]
                    .as_f64()
                    .unwrap()
                    - tuned_rows["rows"][index]["ahu_1_outdoor_airflow"]
                        .as_f64()
                        .unwrap())
                .abs()
            })
            .fold(0.0_f64, f64::max);
        assert!(greatest_flow_change > 0.01,
            "edited rate should change physical outdoor airflow; maximum change {greatest_flow_change}");
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
            ("actual_fan_speed".into(), json!(1.0)),
            ("actual_outdoor_damper_position".into(), json!(0.2)),
            ("actual_return_damper_position".into(), json!(0.8)),
            ("occupied_mode".into(), json!(true)),
        ]);
        add_ahu_ventilation_inputs(&mut inputs);
        for zone in 1..=5 {
            inputs.insert(format!("zone_{zone:02}_pressure_request"), json!(0));
            inputs.insert(format!("zone_{zone:02}_sat_request"), json!(0));
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
    fn g36_economizer_uses_physical_fan_motor_status_as_proof() {
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
            include_str!("../../web/public/examples/obc_zone_status.json"),
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
                        json!(if p.kind == "Boolean" {
                            1.0
                        } else if p.name.contains("pressure") {
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
            "zone_01_co2_ppm",
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
            inputs.insert(format!("zone_{zone:02}_occupied"), json!(false));
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
