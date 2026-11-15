use crate::exec::interp::{param, pid_controller_type, PidControllerType};
use crate::model::*;
use std::collections::HashSet;

pub(crate) fn known_type(kind: &str) -> bool {
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
                if (parameter.value.is_string()
                    && !(block.block_type.rsplit('.').next() == Some("PIDWithReset")
                        && parameter.name == "controllerType"))
                    || parameter.value.is_null()
                {
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
                let controller_type = pid_controller_type(block);
                if controller_type.is_none() {
                    errors.push(format!("{at}: controllerType must be P, PI, PD, or PID"));
                }
                let with_integral = controller_type.is_some_and(PidControllerType::with_integral);
                let with_derivative =
                    controller_type.is_some_and(PidControllerType::with_derivative);
                for parameter in ["k", "r"]
                    .into_iter()
                    .chain(with_integral.then_some("Ti"))
                    .chain(with_integral.then_some("Ni"))
                    .chain(with_derivative.then_some("Td"))
                    .chain(with_derivative.then_some("Nd"))
                {
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

pub(crate) fn order(block: &Block) -> Result<Vec<usize>, String> {
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
