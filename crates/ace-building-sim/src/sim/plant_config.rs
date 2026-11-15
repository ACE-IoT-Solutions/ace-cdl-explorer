//! Scenario/weather validation and initialization for the controller-free plant.
use super::*;
impl Plant {
    /// Construct a plant without a graph, executor or controller.
    /// Weather contains 35040 quarter-hour samples; Scenario selects a fixed
    /// integration interval (15, 30, 60, 300 or 900 seconds).
    pub fn new(records: Vec<WeatherRecord>, scenario: Scenario) -> Result<Self, SimError> {
        let weather_relative_humidity = records
            .iter()
            .map(|record| record.relative_humidity.unwrap_or(0.5))
            .collect::<Vec<_>>();
        if weather_relative_humidity
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(SimError::from_str("Invalid weather relative humidity"));
        }
        let weather = records
            .into_iter()
            .map(|record| record.weather)
            .collect::<Vec<_>>();
        if weather.iter().any(|w| {
            !w.outdoor_c.is_finite() || !w.solar_kw.is_finite() || !w.internal_kw.is_finite()
        }) {
            return Err(SimError("weather values must be finite".into()));
        }
        if weather.len() != 35_040 {
            return Err(SimError::from_str(
                "Expected 35,040 quarter-hour weather samples",
            ));
        }
        // Calendar information is an exogenous input. The graph decides when
        // that lookahead warrants preconditioning.
        let mut seconds_until_occupied = vec![0.0; weather.len()];
        let mut next_occupied = weather.len();
        for index in (0..weather.len()).rev() {
            if weather[index].occupied {
                next_occupied = index;
            }
            seconds_until_occupied[index] = if next_occupied == weather.len() {
                (weather.len() * 900) as f64
            } else {
                ((next_occupied - index) * 900) as f64
            };
        }
        if !scenario.zone_ua_kw_per_k.is_finite()
            || !scenario.zone_capacity_kwh_per_k.is_finite()
            || scenario.zone_ua_kw_per_k <= 0.0
            || scenario.zone_capacity_kwh_per_k <= 0.0
            || scenario
                .terminal_design_flow
                .iter()
                .any(|flow| !flow.is_finite() || *flow <= 0.0 || *flow > 5.0)
            || scenario
                .terminal_reheat_valve_stroke_seconds
                .iter()
                .any(|stroke| !stroke.is_finite() || !(0.0..=3600.0).contains(stroke))
            || scenario
                .ahu_heating_coil_valve_stroke_seconds
                .iter()
                .chain(scenario.ahu_cooling_coil_valve_stroke_seconds.iter())
                .any(|stroke| !stroke.is_finite() || !(0.0..=3600.0).contains(stroke))
            || scenario
                .ahu_fan_design_airflow
                .iter()
                .any(|flow| !flow.is_finite() || *flow <= 0.0 || *flow > 20.0)
            || scenario
                .ahu_fan_startup_seconds
                .iter()
                .any(|seconds| !seconds.is_finite() || !(0.0..=3600.0).contains(seconds))
            || !scenario.ahu_damper_stroke_seconds.is_finite()
            || !(1.0..=3600.0).contains(&scenario.ahu_damper_stroke_seconds)
            || scenario
                .water_pump_design_flow_kg_s
                .iter()
                .any(|flow| !flow.is_finite() || *flow <= 0.0 || *flow > 20.0)
            || scenario
                .water_volume_kg
                .iter()
                .any(|mass| !mass.is_finite() || !(100.0..=100_000.0).contains(mass))
            || scenario
                .water_pump_startup_seconds
                .iter()
                .any(|seconds| !seconds.is_finite() || !(0.0..=3600.0).contains(seconds))
            || scenario
                .water_pump_shutoff_pressure_pa
                .iter()
                .any(|pressure| {
                    !pressure.is_finite() || *pressure < 10_000.0 || *pressure > 500_000.0
                })
            || scenario
                .water_bypass_conductance
                .iter()
                .chain(scenario.water_coil_conductance.iter())
                .chain(std::iter::once(&scenario.hot_reheat_conductance))
                .any(|weight| !weight.is_finite() || !(0.0..=1.0).contains(weight))
            || !scenario.zone_air_volume_m3.is_finite()
            || scenario.zone_air_volume_m3 <= 0.0
            || scenario.zone_air_volume_m3 > 10_000.0
            || scenario.zone_air_volume_m3_by_zone.is_some_and(|values| {
                values
                    .iter()
                    .any(|value| !value.is_finite() || !(1.0..=10_000.0).contains(value))
            })
            || !scenario.zone_infiltration_m3_s.is_finite()
            || scenario.zone_infiltration_m3_s < 0.0
            || scenario.zone_infiltration_m3_s > 5.0
            || scenario
                .zone_infiltration_m3_s_by_zone
                .is_some_and(|values| {
                    values
                        .iter()
                        .any(|value| !value.is_finite() || !(0.0..=5.0).contains(value))
                })
            || !scenario
                .open_window_additional_infiltration_m3_s
                .is_finite()
            || scenario.open_window_additional_infiltration_m3_s < 0.0
            || scenario.open_window_additional_infiltration_m3_s > 5.0
            || scenario
                .open_window_additional_infiltration_m3_s_by_zone
                .is_some_and(|values| {
                    values
                        .iter()
                        .any(|value| !value.is_finite() || !(0.0..=5.0).contains(value))
                })
            || !scenario.design_occupants_per_zone.is_finite()
            || scenario.design_occupants_per_zone < 0.0
            || scenario.design_occupants_per_zone > 1_000.0
            || scenario.zone_design_occupants.is_some_and(|counts| {
                counts
                    .iter()
                    .any(|count| !count.is_finite() || !(0.0..=1_000.0).contains(count))
            })
            || !scenario.co2_generation_lps_per_person.is_finite()
            || scenario.co2_generation_lps_per_person < 0.0
            || scenario.co2_generation_lps_per_person > 0.1
            || !scenario.occupant_sensible_heat_kw_per_person.is_finite()
            || !(0.0..=0.3).contains(&scenario.occupant_sensible_heat_kw_per_person)
            || !scenario.outdoor_co2_ppm.is_finite()
            || !(250.0..=2000.0).contains(&scenario.outdoor_co2_ppm)
            || scenario
                .outdoor_relative_humidity
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || scenario
                .return_air_relative_humidity
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || !scenario.occupant_moisture_kg_h_per_person.is_finite()
            || !(0.0..=0.5).contains(&scenario.occupant_moisture_kg_h_per_person)
            || !(0..=2).contains(&scenario.pollution_operator)
            || !scenario.pollution_on_duration_s.is_finite()
            || !(15.0..=604_800.0).contains(&scenario.pollution_on_duration_s)
            || !scenario.local_utc_offset_hours.is_finite()
            || !(-12.0..=14.0).contains(&scenario.local_utc_offset_hours)
            || scenario.clock_offset_events.iter().any(|event| {
                !event.start_hour.is_finite()
                    || !(0.0..=8760.0).contains(&event.start_hour)
                    || !event.utc_offset_hours.is_finite()
                    || !(-12.0..=14.0).contains(&event.utc_offset_hours)
            })
            || scenario
                .outdoor_pm25_ug_m3
                .is_some_and(|v| !v.is_finite() || !(0.0..=2000.0).contains(&v))
            || scenario
                .outdoor_ozone_ppm
                .is_some_and(|v| !v.is_finite() || !(0.0..=10.0).contains(&v))
            || scenario
                .outdoor_aqi
                .is_some_and(|v| !v.is_finite() || !(0.0..=500.0).contains(&v))
            || scenario.air_quality_events.iter().any(|event| {
                let max = match event.pollutant.as_str() {
                    "pm25" => 2000.0,
                    "ozone" => 10.0,
                    "aqi" => 500.0,
                    _ => return true,
                };
                event
                    .value
                    .is_some_and(|v| !v.is_finite() || !(0.0..=max).contains(&v))
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || !scenario.heat_pump_capacity_kw.is_finite()
            || scenario.heat_pump_capacity_kw <= 0.0
            || !scenario
                .heat_recovery_compressor_savings_fraction
                .is_finite()
            || !(0.0..=1.0).contains(&scenario.heat_recovery_compressor_savings_fraction)
            || scenario.heat_pump_performance.is_empty()
            || scenario.heat_pump_performance.iter().any(|point| {
                !point.outdoor_c.is_finite()
                    || !point.capacity_fraction.is_finite()
                    || point.capacity_fraction <= 0.0
                    || point.capacity_fraction > 1.0
                    || !point.heating_cop.is_finite()
                    || point.heating_cop <= 0.0
                    || !point.cooling_cop.is_finite()
                    || point.cooling_cop <= 0.0
            })
            || scenario
                .heat_pump_performance
                .windows(2)
                .any(|pair| pair[0].outdoor_c >= pair[1].outdoor_c)
            || scenario.heat_pump_part_load_performance.is_empty()
            || scenario
                .heat_pump_part_load_performance
                .iter()
                .any(|point| {
                    !point.load_fraction.is_finite()
                        || !(0.0..=1.0).contains(&point.load_fraction)
                        || !point.cop_multiplier.is_finite()
                        || !(0.1..=2.0).contains(&point.cop_multiplier)
                })
            || scenario
                .heat_pump_part_load_performance
                .windows(2)
                .any(|pair| pair[0].load_fraction >= pair[1].load_fraction)
            || !valid_water_cop_curve(&scenario.heat_pump_hot_water_cop_performance)
            || !valid_water_cop_curve(&scenario.heat_pump_chilled_water_cop_performance)
            || scenario
                .heat_pump_joint_performance
                .as_ref()
                .is_some_and(|map| !valid_heat_pump_joint_map(map))
            || scenario.equipment_outages.iter().any(|outage| {
                !matches!(
                    outage.equipment.as_str(),
                    "heat_pump" | "boiler" | "chiller"
                ) || !outage.start_hour.is_finite()
                    || outage.start_hour < 0.0
                    || !outage.duration_hours.is_finite()
                    || outage.duration_hours <= 0.0
            })
            || scenario.equipment_start_failures.iter().any(|failure| {
                !matches!(
                    failure.equipment.as_str(),
                    "heat_pump" | "boiler" | "chiller"
                ) || !failure.start_hour.is_finite()
                    || failure.start_hour < 0.0
                    || !failure.duration_hours.is_finite()
                    || failure.duration_hours <= 0.0
            })
            || scenario.fan_faults.iter().any(|fault| {
                !matches!(
                    fault.equipment.as_str(),
                    "ahu_1_stuck_off"
                        | "ahu_1_stuck_on"
                        | "ahu_2_stuck_off"
                        | "ahu_2_stuck_on"
                        | "ahu_1_speed_capped"
                        | "ahu_2_speed_capped"
                ) || !fault.start_hour.is_finite()
                    || fault.start_hour < 0.0
                    || !fault.duration_hours.is_finite()
                    || fault.duration_hours <= 0.0
            })
            || scenario.fan_speed_sensor_events.iter().any(|event| {
                !(1..=2).contains(&event.ahu)
                    || !event.bias.is_finite()
                    || !(-1.0..=1.0).contains(&event.bias)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.water_pump_faults.iter().any(|fault| {
                !matches!(fault.loop_name.as_str(), "hot" | "chilled")
                    || !fault.start_hour.is_finite()
                    || fault.start_hour < 0.0
                    || !fault.duration_hours.is_finite()
                    || fault.duration_hours <= 0.0
            })
            || scenario.water_flow_sensor_events.iter().any(|event| {
                !matches!(event.loop_name.as_str(), "hot" | "chilled")
                    || !event.bias_kg_s.is_finite()
                    || !(-20.0..=20.0).contains(&event.bias_kg_s)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.terminal_faults.iter().any(|fault| {
                !(1..=10).contains(&fault.zone)
                    || !matches!(
                        fault.kind.as_str(),
                        "damper_stuck_open" | "reheat_valve_leak"
                    )
                    || fault.leak_fraction.is_some_and(|fraction| {
                        !fraction.is_finite() || !(0.0..=1.0).contains(&fraction)
                    })
                    || !fault.start_hour.is_finite()
                    || fault.start_hour < 0.0
                    || !fault.duration_hours.is_finite()
                    || fault.duration_hours <= 0.0
            })
            || scenario.terminal_valve_sensor_events.iter().any(|event| {
                !(1..=10).contains(&event.zone)
                    || !event.bias.is_finite()
                    || !(-1.0..=1.0).contains(&event.bias)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.ahu_damper_faults.iter().any(|fault| {
                !(1..=2).contains(&fault.ahu)
                    || !matches!(fault.damper.as_str(), "outdoor" | "return")
                    || !fault.stuck_position.is_finite()
                    || !(0.0..=1.0).contains(&fault.stuck_position)
                    || !fault.start_hour.is_finite()
                    || fault.start_hour < 0.0
                    || !fault.duration_hours.is_finite()
                    || fault.duration_hours <= 0.0
            })
            || scenario.ahu_coil_valve_faults.iter().any(|fault| {
                !(1..=2).contains(&fault.ahu)
                    || !matches!(fault.service.as_str(), "heating" | "cooling")
                    || !fault.stuck_position.is_finite()
                    || !(0.0..=1.0).contains(&fault.stuck_position)
                    || !fault.start_hour.is_finite()
                    || fault.start_hour < 0.0
                    || !fault.duration_hours.is_finite()
                    || fault.duration_hours <= 0.0
            })
            || scenario.ahu_damper_sensor_events.iter().any(|event| {
                !(1..=2).contains(&event.ahu)
                    || !matches!(event.damper.as_str(), "outdoor" | "return")
                    || !event.bias.is_finite()
                    || !(-1.0..=1.0).contains(&event.bias)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.ahu_coil_valve_sensor_events.iter().any(|event| {
                !(1..=2).contains(&event.ahu)
                    || !matches!(event.service.as_str(), "heating" | "cooling")
                    || !event.bias.is_finite()
                    || !(-1.0..=1.0).contains(&event.bias)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.ahu_mixed_air_sensor_events.iter().any(|event| {
                !(1..=2).contains(&event.ahu)
                    || !event.bias_c.is_finite()
                    || event.bias_c.abs() > 25.0
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.outdoor_temperature_events.iter().any(|event| {
                !event.delta_c.is_finite()
                    || event.delta_c.abs() > 25.0
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.window_openings.iter().any(|opening| {
                !(1..=10).contains(&opening.zone)
                    || !opening.start_hour.is_finite()
                    || opening.start_hour < 0.0
                    || !opening.duration_hours.is_finite()
                    || opening.duration_hours <= 0.0
            })
            || scenario.occupancy_overrides.iter().any(|override_event| {
                !(1..=10).contains(&override_event.zone)
                    || !override_event.start_hour.is_finite()
                    || override_event.start_hour < 0.0
                    || !override_event.duration_hours.is_finite()
                    || override_event.duration_hours <= 0.0
            })
            || scenario.occupancy_sensor_events.iter().any(|event| {
                !(1..=10).contains(&event.zone)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.occupant_count_events.iter().any(|event| {
                !(1..=10).contains(&event.zone)
                    || !event.count.is_finite()
                    || !(0.0..=1_000.0).contains(&event.count)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.occupancy_count_sensor_events.iter().any(|event| {
                !(1..=10).contains(&event.zone)
                    || !event.count.is_finite()
                    || !(0.0..=1_000.0).contains(&event.count)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.alarm_acknowledgements.iter().any(|event| {
                !(1..=10).contains(&event.zone)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.ahu_alarm_acknowledgements.iter().any(|event| {
                !(1..=2).contains(&event.ahu)
                    || !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || scenario.plant_alarm_acknowledgements.iter().any(|event| {
                !event.start_hour.is_finite()
                    || event.start_hour < 0.0
                    || !event.duration_hours.is_finite()
                    || event.duration_hours <= 0.0
            })
            || !scenario.heat_pump_fault_reset_hour.is_finite()
            || scenario.heat_pump_fault_reset_hour < -1.0
            || !scenario.boiler_fault_reset_hour.is_finite()
            || scenario.boiler_fault_reset_hour < -1.0
            || !scenario.chiller_fault_reset_hour.is_finite()
            || scenario.chiller_fault_reset_hour < -1.0
            || [
                scenario.equipment_startup_seconds.heat_pump,
                scenario.equipment_startup_seconds.boiler,
                scenario.equipment_startup_seconds.chiller,
            ]
            .iter()
            .any(|seconds| !seconds.is_finite() || *seconds < 0.0)
            || !scenario.boiler_capacity_kw.is_finite()
            || scenario.boiler_capacity_kw <= 0.0
            || !scenario.chiller_capacity_kw.is_finite()
            || scenario.chiller_capacity_kw <= 0.0
            || !matches!(scenario.step_seconds, 15 | 30 | 60 | 300 | 900)
            || scenario.freeze_test_ahu > 2
            || !scenario.freeze_test_start_hour.is_finite()
            || scenario.freeze_test_start_hour < -1.0
            || !scenario.freeze_test_duration_hours.is_finite()
            || scenario.freeze_test_duration_hours < 0.0
            || !scenario.freeze_test_reset_hour.is_finite()
            || scenario.freeze_test_reset_hour < -1.0
        {
            return Err(SimError::from_str(
                "Invalid building assumptions or simulation timestep",
            ));
        }
        let effective_step_seconds = scenario.step_seconds;
        let initial_co2_ppm = scenario.outdoor_co2_ppm;
        Ok(Self {
            weather,
            weather_relative_humidity,
            seconds_until_occupied,
            scenario,
            effective_step_seconds,
            prepared: None,
            temps: [22.0; 10],
            co2_ppm: [initial_co2_ppm; 10],
            zone_humidity_ratio: [humidity_ratio(22.0, 0.5); 10],
            interval_zone_condensate_kg: [0.0; 10],
            interval_zone_condensation_heat_kwh: [0.0; 10],
            interval_water_start_c: [45.0, 7.0],
            interval_water_source_kwh: [0.0; 2],
            interval_water_delivered_kwh: [0.0; 2],
            cursor: 0,
            electricity_kwh: 0.0,
            gas_kwh: 0.0,
            recorded_electricity_kwh: 0.0,
            recorded_gas_kwh: 0.0,
            unmet_hours: 0.0,
            occupied_unmet_hours: 0.0,
            last_heat_request: 0.0,
            last_cool_request: 0.0,
            last_flow_request: [0.0; 2],
            last_supply_c: [16.0; 2],
            last_mixed_air_c: [20.0; 2],
            last_measured_mixed_air_c: [20.0; 2],
            last_duct_pressure: [1.0; 2],
            last_outdoor_airflow: [0.0; 2],
            last_zone_airflow: [0.0; 10],
            last_fan_running: [false; 2],
            fan_start_elapsed_s: [0.0; 2],
            last_actual_fan_speed: [0.0; 2],
            last_measured_fan_speed: [0.0; 2],
            last_outdoor_damper_position: [0.0; 2],
            last_return_damper_position: [1.0; 2],
            last_measured_outdoor_damper_position: [0.0; 2],
            last_measured_return_damper_position: [1.0; 2],
            last_heating_coil_valve_position: [0.0; 2],
            last_cooling_coil_valve_position: [0.0; 2],
            last_measured_heating_coil_valve_position: [0.0; 2],
            last_measured_cooling_coil_valve_position: [0.0; 2],
            last_zone_discharge_c: [16.0; 10],
            last_zone_damper_position: [0.0; 10],
            last_zone_reheat_valve_position: [0.0; 10],
            last_zone_measured_reheat_valve_position: [0.0; 10],
            last_water_supply_c: [45.0, 7.0],
            last_water_return_c: [45.0, 7.0],
            last_water_flow_kg_s: [0.0; 2],
            last_water_dp_pa: [0.0; 2],
            last_water_pump_running: [false; 2],
            pump_start_elapsed_s: [0.0; 2],
            source_start_elapsed_s: [0.0; 3],
            source_running: [false; 3],
            last_hot_water_plant_proven: false,
            zone_occupied: [false; 10],
            raw_occupancy: [false; 10],
            actual_occupant_count: [0.0; 10],
            raw_occupant_count: [0.0; 10],
            synthetic_occupant_count: [0.0; 10],
            occupant_sensible_heat_kw: [0.0; 10],
        })
    }
}
