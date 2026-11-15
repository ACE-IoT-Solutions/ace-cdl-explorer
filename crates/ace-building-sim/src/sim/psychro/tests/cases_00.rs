use super::*;

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
fn zone_condensation_conserves_moisture_and_releases_latent_heat() {
    let sensible_c = 16.0;
    let initial_ratio = humidity_ratio(25.0, 0.8);
    let (settled_c, remaining_ratio, condensate_kg_s, latent_kw) =
        settle_zone_condensation(sensible_c, initial_ratio, 250.0, 5.0, 900.0);
    assert!(settled_c > sensible_c);
    assert!(remaining_ratio < initial_ratio);
    assert!((remaining_ratio - humidity_ratio(settled_c, 1.0)).abs() < 1e-9);
    assert!(
        (condensate_kg_s * 900.0 - 1.2 * 250.0 * (initial_ratio - remaining_ratio)).abs() < 1e-9
    );
    assert!((latent_kw * 0.25 - (settled_c - sensible_c) * 5.0).abs() < 1e-7);
    let dry = settle_zone_condensation(22.0, humidity_ratio(22.0, 0.5), 250.0, 5.0, 900.0);
    assert_eq!(dry.0, 22.0);
    assert_eq!(dry.2, 0.0);
    assert_eq!(dry.3, 0.0);
}

#[test]
fn cooling_coil_accounts_for_condensation_in_water_energy() {
    let mixed_c = 30.0;
    let wet_ratio = humidity_ratio(mixed_c, 0.8);
    let (request_kw, target_c) = cooling_coil_request_kw(mixed_c, wet_ratio, 2.0, 0.8);
    assert!((target_c - 14.0).abs() < 1e-12);
    assert!(request_kw > 1.2 * 2.0 * (mixed_c - target_c));
    let (full_c, full_ratio) =
        cooling_coil_leaving_state(mixed_c, wet_ratio, 2.0, request_kw, target_c);
    assert!((full_c - target_c).abs() < 1e-4);
    assert!(full_ratio < wet_ratio);
    let (limited_c, limited_ratio) =
        cooling_coil_leaving_state(mixed_c, wet_ratio, 2.0, request_kw / 2.0, target_c);
    assert!(limited_c > full_c && limited_c < mixed_c);
    assert!(limited_ratio > full_ratio && limited_ratio <= wet_ratio);
    let delivered_kw = 1.2
        * 2.0
        * (moist_air_enthalpy_kj_kg(mixed_c, wet_ratio)
            - moist_air_enthalpy_kj_kg(limited_c, limited_ratio));
    assert!((delivered_kw - request_kw / 2.0).abs() < 1e-4);

    let dry_ratio = humidity_ratio(mixed_c, 0.2);
    let (dry_request, dry_target) = cooling_coil_request_kw(mixed_c, dry_ratio, 2.0, 0.8);
    let (dry_c, dry_leaving_ratio) =
        cooling_coil_leaving_state(mixed_c, dry_ratio, 2.0, dry_request, dry_target);
    assert!((dry_c - dry_target).abs() < 1e-8);
    assert!((dry_leaving_ratio - dry_ratio).abs() < 1e-12);
}
