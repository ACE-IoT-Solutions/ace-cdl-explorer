use crate::Result;
use serde::{Deserialize, Serialize};

/// One finite time/value observation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Seconds from the start of the experiment.
    pub time: f64,
    /// Numeric output; Boolean outputs use zero and one.
    pub value: f64,
}
/// A nonempty, strictly increasing time series.
pub type Trace = Vec<Sample>;

/// Absolute-tolerance comparison, with one vertical excess per test sample.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comparison {
    /// True when the largest vertical excess is at most 1e-10.
    pub passed: bool,
    /// Largest distance outside the permitted value envelope.
    pub max_value_excess: f64,
    /// Largest minimum horizontal displacement beyond the allowed time tolerance.
    /// None means at least one test value lies outside the entire reference value band.
    pub max_time_excess: Option<f64>,
    /// Index of the largest vertical excess (first index on ties).
    pub worst_sample: usize,
    /// Vertical distance outside the envelope for each test sample.
    pub errors: Vec<f64>,
}

fn validate(trace: &[Sample]) -> Result<()> {
    if trace.is_empty()
        || trace
            .iter()
            .any(|p| !p.time.is_finite() || !p.value.is_finite())
        || trace.windows(2).any(|w| w[0].time >= w[1].time)
    {
        return Err("trace must be nonempty, finite and strictly increasing in time".into());
    }
    Ok(())
}
fn interpolate(a: Sample, b: Sample, t: f64) -> f64 {
    a.value + (b.value - a.value) * ((t - a.time) / (b.time - a.time))
}
fn at(trace: &[Sample], t: f64) -> f64 {
    let i = trace.partition_point(|p| p.time < t);
    if i == 0 {
        trace[0].value
    } else if i == trace.len() {
        trace[i - 1].value
    } else {
        interpolate(trace[i - 1], trace[i], t)
    }
}

/// Compare a test trace with a piecewise-linear reference using absolute tolerances.
///
/// At each test time, the lower and upper bounds are the minimum and maximum of
/// the reference over the time-tolerance window, expanded by the value tolerance.
/// Endpoints are held within the time tolerance; samples beyond the expanded
/// reference domain are rejected. Duplicate timestamps, NaN, infinity and negative
/// tolerances are rejected. This API supports absolute tolerances only.
pub fn compare(
    reference: &[Sample],
    test: &[Sample],
    time_tolerance: f64,
    value_tolerance: f64,
) -> Result<Comparison> {
    validate(reference)?;
    validate(test)?;
    if !time_tolerance.is_finite()
        || !value_tolerance.is_finite()
        || time_tolerance < 0.0
        || value_tolerance < 0.0
    {
        return Err("tolerances must be finite and nonnegative".into());
    }
    let mut errors = Vec::with_capacity(test.len());
    let mut max_time: Option<f64> = Some(0.0);
    for p in test {
        if p.time < reference[0].time - time_tolerance
            || p.time > reference.last().unwrap().time + time_tolerance
        {
            return Err("test timestamp lies outside the expanded reference domain".into());
        }
        let lo = p.time - time_tolerance;
        let hi = p.time + time_tolerance;
        let mut minimum = at(reference, lo).min(at(reference, hi));
        let mut maximum = at(reference, lo).max(at(reference, hi));
        let first = reference.partition_point(|p| p.time < lo);
        let last = reference.partition_point(|p| p.time <= hi);
        for q in &reference[first..last] {
            minimum = minimum.min(q.value);
            maximum = maximum.max(q.value);
        }
        let excess = (minimum - value_tolerance - p.value)
            .max(p.value - maximum - value_tolerance)
            .max(0.0);
        errors.push(excess);
        // Find the nearest point on the entire reference in the vertical value band.
        // Zero excess proves a feasible shift exists in the allowed window.
        if excess > 0.0 && max_time.is_some() {
            let band_lo = p.value - value_tolerance;
            let band_hi = p.value + value_tolerance;
            let mut distance = f64::INFINITY;
            for q in reference {
                if (band_lo..=band_hi).contains(&q.value) {
                    distance = distance.min((q.time - p.time).abs());
                }
            }
            for w in reference.windows(2) {
                if w[0].value == w[1].value {
                    continue;
                }
                let min = w[0].value.min(w[1].value);
                let max = w[0].value.max(w[1].value);
                for value in [band_lo, band_hi] {
                    if (min..=max).contains(&value) {
                        let time = w[0].time
                            + (w[1].time - w[0].time)
                                * ((value - w[0].value) / (w[1].value - w[0].value));
                        distance = distance.min((time - p.time).abs());
                    }
                }
            }
            max_time = distance
                .is_finite()
                .then(|| max_time.unwrap().max((distance - time_tolerance).max(0.0)));
        }
    }
    let mut worst_sample = 0;
    for i in 1..errors.len() {
        if errors[i] > errors[worst_sample] {
            worst_sample = i;
        }
    }
    let max_value_excess = errors[worst_sample];
    Ok(Comparison {
        passed: max_value_excess <= 1e-10,
        max_value_excess,
        max_time_excess: max_time,
        worst_sample,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn trace(values: &[(f64, f64)]) -> Trace {
        values
            .iter()
            .map(|&(time, value)| Sample { time, value })
            .collect()
    }
    #[test]
    fn ramp_timing_and_value_errors_are_distinct() {
        let reference = trace(&[(0., 0.), (10., 10.)]);
        let r = compare(&reference, &trace(&[(5., 8.)]), 2., 0.5).unwrap();
        assert!(!r.passed);
        assert_eq!(r.max_value_excess, 0.5);
        assert_eq!(r.max_time_excess, Some(0.5));
        assert!(
            compare(&reference, &trace(&[(5., 7.5)]), 2., 0.5)
                .unwrap()
                .passed
        );
    }
    #[test]
    fn extrema_and_singletons() {
        let r = trace(&[(0., 0.), (1., 10.), (2., 0.)]);
        assert!(compare(&r, &trace(&[(1.2, 10.)]), 0.3, 0.).unwrap().passed);
        let r = compare(&trace(&[(0., 1.)]), &trace(&[(0., 3.)]), 0., 0.).unwrap();
        assert_eq!(r.max_time_excess, None);
        assert_eq!(r.max_value_excess, 2.);
    }
    #[test]
    fn rejects_invalid_data() {
        let r = trace(&[(0., 0.), (1., 1.)]);
        assert!(compare(&r, &r, -1., 0.).is_err());
        assert!(compare(&r, &trace(&[(0., f64::NAN)]), 0., 0.).is_err());
        assert!(compare(&r, &trace(&[(2., 1.)]), 0., 0.).is_err());
        assert!(compare(&trace(&[(0., 1.), (0., 2.)]), &r, 0., 0.).is_err());
    }
}
