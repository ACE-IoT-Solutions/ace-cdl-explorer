use cdl_model::{BlockClass, BlockKind, DataType};
pub(crate) fn supported(class: &BlockClass, family: &str) -> bool {
    // Legacy extensions retain their own explicit interfaces; their builders
    // diagnose missing ports. Standard implementations may claim only the typed
    // scalar interfaces their kernels actually implement.
    if class.kind == BlockKind::Extension {
        return true;
    }
    let family_group = match family {
        "BooleanToInteger" | "BooleanToReal" | "IntegerToReal" | "RealToInteger" => "Conversions",
        "UnitDelay" | "Sampler" | "TriggeredSampler" | "TriggeredMovingMean" => "Discrete",
        "And" | "Or" | "Not" | "Pre" | "Edge" | "FallingEdge" | "TrueDelay" | "TrueFalseHold"
        | "Latch" | "Timer" => "Logical",
        "SampleTrigger" => "Logical.Sources",
        "Switch"
            if class
                .outputs
                .first()
                .is_some_and(|p| p.data_type == DataType::Boolean) =>
        {
            "Logical"
        }
        "Constant" => match class.outputs.first().map(|p| &p.data_type) {
            Some(DataType::Boolean) => "Logical.Sources",
            Some(DataType::Integer) => "Integers.Sources",
            _ => "Reals.Sources",
        },
        _ => {
            if class
                .inputs
                .first()
                .is_some_and(|p| p.data_type == DataType::Integer)
            {
                "Integers"
            } else {
                "Reals"
            }
        }
    };
    if class.id.0
        != format!("http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.{family_group}.{family}")
    {
        return false;
    }
    if class.inputs.iter().chain(&class.outputs).any(|p| {
        !p.dimensions.is_empty()
            || !matches!(
                p.data_type,
                DataType::Real | DataType::Integer | DataType::Boolean
            )
    }) {
        return false;
    }
    let port = |name: &str| class.connector(name).map(|p| &p.data_type);
    let boolean = |name: &str| port(name) == Some(&DataType::Boolean);
    let real = |name: &str| port(name) == Some(&DataType::Real);
    let numeric = |name: &str| matches!(port(name), Some(DataType::Real | DataType::Integer));
    let same = |a: &str, b: &str| port(a).is_some() && port(a) == port(b);
    match family {
        "Constant" => class.inputs.is_empty() && (numeric("y") || boolean("y")),
        "BooleanToInteger" => boolean("u") && port("y") == Some(&DataType::Integer),
        "BooleanToReal" => boolean("u") && real("y"),
        "IntegerToReal" => port("u") == Some(&DataType::Integer) && real("y"),
        "RealToInteger" => real("u") && port("y") == Some(&DataType::Integer),
        "And" | "Or" => boolean("u1") && boolean("u2") && boolean("y"),
        "Not" | "Pre" | "Edge" | "FallingEdge" | "TrueDelay" | "TrueFalseHold" => {
            boolean("u") && boolean("y")
        }
        "Latch" => boolean("u") && boolean("clr") && boolean("y"),
        "Timer" => boolean("u") && real("y") && boolean("passed"),
        "SampleTrigger" => class.inputs.is_empty() && boolean("y"),
        "Switch" => boolean("u2") && same("u1", "u3") && same("u1", "y"),
        "Greater" | "Less" | "Equal" => numeric("u1") && same("u1", "u2") && boolean("y"),
        "GreaterThreshold" | "LessThreshold" => numeric("u") && boolean("y"),
        "Hysteresis" => real("u") && boolean("y"),
        "Add" | "Subtract" | "Multiply" | "Divide" | "Min" | "Max" => {
            numeric("u1") && same("u1", "u2") && same("u1", "y")
        }
        "Line" => ["x1", "f1", "u", "x2", "f2", "y"]
            .iter()
            .all(|name| real(name)),
        "PIDWithReset" => real("u_s") && real("u_m") && real("y") && boolean("trigger"),
        "Sampler" | "UnitDelay" => real("u") && real("y"),
        "TriggeredSampler" | "TriggeredMovingMean" => real("u") && real("y") && boolean("trigger"),
        "AddParameter" | "MultiplyByParameter" | "Limiter" => numeric("u") && same("u", "y"),
        _ => false,
    }
}
