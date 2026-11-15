use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct HeatPumpPerformancePoint {
    pub outdoor_c: f64,
    pub capacity_fraction: f64,
    pub heating_cop: f64,
    pub cooling_cop: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct HeatPumpPartLoadPoint {
    pub load_fraction: f64,
    pub cop_multiplier: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct HeatPumpWaterCopPoint {
    pub water_c: f64,
    pub cop_multiplier: f64,
    #[serde(default = "default_capacity_multiplier")]
    pub capacity_multiplier: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct HeatPumpJointValue {
    pub heating_capacity_fraction: f64,
    pub cooling_capacity_fraction: f64,
    pub heating_cop: f64,
    pub cooling_cop: f64,
    /// Combined useful heating plus cooling divided by compressor electricity.
    /// Optional for compatibility with maps that use the teaching savings rule.
    pub simultaneous_cop: Option<f64>,
    /// Optional COP curve over the map's simultaneous_heating_fraction axis.
    pub simultaneous_cop_by_heating_fraction: Option<Vec<f64>>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct HeatPumpJointMap {
    pub outdoor_c: Vec<f64>,
    pub hot_water_c: Vec<f64>,
    pub chilled_water_c: Vec<f64>,
    pub load_fraction: Vec<f64>,
    /// Heating divided by total useful heating plus cooling during simultaneous service.
    pub simultaneous_heating_fraction: Option<Vec<f64>>,
    /// Flattened in outdoor, hot-water, chilled-water, then load order.
    pub values: Vec<HeatPumpJointValue>,
}

fn valid_joint_axis(axis: &[f64], lower: f64, upper: f64) -> bool {
    !axis.is_empty()
        && axis.len() <= 16
        && axis
            .iter()
            .all(|value| value.is_finite() && (lower..=upper).contains(value))
        && axis.windows(2).all(|pair| pair[0] < pair[1])
}

pub(crate) fn valid_heat_pump_joint_map(map: &HeatPumpJointMap) -> bool {
    valid_joint_axis(&map.outdoor_c, -60.0, 70.0)
        && valid_joint_axis(&map.hot_water_c, -20.0, 100.0)
        && valid_joint_axis(&map.chilled_water_c, -20.0, 100.0)
        && valid_joint_axis(&map.load_fraction, 0.0, 1.0)
        && map.load_fraction.last() == Some(&1.0)
        && map
            .simultaneous_heating_fraction
            .as_ref()
            .is_none_or(|axis| valid_joint_axis(axis, 0.0, 1.0))
        && map.values.len()
            == map.outdoor_c.len()
                * map.hot_water_c.len()
                * map.chilled_water_c.len()
                * map.load_fraction.len()
        && map.values.len() <= 4096
        && map.values.iter().all(|value| {
            [
                value.heating_capacity_fraction,
                value.cooling_capacity_fraction,
            ]
            .iter()
            .all(|fraction| fraction.is_finite() && (0.0..=2.0).contains(fraction))
                && [value.heating_cop, value.cooling_cop]
                    .iter()
                    .all(|cop| cop.is_finite() && (0.5..=15.0).contains(cop))
                && value
                    .simultaneous_cop
                    .is_none_or(|cop| cop.is_finite() && (0.5..=30.0).contains(&cop))
        })
        && map.values.iter().all(|value| {
            value.simultaneous_cop.is_some() == map.values[0].simultaneous_cop.is_some()
        })
        && map.values.iter().all(|value| {
            match (
                &map.simultaneous_heating_fraction,
                &value.simultaneous_cop_by_heating_fraction,
            ) {
                (Some(axis), Some(cops)) => {
                    value.simultaneous_cop.is_none()
                        && cops.len() == axis.len()
                        && cops
                            .iter()
                            .all(|cop| cop.is_finite() && (0.5..=30.0).contains(cop))
                }
                (None, None) => true,
                _ => false,
            }
        })
}

fn joint_bracket(axis: &[f64], value: f64) -> (usize, usize, f64) {
    let upper = axis.partition_point(|point| *point < value);
    if upper == 0 {
        (0, 0, 0.0)
    } else if upper == axis.len() {
        let last = axis.len() - 1;
        (last, last, 0.0)
    } else {
        let lower = upper - 1;
        (
            lower,
            upper,
            (value - axis[lower]) / (axis[upper] - axis[lower]),
        )
    }
}

pub(crate) fn heat_pump_joint_value(
    map: &HeatPumpJointMap,
    outdoor_c: f64,
    hot_water_c: f64,
    chilled_water_c: f64,
    load_fraction: f64,
    simultaneous_heating_fraction: Option<f64>,
) -> HeatPumpJointValue {
    let brackets = [
        joint_bracket(&map.outdoor_c, outdoor_c),
        joint_bracket(&map.hot_water_c, hot_water_c),
        joint_bracket(&map.chilled_water_c, chilled_water_c),
        joint_bracket(&map.load_fraction, load_fraction),
    ];
    let ratio_bracket = map
        .simultaneous_heating_fraction
        .as_ref()
        .zip(simultaneous_heating_fraction)
        .map(|(axis, ratio)| joint_bracket(axis, ratio));
    let mut result = HeatPumpJointValue {
        heating_capacity_fraction: 0.0,
        cooling_capacity_fraction: 0.0,
        heating_cop: 0.0,
        cooling_cop: 0.0,
        simultaneous_cop: (map.values[0].simultaneous_cop.is_some() || ratio_bracket.is_some())
            .then_some(0.0),
        simultaneous_cop_by_heating_fraction: None,
    };
    for corner in 0..16 {
        let mut coordinates = [0; 4];
        let mut weight = 1.0;
        for dimension in 0..4 {
            let (lower, upper, fraction) = brackets[dimension];
            let high = corner & (1 << dimension) != 0;
            coordinates[dimension] = if high { upper } else { lower };
            weight *= if high { fraction } else { 1.0 - fraction };
        }
        if weight == 0.0 {
            continue;
        }
        let index = (((coordinates[0] * map.hot_water_c.len() + coordinates[1])
            * map.chilled_water_c.len()
            + coordinates[2])
            * map.load_fraction.len())
            + coordinates[3];
        let value = &map.values[index];
        result.heating_capacity_fraction += weight * value.heating_capacity_fraction;
        result.cooling_capacity_fraction += weight * value.cooling_capacity_fraction;
        result.heating_cop += weight * value.heating_cop;
        result.cooling_cop += weight * value.cooling_cop;
        if let Some(total) = &mut result.simultaneous_cop {
            let cop = if let (Some((lower, upper, fraction)), Some(cops)) =
                (ratio_bracket, &value.simultaneous_cop_by_heating_fraction)
            {
                cops[lower] * (1.0 - fraction) + cops[upper] * fraction
            } else {
                value.simultaneous_cop.unwrap_or(0.0)
            };
            *total += weight * cop;
        }
    }
    result
}

fn default_capacity_multiplier() -> f64 {
    1.0
}

pub(crate) fn default_heat_pump_performance() -> Vec<HeatPumpPerformancePoint> {
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

pub(crate) fn default_heat_pump_part_load_performance() -> Vec<HeatPumpPartLoadPoint> {
    vec![
        HeatPumpPartLoadPoint {
            load_fraction: 0.0,
            cop_multiplier: 1.0,
        },
        HeatPumpPartLoadPoint {
            load_fraction: 1.0,
            cop_multiplier: 1.0,
        },
    ]
}

pub(crate) fn default_hot_water_cop_performance() -> Vec<HeatPumpWaterCopPoint> {
    vec![
        HeatPumpWaterCopPoint {
            water_c: 35.0,
            cop_multiplier: 1.0,
            capacity_multiplier: 1.0,
        },
        HeatPumpWaterCopPoint {
            water_c: 55.0,
            cop_multiplier: 1.0,
            capacity_multiplier: 1.0,
        },
    ]
}

pub(crate) fn default_chilled_water_cop_performance() -> Vec<HeatPumpWaterCopPoint> {
    vec![
        HeatPumpWaterCopPoint {
            water_c: 4.0,
            cop_multiplier: 1.0,
            capacity_multiplier: 1.0,
        },
        HeatPumpWaterCopPoint {
            water_c: 17.0,
            cop_multiplier: 1.0,
            capacity_multiplier: 1.0,
        },
    ]
}

pub(crate) fn water_cop_multiplier(points: &[HeatPumpWaterCopPoint], water_c: f64) -> f64 {
    let upper = points.partition_point(|point| point.water_c < water_c);
    if upper == 0 {
        return points[0].cop_multiplier;
    }
    if upper == points.len() {
        return points[points.len() - 1].cop_multiplier;
    }
    let before = &points[upper - 1];
    let after = &points[upper];
    let weight = (water_c - before.water_c) / (after.water_c - before.water_c);
    before.cop_multiplier + weight * (after.cop_multiplier - before.cop_multiplier)
}

pub(crate) fn water_capacity_multiplier(points: &[HeatPumpWaterCopPoint], water_c: f64) -> f64 {
    let upper = points.partition_point(|point| point.water_c < water_c);
    if upper == 0 {
        return points[0].capacity_multiplier;
    }
    if upper == points.len() {
        return points[points.len() - 1].capacity_multiplier;
    }
    let before = &points[upper - 1];
    let after = &points[upper];
    let weight = (water_c - before.water_c) / (after.water_c - before.water_c);
    before.capacity_multiplier + weight * (after.capacity_multiplier - before.capacity_multiplier)
}

pub(crate) fn valid_water_cop_curve(points: &[HeatPumpWaterCopPoint]) -> bool {
    !points.is_empty()
        && points.iter().all(|point| {
            point.water_c.is_finite()
                && (-20.0..=100.0).contains(&point.water_c)
                && point.cop_multiplier.is_finite()
                && (0.1..=2.0).contains(&point.cop_multiplier)
                && point.capacity_multiplier.is_finite()
                && (0.1..=2.0).contains(&point.capacity_multiplier)
        })
        && points
            .windows(2)
            .all(|pair| pair[0].water_c < pair[1].water_c)
}

pub(crate) fn part_load_cop_multiplier(
    points: &[HeatPumpPartLoadPoint],
    load_fraction: f64,
) -> f64 {
    let upper = points.partition_point(|point| point.load_fraction < load_fraction);
    if upper == 0 {
        return points[0].cop_multiplier;
    }
    if upper == points.len() {
        return points[points.len() - 1].cop_multiplier;
    }
    let before = &points[upper - 1];
    let after = &points[upper];
    let weight =
        (load_fraction - before.load_fraction) / (after.load_fraction - before.load_fraction);
    before.cop_multiplier + weight * (after.cop_multiplier - before.cop_multiplier)
}

pub(crate) fn heat_pump_performance(
    points: &[HeatPumpPerformancePoint],
    outdoor_c: f64,
) -> (f64, f64, f64) {
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

pub(crate) fn heat_pump_compressor_work_kw(
    heating_kw: f64,
    cooling_kw: f64,
    heating_cop: f64,
    cooling_cop: f64,
    simultaneous: bool,
    savings_fraction: f64,
) -> f64 {
    let heating_work_kw = heating_kw / heating_cop;
    let cooling_work_kw = cooling_kw / cooling_cop;
    let savings_kw = if simultaneous {
        savings_fraction * heating_work_kw.min(cooling_work_kw)
    } else {
        0.0
    };
    heating_work_kw + cooling_work_kw - savings_kw
}

#[cfg(test)]
mod tests;
