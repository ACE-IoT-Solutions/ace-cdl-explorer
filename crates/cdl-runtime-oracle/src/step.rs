use crate::model::*;

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

pub fn sampled_step(graph: &Block, requested: usize) -> Result<usize, String> {
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
            Some("TrueDelay") => {
                // A timed control transition needs the same resolution as a
                // sampled event; a zero delay is immediate and adds no clock.
                if let Some(delay) = block
                    .parameters
                    .iter()
                    .find(|p| p.name == "delayTime")
                    .and_then(|p| p.value.as_f64())
                {
                    if delay != 0.0 {
                        result.push(("TrueDelay delayTime", delay));
                    }
                }
            }
            Some("TrueFalseHold") => {
                for parameter in ["trueHoldDuration", "falseHoldDuration"] {
                    if let Some(duration) = block
                        .parameters
                        .iter()
                        .find(|p| p.name == parameter)
                        .and_then(|p| p.value.as_f64())
                    {
                        if duration != 0.0 {
                            result.push(("TrueFalseHold duration", duration));
                        }
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
