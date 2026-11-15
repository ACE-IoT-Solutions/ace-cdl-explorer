use crate::config::{pid_controller_type, ConfigValue};
use crate::ConfiguredInstance;
use cdl_model::{Diagnostic, DiagnosticCode, Value};
fn numeric(block: &ConfiguredInstance<'_>, name: &str) -> Option<f64> {
    let value = block.value(name)?;
    value.as_f64().or_else(|| {
        if !block.is_legacy() && matches!(value.0, Value::Expr(_)) {
            Some(block.number(name, f64::NAN))
        } else {
            None
        }
    })
}
pub(crate) fn check(block: &ConfiguredInstance<'_>, out: &mut Vec<Diagnostic>) {
    let mut issue = |message: String| {
        out.push(Diagnostic::new(
            block.path,
            DiagnosticCode::InvalidValue,
            message,
        ))
    };
    let name = block.family();
    if !block.is_legacy() {
        for parameter in &block.class.parameters {
            if parameter.default.is_none()
                && !block
                    .instance
                    .assignments
                    .iter()
                    .any(|p| p.name == parameter.name)
            {
                issue(format!("missing required parameter {}", parameter.name));
            }
        }
    }
    for parameter in &block.instance.assignments {
        if matches!(parameter.value, Value::String(_) | Value::Expr(_))
            && !(name == "PIDWithReset" && parameter.name == "controllerType")
        {
            issue(format!("unsupported parameter value {}", parameter.name));
        }
        let integer_parameter = (name == "BooleanToInteger"
            && matches!(parameter.name.as_str(), "integerTrue" | "integerFalse"))
            || (name == "GreaterThreshold"
                && block.original_type().contains("Integers.")
                && parameter.name == "t");
        if integer_parameter
            && !ConfigValue(&parameter.value)
                .as_f64()
                .is_some_and(|v| v.is_finite() && v.fract() == 0.0)
        {
            issue(format!("{} must be an integer", parameter.name));
        }
    }
    if matches!(name, "UnitDelay" | "Sampler")
        && !numeric(block, "samplePeriod").is_some_and(|v| v.is_finite() && v > 0.0)
    {
        issue("sampled block requires a positive samplePeriod".into());
    }
    let original = block.original_type();
    if (original.ends_with("Reals.GreaterThreshold")
        || original.ends_with("Reals.LessThreshold")
        || original.ends_with("Reals.Greater")
        || original.ends_with("Reals.Less")
        || matches!(original, "Greater" | "Less"))
        && block.value("h").is_some()
        && !numeric(block, "h").is_some_and(|v| v.is_finite() && v >= 0.0)
    {
        issue("comparator hysteresis must be nonnegative".into());
    }
    if name == "TriggeredMovingMean"
        && !block
            .value("n")
            .is_some_and(|v| matches!(v.0,Value::Integer(n) if *n>0))
    {
        issue("TriggeredMovingMean requires a positive integer n".into());
    }
    if name == "PIDWithReset" {
        let controller = pid_controller_type(block);
        if controller.is_none() {
            issue("controllerType must be P, PI, PD, or PID".into());
        }
        let integral = controller.is_some_and(|c| c.with_integral());
        let derivative = controller.is_some_and(|c| c.with_derivative());
        for parameter in ["k", "r"]
            .into_iter()
            .chain(integral.then_some("Ti"))
            .chain(integral.then_some("Ni"))
            .chain(derivative.then_some("Td"))
            .chain(derivative.then_some("Nd"))
        {
            if block.value(parameter).is_some()
                && !numeric(block, parameter).is_some_and(|v| v.is_finite() && v > 0.0)
            {
                issue(format!("{parameter} must be positive"));
            }
        }
        if block.number("yMin", 0.0) >= block.number("yMax", 1.0) {
            issue("yMin must be less than yMax".into());
        }
    }
    if name == "SampleTrigger"
        && (!numeric(block, "period").is_some_and(|v| v.is_finite() && v > 0.0)
            || block.value("shift").is_some()
                && !numeric(block, "shift").is_some_and(f64::is_finite))
    {
        issue("SampleTrigger requires a positive period and finite shift".into());
    }
    if name == "Timer"
        && block.value("t").is_some()
        && !numeric(block, "t").is_some_and(|v| v.is_finite() && v >= 0.0)
    {
        issue("Timer requires a nonnegative finite t".into());
    }
    if name == "TrueFalseHold"
        && (!numeric(block, "trueHoldDuration").is_some_and(|v| v.is_finite() && v >= 0.0)
            || block.value("falseHoldDuration").is_some()
                && !numeric(block, "falseHoldDuration").is_some_and(|v| v.is_finite() && v >= 0.0))
    {
        issue("TrueFalseHold requires nonnegative finite hold durations".into());
    }
}
