pub(super) fn clamp(x: f64, low: f64, high: f64) -> f64 {
    x.max(low).min(high)
}

pub(crate) fn humidity_ratio(temperature_c: f64, relative_humidity: f64) -> f64 {
    let saturation_pa = 610.94 * (17.625 * temperature_c / (temperature_c + 243.04)).exp();
    let vapor_pa = relative_humidity * saturation_pa;
    0.62198 * vapor_pa / (101_325.0 - vapor_pa)
}
pub(crate) fn relative_humidity_from_ratio(temperature_c: f64, ratio: f64) -> f64 {
    if ratio <= 0.0 {
        return 0.0;
    }
    let saturation_pa = 610.94 * (17.625 * temperature_c / (temperature_c + 243.04)).exp();
    let vapor_pa = 101_325.0 * ratio / (0.62198 + ratio);
    vapor_pa / saturation_pa
}
pub(crate) fn moist_air_enthalpy_kj_kg(temperature_c: f64, humidity_ratio: f64) -> f64 {
    1.006 * temperature_c + humidity_ratio * (2501.0 + 1.86 * temperature_c)
}
pub(crate) fn temperature_from_moist_air_enthalpy_c(
    enthalpy_kj_kg: f64,
    humidity_ratio: f64,
) -> f64 {
    (enthalpy_kj_kg - 2501.0 * humidity_ratio) / (1.006 + 1.86 * humidity_ratio)
}
/// Condensate formed at the zone's saturation limit warms the modeled zone.
/// The effective zone capacity includes the air and surrounding thermal mass.
/// Return (temperature, humidity ratio, condensate kg/s, latent heat kW).
pub(crate) fn settle_zone_condensation(
    sensible_c: f64,
    humidity: f64,
    air_volume_m3: f64,
    capacity_kwh_per_k: f64,
    dt_seconds: f64,
) -> (f64, f64, f64, f64) {
    let saturation = humidity_ratio(sensible_c, 1.0);
    if humidity <= saturation || dt_seconds <= 0.0 {
        return (sensible_c, humidity, 0.0, 0.0);
    }
    let dry_air_kg = 1.2 * air_volume_m3;
    let latent_kwh_per_kg = |temperature_c: f64| (2501.0 - 2.361 * temperature_c).max(0.0) / 3600.0;
    let mut low = sensible_c;
    let mut high =
        sensible_c + dry_air_kg * humidity * latent_kwh_per_kg(sensible_c) / capacity_kwh_per_k;
    for _ in 0..32 {
        let mid = (low + high) * 0.5;
        let condensed_kg = dry_air_kg * (humidity - humidity_ratio(mid, 1.0)).max(0.0);
        let settled_c = sensible_c + condensed_kg * latent_kwh_per_kg(mid) / capacity_kwh_per_k;
        if mid < settled_c {
            low = mid;
        } else {
            high = mid;
        }
    }
    let temperature_c = (low + high) * 0.5;
    let remaining_ratio = humidity.min(humidity_ratio(temperature_c, 1.0));
    let condensate_kg_s = dry_air_kg * (humidity - remaining_ratio) / dt_seconds;
    let latent_kw = condensate_kg_s * latent_kwh_per_kg(temperature_c) * 3600.0;
    (temperature_c, remaining_ratio, condensate_kg_s, latent_kw)
}
/// Physical cooling-coil request for the valve's target leaving temperature.
/// A saturated leaving state includes the energy carried by condensed water.
pub(crate) fn cooling_coil_request_kw(
    mixed_c: f64,
    mixed_ratio: f64,
    airflow_m3_s: f64,
    valve_position: f64,
) -> (f64, f64) {
    if airflow_m3_s <= 0.0 || valve_position <= 0.0 || mixed_c <= 8.0 {
        return (0.0, mixed_c);
    }
    let target_c = (mixed_c - 20.0 * valve_position).max(8.0);
    let target_ratio = mixed_ratio.min(humidity_ratio(target_c, 1.0));
    let request_kw = 1.2
        * airflow_m3_s
        * (moist_air_enthalpy_kj_kg(mixed_c, mixed_ratio)
            - moist_air_enthalpy_kj_kg(target_c, target_ratio));
    (request_kw.max(0.0), target_c)
}

/// Resolve the physical leaving air from cooling energy actually delivered by
/// the water loop. Only the water-limited energy can cool and dehumidify air.
pub(crate) fn cooling_coil_leaving_state(
    mixed_c: f64,
    mixed_ratio: f64,
    airflow_m3_s: f64,
    delivered_kw: f64,
    target_c: f64,
) -> (f64, f64) {
    if airflow_m3_s <= 0.0 || delivered_kw <= 0.0 {
        return (mixed_c, mixed_ratio);
    }
    let target_h =
        moist_air_enthalpy_kj_kg(mixed_c, mixed_ratio) - delivered_kw / (1.2 * airflow_m3_s);
    let dry_c = (target_h - 2501.0 * mixed_ratio) / (1.006 + 1.86 * mixed_ratio);
    if dry_c >= target_c && mixed_ratio <= humidity_ratio(dry_c, 1.0) {
        return (dry_c, mixed_ratio);
    }
    let mut low = target_c;
    let mut high = mixed_c;
    for _ in 0..22 {
        let mid = (low + high) * 0.5;
        let mid_ratio = mixed_ratio.min(humidity_ratio(mid, 1.0));
        if moist_air_enthalpy_kj_kg(mid, mid_ratio) > target_h {
            high = mid;
        } else {
            low = mid;
        }
    }
    let leaving_c = (low + high) * 0.5;
    (leaving_c, mixed_ratio.min(humidity_ratio(leaving_c, 1.0)))
}
/// Approximate moist-air enthalpy at standard atmospheric pressure. The
/// economizer cutoff and selection live in CDL.
pub(crate) fn outdoor_air_enthalpy_kj_kg(temperature_c: f64, relative_humidity: f64) -> f64 {
    moist_air_enthalpy_kj_kg(
        temperature_c,
        humidity_ratio(temperature_c, relative_humidity),
    )
}

/// Equivalent pump curve and parallel-load resistance for one water loop.
/// Full speed and full valve conductance produce 40 kPa and 4.31 kg/s.
pub(crate) fn water_loop_hydraulics(
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

#[cfg(test)]
mod tests;
