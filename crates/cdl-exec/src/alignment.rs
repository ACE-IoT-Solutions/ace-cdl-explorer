use crate::Error;
use cdl_blocks::{BlockImpl, ConfiguredInstance, Registry};
use cdl_model::Document;

/// Inspect controller timing without compiling or executing the graph.
///
/// This uses the same registry event periods and alignment rules as
/// [`crate::Program::aligned_step`]. It also works while an editor graph has incomplete
/// wiring. Class resolution and recursion errors are still reported. Executable
/// validation remains the responsibility of [`crate::Program::compile`].
pub fn aligned_step(
    document: &Document,
    registry: &Registry,
    requested_seconds: u64,
) -> Result<u64, Error> {
    let mut periods = Vec::new();
    for item in document.instances().map_err(Error::Diagnostics)? {
        if item.class.body.is_none() {
            if let Some(definition) = registry.get(&item.class.id) {
                let path = item.path.to_string();
                let config = ConfiguredInstance::new(item.instance, item.class, &path);
                periods.extend(definition.event_periods(&config));
            }
        }
    }
    align_periods(requested_seconds, periods)
}

pub(crate) fn align_periods(
    requested_seconds: u64,
    periods: impl IntoIterator<Item = f64>,
) -> Result<u64, Error> {
    if requested_seconds < 15 {
        return Err(Error::InvalidAlignment(
            "requested building step must be at least 15 seconds".into(),
        ));
    }
    fn gcd(mut a: u64, mut b: u64) -> u64 {
        while b != 0 {
            let remainder = a % b;
            a = b;
            b = remainder;
        }
        a
    }
    let mut step = requested_seconds;
    for seconds in periods {
        if seconds == 0.0 {
            continue;
        }
        let integer = seconds.round();
        if !seconds.is_finite()
            || seconds < 15.0
            || (integer - seconds).abs() > 1e-9
            || integer >= u64::MAX as f64
        {
            return Err(Error::InvalidAlignment(format!("Building simulation requires timed events to be a whole number of seconds at least 15; got {seconds}")));
        }
        step = gcd(step, integer as u64);
    }
    if step < 15 {
        return Err(Error::InvalidAlignment(
            "CDL sampling events require a simulation step below the supported 15-second minimum"
                .into(),
        ));
    }
    Ok(step)
}
