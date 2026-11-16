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
    // One-second operation is explicitly selected. Preserve the existing
    // coarse-step alignment policy for all other requests.
    let minimum = if requested_seconds == 1 { 1 } else { 15 };
    if requested_seconds < minimum {
        return Err(Error::InvalidAlignment(
            "requested building step must be 1 second or at least 15 seconds".into(),
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
            || seconds < minimum as f64
            || (integer - seconds).abs() > 1e-9
            || integer >= u64::MAX as f64
        {
            return Err(Error::InvalidAlignment(format!("Building simulation requires timed events to be a whole number of seconds at least {minimum}; got {seconds}")));
        }
        step = gcd(step, integer as u64);
    }
    if step < minimum {
        return Err(Error::InvalidAlignment(
            "CDL sampling events require a simulation step below the supported 15-second minimum"
                .into(),
        ));
    }
    Ok(step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_one_second_alignment_validates_all_event_periods() {
        assert_eq!(align_periods(1, []).unwrap(), 1);
        assert_eq!(align_periods(1, [0., 1., 2., 15., 900.]).unwrap(), 1);
        for period in [0.5, -1., f64::NAN, f64::INFINITY] {
            assert!(align_periods(1, [period]).is_err());
        }
        assert!(align_periods(0, []).is_err());
        // Existing coarse requests still reject unsupported event resolutions.
        assert!(align_periods(900, [1.]).is_err());
        assert!(align_periods(900, [21.]).is_err());
        assert_eq!(align_periods(900, [15.]).unwrap(), 15);
    }
}
