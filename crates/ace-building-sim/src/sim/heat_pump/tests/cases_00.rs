use super::*;

#[test]
fn joint_heat_pump_map_interpolates_all_four_conditions_and_rejects_bad_grids() {
    let mut map = HeatPumpJointMap {
        outdoor_c: vec![0.0, 20.0],
        hot_water_c: vec![40.0, 50.0],
        chilled_water_c: vec![5.0, 15.0],
        load_fraction: vec![0.0, 1.0],
        simultaneous_heating_fraction: None,
        values: Vec::new(),
    };
    for outdoor in &map.outdoor_c {
        for hot in &map.hot_water_c {
            for chilled in &map.chilled_water_c {
                for load in &map.load_fraction {
                    let linear =
                        outdoor / 20.0 + (hot - 40.0) / 10.0 + (chilled - 5.0) / 10.0 + load;
                    map.values.push(HeatPumpJointValue {
                        heating_capacity_fraction: 0.4 + 0.1 * linear,
                        cooling_capacity_fraction: 0.5 + 0.1 * linear,
                        heating_cop: 2.0 + linear,
                        cooling_cop: 3.0 + linear,
                        simultaneous_cop: None,
                        simultaneous_cop_by_heating_fraction: None,
                    });
                }
            }
        }
    }
    assert!(valid_heat_pump_joint_map(&map));
    let center = heat_pump_joint_value(&map, 10.0, 45.0, 10.0, 0.5, None);
    assert!((center.heating_capacity_fraction - 0.6).abs() < 1e-12);
    assert!((center.cooling_capacity_fraction - 0.7).abs() < 1e-12);
    assert!((center.heating_cop - 4.0).abs() < 1e-12);
    assert!((center.cooling_cop - 5.0).abs() < 1e-12);
    assert_eq!(
        heat_pump_joint_value(&map, -10.0, 40.0, 5.0, 0.0, None).heating_cop,
        2.0
    );
    for value in &mut map.values {
        value.simultaneous_cop = Some(value.heating_cop + 2.0);
    }
    assert!(valid_heat_pump_joint_map(&map));
    assert_eq!(
        heat_pump_joint_value(&map, 10.0, 45.0, 10.0, 0.5, None).simultaneous_cop,
        Some(6.0)
    );
    map.values[0].simultaneous_cop = None;
    assert!(!valid_heat_pump_joint_map(&map));
    map.values[0].simultaneous_cop = Some(4.0);
    map.simultaneous_heating_fraction = Some(vec![0.2, 0.8]);
    for value in &mut map.values {
        value.simultaneous_cop = None;
        value.simultaneous_cop_by_heating_fraction =
            Some(vec![value.heating_cop + 1.0, value.heating_cop + 3.0]);
    }
    assert!(valid_heat_pump_joint_map(&map));
    assert_eq!(
        heat_pump_joint_value(&map, 10.0, 45.0, 10.0, 0.5, None).simultaneous_cop,
        None
    );
    for (ratio, expected) in [(0.5, 6.0), (0.2, 5.0), (0.8, 7.0)] {
        let value = heat_pump_joint_value(&map, 10.0, 45.0, 10.0, 0.5, Some(ratio));
        assert!((value.simultaneous_cop.unwrap() - expected).abs() < 1e-12);
        assert!(value.simultaneous_cop_by_heating_fraction.is_none());
    }
    map.values[0].simultaneous_cop_by_heating_fraction = Some(vec![5.0]);
    assert!(!valid_heat_pump_joint_map(&map));
    map.values[0].simultaneous_cop_by_heating_fraction = Some(vec![5.0, 7.0]);
    map.values[0].simultaneous_cop = Some(5.0);
    assert!(!valid_heat_pump_joint_map(&map));
    map.values[0].simultaneous_cop = None;
    map.values.pop();
    assert!(!valid_heat_pump_joint_map(&map));
}

#[test]
fn part_load_cop_curve_interpolates_without_changing_thermal_dispatch() {
    let default: Scenario = serde_json::from_str("{}").unwrap();
    assert_eq!(
        part_load_cop_multiplier(&default.heat_pump_part_load_performance, 0.3),
        1.0
    );
    let tuned: Scenario = serde_json::from_str(r#"{"heat_pump_part_load_performance":[{"load_fraction":0.0,"cop_multiplier":0.8},{"load_fraction":0.5,"cop_multiplier":0.9},{"load_fraction":1.0,"cop_multiplier":1.0}]}"#).unwrap();
    let curve = &tuned.heat_pump_part_load_performance;
    assert_eq!(part_load_cop_multiplier(curve, -0.2), 0.8);
    assert!((part_load_cop_multiplier(curve, 0.25) - 0.85).abs() < 1e-12);
    assert!((part_load_cop_multiplier(curve, 0.75) - 0.95).abs() < 1e-12);
    assert_eq!(part_load_cop_multiplier(curve, 1.2), 1.0);
    let heat_kw = 40.0;
    let cool_kw = 20.0;
    let base = heat_pump_compressor_work_kw(heat_kw, cool_kw, 3.0, 4.0, true, 1.0);
    let part_load_work =
        heat_pump_compressor_work_kw(heat_kw, cool_kw, 3.0 * 0.85, 4.0 * 0.85, true, 1.0);
    assert!((part_load_work - base / 0.85).abs() < 1e-12);
}

#[test]
fn water_temperature_cop_curves_interpolate_and_reject_invalid_points() {
    let default: Scenario = serde_json::from_str("{}").unwrap();
    assert_eq!(
        water_cop_multiplier(&default.heat_pump_hot_water_cop_performance, 45.0),
        1.0
    );
    assert_eq!(
        water_cop_multiplier(&default.heat_pump_chilled_water_cop_performance, 7.0),
        1.0
    );
    let points = vec![
        HeatPumpWaterCopPoint {
            water_c: 35.0,
            cop_multiplier: 1.2,
            capacity_multiplier: 0.8,
        },
        HeatPumpWaterCopPoint {
            water_c: 55.0,
            cop_multiplier: 0.8,
            capacity_multiplier: 1.2,
        },
    ];
    assert!(valid_water_cop_curve(&points));
    assert_eq!(water_cop_multiplier(&points, 20.0), 1.2);
    assert!((water_cop_multiplier(&points, 45.0) - 1.0).abs() < 1e-12);
    assert_eq!(water_cop_multiplier(&points, 70.0), 0.8);
    assert_eq!(water_capacity_multiplier(&points, 20.0), 0.8);
    assert!((water_capacity_multiplier(&points, 45.0) - 1.0).abs() < 1e-12);
    assert_eq!(water_capacity_multiplier(&points, 70.0), 1.2);
    let legacy: HeatPumpWaterCopPoint =
        serde_json::from_str(r#"{"water_c":45.0,"cop_multiplier":0.9}"#).unwrap();
    assert_eq!(legacy.capacity_multiplier, 1.0);
    let mut invalid = points.clone();
    invalid[1].capacity_multiplier = 0.0;
    assert!(!valid_water_cop_curve(&invalid));
    let reversed = vec![points[1].clone(), points[0].clone()];
    assert!(!valid_water_cop_curve(&reversed));
}
