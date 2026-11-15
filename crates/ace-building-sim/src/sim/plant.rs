//! Controller-independent fixed-topology plant and one-step physical integration.
use super::*;
#[path = "plant_config.rs"]
mod config;
pub struct Plant {
    prepared: Option<PreparedStep>,
    pub(super) weather: Vec<Weather>,
    pub(super) weather_relative_humidity: Vec<f64>,
    pub(super) seconds_until_occupied: Vec<f64>,
    pub(super) scenario: Scenario,
    pub(super) effective_step_seconds: usize,
    pub(super) temps: [f64; 10],
    pub(super) co2_ppm: [f64; 10],
    pub(super) zone_humidity_ratio: [f64; 10],
    pub(super) interval_zone_condensate_kg: [f64; 10],
    pub(super) interval_zone_condensation_heat_kwh: [f64; 10],
    pub(super) interval_water_start_c: [f64; 2],
    pub(super) interval_water_source_kwh: [f64; 2],
    pub(super) interval_water_delivered_kwh: [f64; 2],
    pub(super) cursor: usize,
    pub(super) electricity_kwh: f64,
    pub(super) gas_kwh: f64,
    pub(super) recorded_electricity_kwh: f64,
    pub(super) recorded_gas_kwh: f64,
    pub(super) unmet_hours: f64,
    pub(super) occupied_unmet_hours: f64,
    pub(super) last_heat_request: f64,
    pub(super) last_cool_request: f64,
    pub(super) last_flow_request: [f64; 2],
    pub(super) last_supply_c: [f64; 2],
    pub(super) last_mixed_air_c: [f64; 2],
    pub(super) last_measured_mixed_air_c: [f64; 2],
    pub(super) last_duct_pressure: [f64; 2],
    pub(super) last_outdoor_airflow: [f64; 2],
    pub(super) last_zone_airflow: [f64; 10],
    pub(super) last_fan_running: [bool; 2],
    pub(super) fan_start_elapsed_s: [f64; 2],
    pub(super) last_actual_fan_speed: [f64; 2],
    pub(super) last_measured_fan_speed: [f64; 2],
    pub(super) last_outdoor_damper_position: [f64; 2],
    pub(super) last_return_damper_position: [f64; 2],
    pub(super) last_measured_outdoor_damper_position: [f64; 2],
    pub(super) last_measured_return_damper_position: [f64; 2],
    pub(super) last_heating_coil_valve_position: [f64; 2],
    pub(super) last_cooling_coil_valve_position: [f64; 2],
    pub(super) last_measured_heating_coil_valve_position: [f64; 2],
    pub(super) last_measured_cooling_coil_valve_position: [f64; 2],
    pub(super) last_zone_discharge_c: [f64; 10],
    pub(super) last_zone_damper_position: [f64; 10],
    pub(super) last_zone_reheat_valve_position: [f64; 10],
    pub(super) last_zone_measured_reheat_valve_position: [f64; 10],
    pub(super) last_water_supply_c: [f64; 2],
    pub(super) last_water_return_c: [f64; 2],
    pub(super) last_water_flow_kg_s: [f64; 2],
    pub(super) last_water_dp_pa: [f64; 2],
    pub(super) last_water_pump_running: [bool; 2],
    pub(super) pump_start_elapsed_s: [f64; 2],
    pub(super) source_start_elapsed_s: [f64; 3],
    pub(super) source_running: [bool; 3],
    pub(super) last_hot_water_plant_proven: bool,
    pub(super) zone_occupied: [bool; 10],
    pub(super) raw_occupancy: [bool; 10],
    pub(super) actual_occupant_count: [f64; 10],
    pub(super) raw_occupant_count: [f64; 10],
    pub(super) synthetic_occupant_count: [f64; 10],
    pub(super) occupant_sensible_heat_kw: [f64; 10],
}

pub(super) struct PreparedStep {
    pub(super) sensors: ControlInputs,
    index: usize,
    w: Weather,
    elapsed_hours: f64,
    outdoor_c: f64,
    equipment_status: [bool; 3],
    start_failure: [bool; 3],
    hp_heating_available_capacity_kw: f64,
    hp_cooling_available_capacity_kw: f64,
    heating_cop: f64,
    cooling_cop: f64,
    hp_available_capacity_kw: f64,
    return_rh_input: [f64; 2],
    window_closed: [bool; 10],
}

/// Sensor observations at the beginning of an integration interval.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlantObservation {
    /// Seconds elapsed since the start of the weather year.
    pub time_seconds: f64,
    /// Fixed integration interval selected in Scenario.
    pub step_seconds: usize,
    /// Sensor readings, calendar and exogenous equipment/environment inputs.
    pub sensors: PlantSensors,
}
/// Result of one physical integration interval.
#[derive(Clone, Debug, serde::Serialize)]
pub struct PlantStep {
    /// New simulation time in seconds.
    pub time_seconds: f64,
    /// Completed quarter-hour row, available only at recording boundaries.
    pub row: Option<SimulationRow>,
    /// Whether the supplied weather year has completed.
    pub done: bool,
}
impl Plant {
    /// Read prepared observations without advancing time. Repeated calls return
    /// the same interval; sensor events/presence are prepared only once.
    pub fn observe(&mut self) -> Result<PlantObservation, SimError> {
        self.ensure_prepared()?;
        Ok(PlantObservation {
            time_seconds: self.cursor as f64 * self.effective_step_seconds as f64,
            step_seconds: self.effective_step_seconds,
            sensors: self.prepared.as_ref().unwrap().sensors.clone(),
        })
    }
    /// Advance physical state with externally supplied commands. dt must match
    /// the configured interval. Reporting affects metrics only. Invalid commands
    /// or durations are rejected before consuming observations or changing physics.
    pub fn step(
        &mut self,
        commands: &PlantCommands,
        dt_seconds: f64,
    ) -> Result<PlantStep, SimError> {
        self.step_reported(commands, &ReportingContext::default(), dt_seconds)
    }
    /// Advance the plant with separate controller reporting status. Reporting is
    /// recorded alongside physical data; it does not change physical occupancy.
    pub fn step_reported(
        &mut self,
        commands: &PlantCommands,
        reporting: &ReportingContext,
        dt_seconds: f64,
    ) -> Result<PlantStep, SimError> {
        if dt_seconds != self.effective_step_seconds as f64 {
            return Err(SimError(
                "dt must match the plant integration interval".into(),
            ));
        }
        commands.validate()?;
        let row = self.advance_commands(commands, reporting, std::collections::BTreeMap::new())?;
        Ok(PlantStep {
            time_seconds: self.cursor as f64 * dt_seconds,
            row,
            done: self.done(),
        })
    }
    pub(super) fn ensure_prepared(&mut self) -> Result<(), SimError> {
        if self.done() {
            return Err(SimError("plant weather year is complete".into()));
        }
        if self.prepared.is_none() {
            self.prepared = Some(self.prepare());
        }
        Ok(())
    }
    pub(super) fn sensors(&self) -> &PlantSensors {
        &self.prepared.as_ref().unwrap().sensors
    }
    fn prepare(&mut self) -> PreparedStep {
        let steps_per_weather = 900 / self.effective_step_seconds;
        let dt_seconds = self.effective_step_seconds as f64;
        let dt_hours = dt_seconds / 3600.;
        let index = self.cursor / steps_per_weather;
        let w = self.weather[index].clone();
        let elapsed_hours = self.cursor as f64 * dt_hours;
        let outdoor_c = w.outdoor_c
            + self
                .scenario
                .outdoor_temperature_events
                .iter()
                .filter(|event| {
                    elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                })
                .map(|event| event.delta_c)
                .sum::<f64>();
        let equipment_status = ["heat_pump", "boiler", "chiller"]
            .map(|name| equipment_available(&self.scenario.equipment_outages, name, elapsed_hours));
        let start_failure = ["heat_pump", "boiler", "chiller"].map(|name| {
            !equipment_available(&self.scenario.equipment_start_failures, name, elapsed_hours)
        });
        let mut control_inputs = ControlInputs::default();
        control_inputs.global.outdoor_c = outdoor_c;

        control_inputs.global.outdoor_air_enthalpy_kj_kg = outdoor_air_enthalpy_kj_kg(
            outdoor_c,
            self.scenario
                .outdoor_relative_humidity
                .unwrap_or(self.weather_relative_humidity[index]),
        );

        control_inputs.global.pollution_operator = (self.scenario.pollution_operator) as f64;
        control_inputs.global.pollution_on_duration_s = self.scenario.pollution_on_duration_s;
        // A wall-clock reading is a raw input. The reminder threshold,
        // daily edge, active-mode gate, and alarm level remain in CDL.
        control_inputs.global.local_day_seconds = local_day_seconds(
            elapsed_hours,
            self.scenario.local_utc_offset_hours,
            &self.scenario.clock_offset_events,
        );
        for (pollutant, (name, baseline)) in [
            ("pm25", self.scenario.outdoor_pm25_ug_m3),
            ("ozone", self.scenario.outdoor_ozone_ppm),
            ("aqi", self.scenario.outdoor_aqi),
        ]
        .into_iter()
        .enumerate()
        {
            let reading = self
                .scenario
                .air_quality_events
                .iter()
                .rev()
                .find(|event| {
                    event.pollutant == name
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                })
                .map_or(baseline, |event| event.value);
            control_inputs.pollution[pollutant].value = reading.unwrap_or(0.0);
            control_inputs.pollution[pollutant].available = reading.is_some();
        }
        control_inputs.global.schedule_occupied = w.occupied;
        control_inputs.global.seconds_until_occupied = (self.seconds_until_occupied[index]
            - (self.cursor % steps_per_weather) as f64 * dt_seconds)
            .max(0.0);
        control_inputs.global.heat_request_kw = self.last_heat_request;
        control_inputs.global.cool_request_kw = self.last_cool_request;
        for (source, available) in equipment_status.into_iter().enumerate() {
            control_inputs.equipment[source].available = available;
        }
        // Running status is measured at the end of the preceding physical step.
        for (source, running) in self.source_running.into_iter().enumerate() {
            control_inputs.equipment[source].running = running;
        }
        control_inputs.global.hot_water_plant_proven = self.last_hot_water_plant_proven;
        for (source, reset_hour) in [
            self.scenario.heat_pump_fault_reset_hour,
            self.scenario.boiler_fault_reset_hour,
            self.scenario.chiller_fault_reset_hour,
        ]
        .into_iter()
        .enumerate()
        {
            control_inputs.equipment[source].fault_reset = reset_hour >= 0.0
                && elapsed_hours <= reset_hour
                && elapsed_hours + dt_hours > reset_hour;
        }
        let plant_alarm_acknowledge =
            self.scenario
                .plant_alarm_acknowledgements
                .iter()
                .any(|event| {
                    elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                });
        control_inputs.global.plant_alarm_acknowledge = plant_alarm_acknowledge;
        let joint_full_load = self
            .scenario
            .heat_pump_joint_performance
            .as_ref()
            .map(|map| {
                heat_pump_joint_value(
                    map,
                    outdoor_c,
                    self.last_water_supply_c[0],
                    self.last_water_supply_c[1],
                    1.0,
                    None,
                )
            });
        let (
            hp_heating_available_capacity_kw,
            hp_cooling_available_capacity_kw,
            heating_cop,
            cooling_cop,
        ) = if let Some(value) = &joint_full_load {
            (
                self.scenario.heat_pump_capacity_kw * value.heating_capacity_fraction,
                self.scenario.heat_pump_capacity_kw * value.cooling_capacity_fraction,
                value.heating_cop,
                value.cooling_cop,
            )
        } else {
            let (fraction, heating_cop, cooling_cop) =
                heat_pump_performance(&self.scenario.heat_pump_performance, outdoor_c);
            let outdoor_capacity_kw = self.scenario.heat_pump_capacity_kw * fraction;
            (
                outdoor_capacity_kw
                    * water_capacity_multiplier(
                        &self.scenario.heat_pump_hot_water_cop_performance,
                        self.last_water_supply_c[0],
                    ),
                outdoor_capacity_kw
                    * water_capacity_multiplier(
                        &self.scenario.heat_pump_chilled_water_cop_performance,
                        self.last_water_supply_c[1],
                    ),
                heating_cop,
                cooling_cop,
            )
        };
        // The common compressor envelope covers the larger available
        // service capacity. Separate limits still govern the heating
        // and cooling delivery commanded by CDL.
        let hp_available_capacity_kw =
            hp_heating_available_capacity_kw.max(hp_cooling_available_capacity_kw);
        control_inputs.global.heat_pump_available_capacity_kw = hp_available_capacity_kw;
        control_inputs
            .global
            .heat_pump_heating_available_capacity_kw = hp_heating_available_capacity_kw;
        control_inputs
            .global
            .heat_pump_cooling_available_capacity_kw = hp_cooling_available_capacity_kw;
        control_inputs.global.hot_water_supply_temp_c = self.last_water_supply_c[0];
        control_inputs.global.hot_water_return_temp_c = self.last_water_return_c[0];
        control_inputs.global.chilled_water_supply_temp_c = self.last_water_supply_c[1];
        control_inputs.global.chilled_water_return_temp_c = self.last_water_return_c[1];
        control_inputs.global.hot_water_flow_kg_s = measured_water_flow(
            self.last_water_flow_kg_s[0],
            "hot",
            elapsed_hours,
            &self.scenario.water_flow_sensor_events,
        );
        control_inputs.global.chilled_water_flow_kg_s = measured_water_flow(
            self.last_water_flow_kg_s[1],
            "chilled",
            elapsed_hours,
            &self.scenario.water_flow_sensor_events,
        );
        control_inputs.global.hot_water_pump_running = self.last_water_pump_running[0];
        control_inputs.global.chilled_water_pump_running = self.last_water_pump_running[1];
        control_inputs.global.hot_water_differential_pressure_pa = self.last_water_dp_pa[0];
        control_inputs.global.chilled_water_differential_pressure_pa = self.last_water_dp_pa[1];
        control_inputs.ahu[0].flow_request = self.last_flow_request[0];
        control_inputs.ahu[1].flow_request = self.last_flow_request[1];
        control_inputs.ahu[0].actual_supply_air_temp = self.last_supply_c[0];
        control_inputs.ahu[1].actual_supply_air_temp = self.last_supply_c[1];
        let next_hours = elapsed_hours + dt_seconds / 3600.0;
        let mut return_rh_input = [0.5; 2];
        for ahu in 0..2 {
            let alarm_acknowledge = self
                .scenario
                .ahu_alarm_acknowledgements
                .iter()
                .any(|event| {
                    event.ahu == ahu + 1
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                });
            control_inputs.ahu[ahu].alarm_acknowledge = alarm_acknowledge;
            control_inputs.ahu[ahu].supply_fan_proven = self.last_fan_running[ahu];
            control_inputs.ahu[ahu].actual_fan_speed = self.last_measured_fan_speed[ahu];
            control_inputs.ahu[ahu].actual_outdoor_damper_position =
                self.last_measured_outdoor_damper_position[ahu];
            control_inputs.ahu[ahu].actual_return_damper_position =
                self.last_measured_return_damper_position[ahu];
            control_inputs.ahu[ahu].actual_heating_coil_valve_position =
                self.last_measured_heating_coil_valve_position[ahu];
            control_inputs.ahu[ahu].actual_cooling_coil_valve_position =
                self.last_measured_cooling_coil_valve_position[ahu];
            let selected = self.scenario.freeze_test_ahu == ahu + 1;
            let fault_start = self.scenario.freeze_test_start_hour;
            control_inputs.ahu[ahu].freeze_stat_trip = selected
                && fault_start >= 0.0
                && self.scenario.freeze_test_duration_hours > 0.0
                && elapsed_hours >= fault_start
                && elapsed_hours < fault_start + self.scenario.freeze_test_duration_hours;
            let reset_hour = self.scenario.freeze_test_reset_hour;
            control_inputs.ahu[ahu].freeze_reset = selected
                && reset_hour >= 0.0
                && elapsed_hours <= reset_hour
                && next_hours > reset_hour;
            control_inputs.ahu[ahu].mixed_air_temp = self.last_measured_mixed_air_c[ahu];
            control_inputs.ahu[ahu].duct_pressure = self.last_duct_pressure[ahu];
            control_inputs.ahu[ahu].outdoor_airflow = self.last_outdoor_airflow[ahu];
            let return_c = ahu_return_value(&self.temps, &self.last_zone_airflow, ahu);
            control_inputs.ahu[ahu].return_air_temp = return_c;
            let return_humidity = self
                .scenario
                .return_air_relative_humidity
                .map(|rh| humidity_ratio(return_c, rh))
                .unwrap_or_else(|| {
                    ahu_return_value(&self.zone_humidity_ratio, &self.last_zone_airflow, ahu)
                });
            return_rh_input[ahu] = self
                .scenario
                .return_air_relative_humidity
                .unwrap_or_else(|| relative_humidity_from_ratio(return_c, return_humidity));

            let return_enthalpy = if self.scenario.return_air_relative_humidity.is_some() {
                moist_air_enthalpy_kj_kg(return_c, return_humidity)
            } else {
                ahu_return_enthalpy_kj_kg(
                    &self.temps,
                    &self.zone_humidity_ratio,
                    &self.last_zone_airflow,
                    ahu,
                )
            };
            control_inputs.ahu[ahu].return_air_enthalpy_kj_kg = return_enthalpy;
        }
        let mut window_closed = [true; 10];
        for zone in 0..10 {
            let synthetic_present = raw_zone_occupancy(index, zone, w.occupied);
            self.synthetic_occupant_count[zone] = if synthetic_present {
                default_zone_occupants()
            } else {
                0.0
            };
            let actual_event = self
                .scenario
                .occupant_count_events
                .iter()
                .rev()
                .find(|event| {
                    event.zone == zone + 1
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                });
            let actual_count = actual_event.map_or(
                if synthetic_present {
                    self.scenario.design_occupants(zone)
                } else {
                    0.0
                },
                |event| event.count,
            );
            self.actual_occupant_count[zone] = actual_count;
            let sensed_count = self
                .scenario
                .occupancy_count_sensor_events
                .iter()
                .rev()
                .find(|event| {
                    event.zone == zone + 1
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                })
                .map_or(actual_count, |event| event.count);
            self.raw_occupant_count[zone] = sensed_count;
            self.occupant_sensible_heat_kw[zone] = (actual_count
                - self.synthetic_occupant_count[zone])
                * self.scenario.occupant_sensible_heat_kw_per_person;
            let raw = self
                .scenario
                .occupancy_sensor_events
                .iter()
                .rev()
                .find(|event| {
                    event.zone == zone + 1
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                })
                .map_or(
                    actual_event.map_or(synthetic_present, |event| event.count > 0.0),
                    |event| event.present,
                );
            self.raw_occupancy[zone] = raw;
            control_inputs.zone[zone].occupied = raw;
            control_inputs.zone[zone].occupancy_count = sensed_count;
            window_closed[zone] = !self.scenario.window_openings.iter().any(|opening| {
                opening.zone == zone + 1
                    && elapsed_hours >= opening.start_hour
                    && elapsed_hours < opening.start_hour + opening.duration_hours
            });
            control_inputs.zone[zone].window_closed = window_closed[zone];
            let local_override = self.scenario.occupancy_overrides.iter().any(|event| {
                event.zone == zone + 1
                    && elapsed_hours >= event.start_hour
                    && elapsed_hours < event.start_hour + event.duration_hours
            });
            control_inputs.zone[zone].local_override = local_override;
            let alarm_acknowledge = self.scenario.alarm_acknowledgements.iter().any(|event| {
                event.zone == zone + 1
                    && elapsed_hours >= event.start_hour
                    && elapsed_hours < event.start_hour + event.duration_hours
            });
            control_inputs.zone[zone].alarm_acknowledge = alarm_acknowledge;
            // Older saved projects may still use the former low-flow-only
            // raw contact name in custom wiring.
            control_inputs.zone[zone].low_airflow_acknowledge = alarm_acknowledge;
            control_inputs.zone[zone].room_temp = self.temps[zone];
            control_inputs.zone[zone].co2_ppm = self.co2_ppm[zone];
            control_inputs.zone[zone].measured_airflow = self.last_zone_airflow[zone];
            control_inputs.zone[zone].discharge_temp = self.last_zone_discharge_c[zone];
            control_inputs.zone[zone].actual_damper_position = self.last_zone_damper_position[zone];
            control_inputs.zone[zone].actual_reheat_valve_position =
                self.last_zone_measured_reheat_valve_position[zone];
            control_inputs.zone[zone].supply_pressure = self.last_duct_pressure[zone / 5];
        }
        PreparedStep {
            sensors: control_inputs,
            index,
            w,
            elapsed_hours,
            outdoor_c,
            equipment_status,
            start_failure,
            hp_heating_available_capacity_kw,
            hp_cooling_available_capacity_kw,
            heating_cop,
            cooling_cop,
            hp_available_capacity_kw,
            return_rh_input,
            window_closed,
        }
    }

    pub(super) fn advance_commands(
        &mut self,
        commands: &PlantCommands,
        reporting: &ReportingContext,
        traces: std::collections::BTreeMap<String, f64>,
    ) -> Result<Option<SimulationRow>, SimError> {
        self.ensure_prepared()?;
        let PreparedStep {
            sensors: _,
            index,
            w,
            elapsed_hours,
            outdoor_c,
            equipment_status,
            start_failure,
            hp_heating_available_capacity_kw,
            hp_cooling_available_capacity_kw,
            heating_cop,
            cooling_cop,
            hp_available_capacity_kw,
            return_rh_input,
            window_closed,
        } = self.prepared.take().unwrap();
        let dt_seconds = self.effective_step_seconds as f64;
        let dt_hours = dt_seconds / 3600.;
        let steps_per_weather = 900 / self.effective_step_seconds;
        let controls = commands;
        let mut cool_request = 0.;
        let mut heat_request = 0.;
        let mut requested_flow = [0.; 10];
        let mut damper_command = [0.; 10];
        let mut requested_total = [0.; 2];
        let mut reheat_command = [0.; 10];
        let mut result = None;
        let preconditioning = reporting.preconditioning_mode;
        for zone in 0..10 {
            self.zone_occupied[zone] = reporting.occupancy_confirmed[zone];
        }
        let hot_water_enabled = controls.global.plant_hot_water_enable;
        let chilled_water_enabled = controls.global.plant_chilled_water_enable;
        let hot_water_pump_speed = clamp(controls.global.plant_hot_water_pump_speed, 0.0, 1.0);
        let chilled_water_pump_speed =
            clamp(controls.global.plant_chilled_water_pump_speed, 0.0, 1.0);
        let water_setpoint_c = [
            controls.global.plant_hot_water_supply_setpoint_c,
            controls.global.plant_chilled_water_supply_setpoint_c,
        ];
        let water_supply_c = self.last_water_supply_c;
        if self.cursor % steps_per_weather == 0 {
            self.interval_water_start_c = water_supply_c;
        }
        // The physical machine accepts only the services selected by the
        // editable CDL mode output; the harness does not choose a mode.
        let heat_pump_mode = controls.global.plant_heat_pump_requested_mode;
        if ![0.0, 1.0, 2.0, 3.0].contains(&heat_pump_mode) {
            return Err(SimError::from_str(
                    "CDL plant_heat_pump_requested_mode must be 0 off, 1 heating, 2 cooling, or 3 simultaneous",
                ));
        }
        let hp_heat_command = if heat_pump_mode == 1.0 || heat_pump_mode == 3.0 {
            controls.global.plant_heat_pump_heating_kw.max(0.0)
        } else {
            0.0
        };
        let hp_cool_command = if heat_pump_mode == 2.0 || heat_pump_mode == 3.0 {
            controls.global.plant_heat_pump_cooling_kw.max(0.0)
        } else {
            0.0
        };
        let boiler_command = controls.global.plant_boiler_kw.max(0.0);
        let chiller_command = controls.global.plant_chiller_kw.max(0.0);
        let source_start_commands = [
            controls.global.plant_heat_pump_start_command,
            controls.global.plant_boiler_start_command,
            controls.global.plant_chiller_start_command,
        ];
        let source_start_faults = [
            controls.global.plant_heat_pump_start_fault,
            controls.global.plant_boiler_start_fault,
            controls.global.plant_chiller_start_fault,
        ];
        let startup_seconds = [
            self.scenario.equipment_startup_seconds.heat_pump,
            self.scenario.equipment_startup_seconds.boiler,
            self.scenario.equipment_startup_seconds.chiller,
        ];
        for source in 0..3 {
            if equipment_status[source] && !start_failure[source] && source_start_commands[source] {
                self.source_start_elapsed_s[source] += dt_seconds;
                self.source_running[source] =
                    self.source_start_elapsed_s[source] >= startup_seconds[source];
            } else {
                self.source_start_elapsed_s[source] = 0.0;
                self.source_running[source] = false;
            }
        }
        let physical_hp_capacity_kw = if self.source_running[0] {
            hp_available_capacity_kw
        } else {
            0.0
        };
        let hp_command_limit = if hp_heat_command + hp_cool_command > 0.0 {
            (physical_hp_capacity_kw / (hp_heat_command + hp_cool_command)).min(1.0)
        } else {
            1.0
        };
        let hp_heat = (hp_heat_command * hp_command_limit).min(if self.source_running[0] {
            hp_heating_available_capacity_kw
        } else {
            0.0
        });
        let hp_cool = (hp_cool_command * hp_command_limit).min(if self.source_running[0] {
            hp_cooling_available_capacity_kw
        } else {
            0.0
        });
        let boiler = boiler_command.min(if self.source_running[1] {
            self.scenario.boiler_capacity_kw
        } else {
            0.0
        });
        let chiller = chiller_command.min(if self.source_running[2] {
            self.scenario.chiller_capacity_kw
        } else {
            0.0
        });
        let plant_heat = hp_heat + boiler;
        let plant_cool = hp_cool + chiller;
        let supply_setpoint = [
            controls.ahu[0].supply_air_temp,
            controls.ahu[1].supply_air_temp,
        ];
        let fan_speed = [controls.ahu[0].fan_speed, controls.ahu[1].fan_speed];
        // Motor status is physical feedback, distinct from the CDL command
        // and from delivered terminal airflow. Fault intervals affect the
        // motor and air/energy physics; only CDL decides the alarm response.
        let actual_fan_speed = [0, 1].map(|ahu| {
            let stopped = !equipment_available(
                &self.scenario.fan_faults,
                &format!("ahu_{}_stuck_off", ahu + 1),
                elapsed_hours,
            );
            let stuck_on = !equipment_available(
                &self.scenario.fan_faults,
                &format!("ahu_{}_stuck_on", ahu + 1),
                elapsed_hours,
            );
            let speed_capped = !equipment_available(
                &self.scenario.fan_faults,
                &format!("ahu_{}_speed_capped", ahu + 1),
                elapsed_hours,
            );
            let speed = if stopped {
                0.0
            } else if stuck_on {
                clamp(fan_speed[ahu], 0.2, 1.0)
            } else if speed_capped {
                clamp(fan_speed[ahu], 0.0, 0.3)
            } else {
                clamp(fan_speed[ahu], 0.0, 1.0)
            };
            // Motor start is physical feedback. The CDL fan command stays
            // unchanged while proof and delivered air wait for the motor.
            let actual = if speed > 0.01 {
                self.fan_start_elapsed_s[ahu] += dt_seconds;
                if self.fan_start_elapsed_s[ahu] >= self.scenario.ahu_fan_startup_seconds[ahu] {
                    speed
                } else {
                    0.0
                }
            } else {
                self.fan_start_elapsed_s[ahu] = 0.0;
                speed
            };
            self.last_fan_running[ahu] = actual > 0.01;
            actual
        });
        self.last_actual_fan_speed = actual_fan_speed;
        // The sensor can be biased without changing physical fan speed,
        // airflow, or energy. CDL decides what the discrepancy means.
        self.last_measured_fan_speed = [0, 1].map(|ahu| {
            let bias = self
                .scenario
                .fan_speed_sensor_events
                .iter()
                .rev()
                .find(|event| {
                    event.ahu == ahu + 1
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                })
                .map_or(0.0, |event| event.bias);
            clamp(actual_fan_speed[ahu] + bias, 0.0, 1.0)
        });
        let mut outdoor_air_fraction = [0.0; 2];
        let mut intake_opening = [1.0; 2];
        let mut actual_outdoor_damper_position = [0.0; 2];
        let mut actual_return_damper_position = [1.0; 2];
        for (ahu, fraction) in outdoor_air_fraction.iter_mut().enumerate() {
            let stroke = dt_seconds / self.scenario.ahu_damper_stroke_seconds;
            let outdoor_command = clamp(controls.ahu[ahu].outdoor_air_damper_position, 0.0, 1.0);
            let return_command = clamp(controls.ahu[ahu].return_air_damper_position, 0.0, 1.0);
            let outdoor = ahu_damper_position(
                &self.scenario.ahu_damper_faults,
                ahu + 1,
                "outdoor",
                elapsed_hours,
                clamp(
                    outdoor_command,
                    self.last_outdoor_damper_position[ahu] - stroke,
                    self.last_outdoor_damper_position[ahu] + stroke,
                ),
            );
            let return_air = ahu_damper_position(
                &self.scenario.ahu_damper_faults,
                ahu + 1,
                "return",
                elapsed_hours,
                clamp(
                    return_command,
                    self.last_return_damper_position[ahu] - stroke,
                    self.last_return_damper_position[ahu] + stroke,
                ),
            );
            actual_outdoor_damper_position[ahu] = outdoor;
            actual_return_damper_position[ahu] = return_air;
            let open_area = outdoor + return_air;
            intake_opening[ahu] = clamp(open_area, 0.0, 1.0);
            *fraction = if open_area > 0.0 {
                outdoor / open_area
            } else {
                0.0
            };
        }
        self.last_outdoor_damper_position = actual_outdoor_damper_position;
        self.last_return_damper_position = actual_return_damper_position;
        for ahu in 0..2 {
            for (damper, actual, measured) in [
                (
                    "outdoor",
                    actual_outdoor_damper_position[ahu],
                    &mut self.last_measured_outdoor_damper_position[ahu],
                ),
                (
                    "return",
                    actual_return_damper_position[ahu],
                    &mut self.last_measured_return_damper_position[ahu],
                ),
            ] {
                let bias = self
                    .scenario
                    .ahu_damper_sensor_events
                    .iter()
                    .rev()
                    .find(|event| {
                        event.ahu == ahu + 1
                            && event.damper == damper
                            && elapsed_hours >= event.start_hour
                            && elapsed_hours < event.start_hour + event.duration_hours
                    })
                    .map_or(0.0, |event| event.bias);
                *measured = clamp(actual + bias, 0.0, 1.0);
            }
        }
        let heating_coil_command = [
            clamp(controls.ahu[0].heating_coil_valve_position, 0.0, 1.0),
            clamp(controls.ahu[1].heating_coil_valve_position, 0.0, 1.0),
        ];
        let cooling_coil_command = [
            clamp(controls.ahu[0].cooling_coil_valve_position, 0.0, 1.0),
            clamp(controls.ahu[1].cooling_coil_valve_position, 0.0, 1.0),
        ];
        let heating_coil_valve = [0, 1].map(|ahu| {
            let stroke = self.scenario.ahu_heating_coil_valve_stroke_seconds[ahu];
            let traveled = if stroke == 0.0 {
                heating_coil_command[ahu]
            } else {
                let travel = dt_seconds / stroke;
                clamp(
                    heating_coil_command[ahu],
                    self.last_heating_coil_valve_position[ahu] - travel,
                    self.last_heating_coil_valve_position[ahu] + travel,
                )
            };
            ahu_coil_valve_position(
                &self.scenario.ahu_coil_valve_faults,
                ahu + 1,
                "heating",
                elapsed_hours,
                traveled,
            )
        });
        let cooling_coil_valve = [0, 1].map(|ahu| {
            let stroke = self.scenario.ahu_cooling_coil_valve_stroke_seconds[ahu];
            let traveled = if stroke == 0.0 {
                cooling_coil_command[ahu]
            } else {
                let travel = dt_seconds / stroke;
                clamp(
                    cooling_coil_command[ahu],
                    self.last_cooling_coil_valve_position[ahu] - travel,
                    self.last_cooling_coil_valve_position[ahu] + travel,
                )
            };
            ahu_coil_valve_position(
                &self.scenario.ahu_coil_valve_faults,
                ahu + 1,
                "cooling",
                elapsed_hours,
                traveled,
            )
        });
        self.last_heating_coil_valve_position = heating_coil_valve;
        self.last_cooling_coil_valve_position = cooling_coil_valve;
        for ahu in 0..2 {
            for (service, actual, measured) in [
                (
                    "heating",
                    heating_coil_valve[ahu],
                    &mut self.last_measured_heating_coil_valve_position[ahu],
                ),
                (
                    "cooling",
                    cooling_coil_valve[ahu],
                    &mut self.last_measured_cooling_coil_valve_position[ahu],
                ),
            ] {
                let bias = self
                    .scenario
                    .ahu_coil_valve_sensor_events
                    .iter()
                    .rev()
                    .find(|event| {
                        event.ahu == ahu + 1
                            && event.service == service
                            && elapsed_hours >= event.start_hour
                            && elapsed_hours < event.start_hour + event.duration_hours
                    })
                    .map_or(0.0, |event| event.bias);
                *measured = clamp(actual + bias, 0.0, 1.0);
            }
        }
        for zone in 0..10 {
            let ahu = zone / 5;
            requested_flow[zone] = controls.zone[zone].airflow_request.max(0.0);
            damper_command[zone] = clamp(controls.zone[zone].damper_position, 0.0, 1.0);
            reheat_command[zone] = clamp(controls.zone[zone].reheat_valve_position, 0.0, 1.0);
            requested_total[ahu] += requested_flow[zone];
        }
        // The harness applies physical actuator faults after reading CDL
        // commands. A stuck damper changes actual opening and airflow. A
        // leaking valve passes water with its actuator still closed.
        let actual_damper_position = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map(|zone| {
            if terminal_fault_active(
                &self.scenario.terminal_faults,
                zone + 1,
                "damper_stuck_open",
                elapsed_hours,
            ) {
                1.0
            } else {
                damper_command[zone]
            }
        });
        let actual_reheat_valve_position = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map(|zone| {
            let stroke = self.scenario.terminal_reheat_valve_stroke_seconds[zone];
            if stroke == 0.0 {
                reheat_command[zone]
            } else {
                let travel = dt_seconds / stroke;
                clamp(
                    reheat_command[zone],
                    self.last_zone_reheat_valve_position[zone] - travel,
                    self.last_zone_reheat_valve_position[zone] + travel,
                )
            }
        });
        let effective_reheat_opening = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map(|zone| {
            if let Some(leak_fraction) = terminal_reheat_leak_fraction(
                &self.scenario.terminal_faults,
                zone + 1,
                elapsed_hours,
            ) {
                actual_reheat_valve_position[zone].max(leak_fraction)
            } else {
                actual_reheat_valve_position[zone]
            }
        });
        // Two parallel valve groups and a small bypass form a simplified
        // hydraulic network. The remote differential pressure follows a
        // quadratic pump curve and resistance law; opening valves raises
        // flow and lowers pressure at a fixed pump speed.
        let hot_valve_area = self.scenario.water_bypass_conductance[0]
            + self.scenario.water_coil_conductance[0] * heating_coil_valve.iter().sum::<f64>()
                / 2.0
            + self.scenario.hot_reheat_conductance * effective_reheat_opening.iter().sum::<f64>()
                / 10.0;
        let chilled_valve_area = self.scenario.water_bypass_conductance[1]
            + self.scenario.water_coil_conductance[1] * cooling_coil_valve.iter().sum::<f64>()
                / 2.0;
        let valve_area = [hot_valve_area, chilled_valve_area];
        let pump_speed = [hot_water_pump_speed, chilled_water_pump_speed];
        let actual_pump_speed = [0, 1].map(|loop_index| {
            let loop_name = if loop_index == 0 { "hot" } else { "chilled" };
            let speed = if self.scenario.water_pump_faults.iter().any(|fault| {
                fault.loop_name == loop_name
                    && elapsed_hours >= fault.start_hour
                    && elapsed_hours < fault.start_hour + fault.duration_hours
            }) {
                0.0
            } else {
                pump_speed[loop_index]
            };
            if speed > 0.0 {
                self.pump_start_elapsed_s[loop_index] += dt_seconds;
                if self.pump_start_elapsed_s[loop_index]
                    >= self.scenario.water_pump_startup_seconds[loop_index]
                {
                    speed
                } else {
                    0.0
                }
            } else {
                self.pump_start_elapsed_s[loop_index] = 0.0;
                0.0
            }
        });
        let hydraulics = [0, 1].map(|loop_index| {
            water_loop_hydraulics(
                actual_pump_speed[loop_index],
                valve_area[loop_index],
                self.scenario.water_pump_design_flow_kg_s[loop_index],
                self.scenario.water_pump_shutoff_pressure_pa[loop_index],
            )
        });
        let water_dp_pa = hydraulics.map(|(pressure, _)| pressure);
        let water_flow_kg_s = hydraulics.map(|(_, flow)| flow);
        // The harness resolves a simple fan/terminal air network from CDL
        // actuator commands. Airflow setpoints do not directly deliver air.
        // At pressure 1 each fully open terminal passes its configured
        // physical design airflow; each AHU has its own full-speed fan
        // capacity. CDL independently chooses airflow requests.
        let damper_area = [0, 1].map(|ahu| {
            (ahu * 5..ahu * 5 + 5)
                .map(|zone| self.scenario.terminal_design_flow[zone] * actual_damper_position[zone])
                .sum::<f64>()
        });
        let duct_pressure = [0, 1].map(|ahu| {
            if damper_area[ahu] > 0.0 {
                clamp(
                    (self.scenario.ahu_fan_design_airflow[ahu]
                        * actual_fan_speed[ahu]
                        * intake_opening[ahu]
                        / damper_area[ahu])
                        .powi(2),
                    0.0,
                    1.0,
                )
            } else if actual_fan_speed[ahu] > 0.0 && intake_opening[ahu] > 0.0 {
                1.0
            } else {
                0.0
            }
        });
        let mut delivered_flow = [0.0_f64; 10];
        let mut delivered_by_ahu = [0.0_f64; 2];
        let mut reheat_request = [0.0_f64; 10];
        for zone in 0..10 {
            let ahu = zone / 5;
            delivered_flow[zone] = self.scenario.terminal_design_flow[zone]
                * actual_damper_position[zone]
                * duct_pressure[ahu].sqrt();
            delivered_by_ahu[ahu] += delivered_flow[zone];
            let discharge_limit_kw =
                delivered_flow[zone] * 1.2 * (40.0 - supply_setpoint[ahu]).max(0.0);
            reheat_request[zone] = (effective_reheat_opening[zone] * 8.0).min(discharge_limit_kw);
            heat_request += reheat_request[zone];
        }
        let outdoor_humidity_ratio = humidity_ratio(
            outdoor_c,
            self.scenario
                .outdoor_relative_humidity
                .unwrap_or(self.weather_relative_humidity[index]),
        );
        let return_humidity_ratio =
            [0, 1].map(|ahu| ahu_return_value(&self.zone_humidity_ratio, &delivered_flow, ahu));
        let mixed_humidity_ratio = [0, 1].map(|ahu| {
            outdoor_air_fraction[ahu] * outdoor_humidity_ratio
                + (1.0 - outdoor_air_fraction[ahu]) * return_humidity_ratio[ahu]
        });
        let mut mixed_air_c = [0.0_f64; 2];
        let physical_return_air_c =
            [0, 1].map(|ahu| ahu_return_value(&self.temps, &delivered_flow, ahu));
        let physical_return_enthalpy_kj_kg = [0, 1].map(|ahu| {
            ahu_return_enthalpy_kj_kg(&self.temps, &self.zone_humidity_ratio, &delivered_flow, ahu)
        });
        let outdoor_enthalpy_kj_kg = moist_air_enthalpy_kj_kg(outdoor_c, outdoor_humidity_ratio);
        let outdoor_airflow = [0, 1].map(|ahu| delivered_by_ahu[ahu] * outdoor_air_fraction[ahu]);
        let mut coil_heat_request = [0.0_f64; 2];
        let mut coil_cool_request = [0.0_f64; 2];
        let mut coil_cool_target_c = [0.0_f64; 2];
        for ahu in 0..2 {
            let mixed_enthalpy_kj_kg = (1.0 - outdoor_air_fraction[ahu])
                * physical_return_enthalpy_kj_kg[ahu]
                + outdoor_air_fraction[ahu] * outdoor_enthalpy_kj_kg;
            mixed_air_c[ahu] = temperature_from_moist_air_enthalpy_c(
                mixed_enthalpy_kj_kg,
                mixed_humidity_ratio[ahu],
            );
            let coil_capacity_kw = delivered_by_ahu[ahu] * 1.2 * 20.0;
            coil_heat_request[ahu] = (coil_capacity_kw * heating_coil_valve[ahu])
                .min(delivered_by_ahu[ahu] * 1.2 * (40.0 - mixed_air_c[ahu]).max(0.0));
            (coil_cool_request[ahu], coil_cool_target_c[ahu]) = cooling_coil_request_kw(
                mixed_air_c[ahu],
                mixed_humidity_ratio[ahu],
                delivered_by_ahu[ahu],
                cooling_coil_valve[ahu],
            );
            heat_request += coil_heat_request[ahu];
            cool_request += coil_cool_request[ahu];
        }
        // Divide measured loop flow among its parallel branches in
        // proportion to their conductance. Bypass water returns without
        // exchanging heat with an AHU coil or VAV reheat coil.
        let branch_flow = |loop_index: usize, conductance: f64| {
            if valve_area[loop_index] > 0.0 {
                water_flow_kg_s[loop_index] * conductance / valve_area[loop_index]
            } else {
                0.0
            }
        };
        let hot_capacity_per_flow = 4.18 * (water_supply_c[0] - 35.0).max(0.0);
        let chilled_capacity_per_flow = 4.18 * (17.0 - water_supply_c[1]).max(0.0);
        let hot_bypass_flow = branch_flow(0, self.scenario.water_bypass_conductance[0]);
        let chilled_bypass_flow = branch_flow(1, self.scenario.water_bypass_conductance[1]);
        let coil_hot_flow = [0, 1].map(|ahu| {
            branch_flow(
                0,
                self.scenario.water_coil_conductance[0] * heating_coil_valve[ahu] / 2.0,
            )
        });
        let coil_chilled_flow = [0, 1].map(|ahu| {
            branch_flow(
                1,
                self.scenario.water_coil_conductance[1] * cooling_coil_valve[ahu] / 2.0,
            )
        });
        let reheat_flow = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map(|zone| {
            branch_flow(
                0,
                self.scenario.hot_reheat_conductance * effective_reheat_opening[zone] / 10.0,
            )
        });
        let coil_heat_deliverable = [0, 1]
            .map(|ahu| coil_heat_request[ahu].min(coil_hot_flow[ahu] * hot_capacity_per_flow));
        let coil_cool_deliverable = [0, 1].map(|ahu| {
            coil_cool_request[ahu].min(coil_chilled_flow[ahu] * chilled_capacity_per_flow)
        });
        let reheat_deliverable = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            .map(|zone| reheat_request[zone].min(reheat_flow[zone] * hot_capacity_per_flow));
        let deliverable_heat_kw =
            coil_heat_deliverable.iter().sum::<f64>() + reheat_deliverable.iter().sum::<f64>();
        let deliverable_cool_kw = coil_cool_deliverable.iter().sum::<f64>();
        // The branch limits apply before the mixed-volume energy limit.
        // Without the latter, a long step can empty a loop and create
        // energy when its water temperature is clamped.
        let water_capacity_kwh_per_k = self
            .scenario
            .water_volume_kg
            .map(|mass| mass * 4.18 / 3_600.0);
        let stored_heat_kw =
            (water_supply_c[0] - 35.0).max(0.0) * water_capacity_kwh_per_k[0] / dt_hours;
        let stored_cool_kw =
            (17.0 - water_supply_c[1]).max(0.0) * water_capacity_kwh_per_k[1] / dt_hours;
        let heat_availability = if deliverable_heat_kw > 0.0 {
            clamp(
                (stored_heat_kw + plant_heat) / deliverable_heat_kw,
                0.0,
                1.0,
            )
        } else {
            1.0
        };
        let cool_availability = if deliverable_cool_kw > 0.0 {
            clamp(
                (stored_cool_kw + plant_cool) / deliverable_cool_kw,
                0.0,
                1.0,
            )
        } else {
            1.0
        };
        // Water-limited cooling energy sets both leaving temperature and
        // condensate. Heating follows cooling without adding moisture.
        let mut supply_humidity_ratio = [0.0_f64; 2];
        let mut coil_condensate_kg_s = [0.0_f64; 2];
        let actual_supply_c = [0, 1].map(|ahu| {
            let airflow = delivered_by_ahu[ahu];
            let (cooled_c, cooled_ratio) = cooling_coil_leaving_state(
                mixed_air_c[ahu],
                mixed_humidity_ratio[ahu],
                airflow,
                coil_cool_deliverable[ahu] * cool_availability,
                coil_cool_target_c[ahu],
            );
            supply_humidity_ratio[ahu] = cooled_ratio;
            coil_condensate_kg_s[ahu] =
                1.2 * airflow * (mixed_humidity_ratio[ahu] - cooled_ratio).max(0.0);
            if airflow > 0.0 {
                cooled_c
                    + coil_heat_deliverable[ahu] * heat_availability
                        / (1.2 * airflow * (1.006 + 1.86 * cooled_ratio))
            } else {
                cooled_c
            }
        });
        // Each zone is a mixed air volume. The prior zone concentrations
        // determine AHU return concentration; actual outdoor-damper and
        // terminal flows determine the next measured concentrations.
        // This transport model never decides a ventilation command.
        let return_co2_ppm =
            [0, 1].map(|ahu| ahu_return_value(&self.co2_ppm, &delivered_flow, ahu));
        let supply_co2_ppm = [0, 1].map(|ahu| {
            outdoor_air_fraction[ahu] * self.scenario.outdoor_co2_ppm
                + (1.0 - outdoor_air_fraction[ahu]) * return_co2_ppm[ahu]
        });
        for zone in 0..10 {
            // Teaching conversion: one normalized airflow unit is 1 m³/s.
            let airflow = delivered_flow[zone];
            let infiltration = self.scenario.infiltration(zone)
                + if window_closed[zone] {
                    0.0
                } else {
                    self.scenario.open_window_infiltration(zone)
                };
            let total_flow = airflow + infiltration;
            let mixing_decay = if total_flow > 0.0 {
                (-total_flow * dt_seconds / self.scenario.air_volume(zone)).exp()
            } else {
                1.0
            };
            let source_ppm_m3_s = self.actual_occupant_count[zone]
                * self.scenario.co2_generation_lps_per_person
                * 1000.0;
            let outside_source = airflow * supply_co2_ppm[zone / 5]
                + infiltration * self.scenario.outdoor_co2_ppm
                + source_ppm_m3_s;
            self.co2_ppm[zone] = if total_flow > 0.0 {
                let equilibrium = outside_source / total_flow;
                equilibrium + (self.co2_ppm[zone] - equilibrium) * mixing_decay
            } else {
                self.co2_ppm[zone] + source_ppm_m3_s * dt_seconds / self.scenario.air_volume(zone)
            };
            let moisture_source_kg_s = self.actual_occupant_count[zone]
                * self.scenario.occupant_moisture_kg_h_per_person
                / 3600.0;
            let moisture_inflow = airflow * supply_humidity_ratio[zone / 5]
                + infiltration * outdoor_humidity_ratio
                + moisture_source_kg_s / 1.2;
            self.zone_humidity_ratio[zone] = if total_flow > 0.0 {
                let equilibrium = moisture_inflow / total_flow;
                equilibrium + (self.zone_humidity_ratio[zone] - equilibrium) * mixing_decay
            } else {
                self.zone_humidity_ratio[zone]
                    + moisture_source_kg_s * dt_seconds / (1.2 * self.scenario.air_volume(zone))
            };
        }
        let mut discharge_c = [0.0_f64; 10];
        for zone in 0..10 {
            let ahu = zone / 5;
            let reheat_kw = reheat_deliverable[zone] * heat_availability;
            discharge_c[zone] = if delivered_flow[zone] > 0.0 {
                actual_supply_c[ahu] + reheat_kw / (1.2 * delivered_flow[zone])
            } else {
                actual_supply_c[ahu]
            };
            let air_exchange_kw =
                delivered_flow[zone] * 1.2 * (discharge_c[zone] - self.temps[zone]);
            let solar_share = w.solar_kw * (0.7 + 0.08 * (zone % 5) as f64) / 10.0;
            // Closed-window leakage is already included in the effective envelope UA.
            // Only the additional open-window exchange adds sensible heat here.
            let window_exchange_kw = if window_closed[zone] {
                0.0
            } else {
                1.2 * self.scenario.open_window_infiltration(zone) * (outdoor_c - self.temps[zone])
            };
            let load = self.scenario.zone_ua_kw_per_k * (outdoor_c - self.temps[zone])
                + solar_share
                + w.internal_kw / 10.0
                + self.occupant_sensible_heat_kw[zone]
                + air_exchange_kw
                + window_exchange_kw;
            self.temps[zone] = clamp(
                self.temps[zone] + load * dt_hours / self.scenario.zone_capacity_kwh_per_k,
                -10.0,
                50.0,
            );
            let (settled_c, settled_ratio, condensate, latent_kw) = settle_zone_condensation(
                self.temps[zone],
                self.zone_humidity_ratio[zone],
                self.scenario.air_volume(zone),
                self.scenario.zone_capacity_kwh_per_k,
                dt_seconds,
            );
            self.temps[zone] = settled_c;
            self.zone_humidity_ratio[zone] = settled_ratio;
            self.interval_zone_condensate_kg[zone] += condensate * dt_seconds;
            self.interval_zone_condensation_heat_kwh[zone] += latent_kw * dt_hours;
            let (lo, hi) = if w.occupied {
                (20.0, 25.0)
            } else {
                (17.0, 29.0)
            };
            if self.temps[zone] < lo || self.temps[zone] > hi {
                self.unmet_hours += dt_hours;
                if w.occupied && self.actual_occupant_count[zone] > 0.0 {
                    self.occupied_unmet_hours += dt_hours;
                }
            }
        }
        self.last_heat_request = heat_request;
        self.last_cool_request = cool_request;
        self.last_flow_request = requested_total;
        self.last_supply_c = actual_supply_c;
        self.last_mixed_air_c = mixed_air_c;
        for ahu in 0..2 {
            let bias = self
                .scenario
                .ahu_mixed_air_sensor_events
                .iter()
                .rev()
                .find(|event| {
                    event.ahu == ahu + 1
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                })
                .map_or(0.0, |event| event.bias_c);
            self.last_measured_mixed_air_c[ahu] = mixed_air_c[ahu] + bias;
        }
        self.last_duct_pressure = duct_pressure;
        self.last_outdoor_airflow = outdoor_airflow;
        self.last_zone_airflow = delivered_flow;
        self.last_zone_discharge_c = discharge_c;
        self.last_zone_damper_position = actual_damper_position;
        self.last_zone_reheat_valve_position = actual_reheat_valve_position;
        for zone in 0..10 {
            let bias = self
                .scenario
                .terminal_valve_sensor_events
                .iter()
                .rev()
                .find(|event| {
                    event.zone == zone + 1
                        && elapsed_hours >= event.start_hour
                        && elapsed_hours < event.start_hour + event.duration_hours
                })
                .map_or(0.0, |event| event.bias);
            self.last_zone_measured_reheat_valve_position[zone] =
                clamp(actual_reheat_valve_position[zone] + bias, 0.0, 1.0);
        }
        let heat_kw = deliverable_heat_kw * heat_availability;
        let cool_kw = deliverable_cool_kw * cool_availability;
        let water_return_c = [
            if water_flow_kg_s[0] > 0.0 {
                water_supply_c[0] - heat_kw / (water_flow_kg_s[0] * 4.18)
            } else {
                water_supply_c[0]
            },
            if water_flow_kg_s[1] > 0.0 {
                water_supply_c[1] + cool_kw / (water_flow_kg_s[1] * 4.18)
            } else {
                water_supply_c[1]
            },
        ];
        // Each configurable mixed volume stores sensible energy.
        // CDL commands source production; this physical state only balances
        // heat produced and heat transferred to coils and terminal reheat.
        let source_heat_kw = plant_heat.min(
            heat_kw + (60.0 - water_supply_c[0]).max(0.0) * water_capacity_kwh_per_k[0] / dt_hours,
        );
        let source_cool_kw = plant_cool.min(
            cool_kw + (water_supply_c[1] - 4.0).max(0.0) * water_capacity_kwh_per_k[1] / dt_hours,
        );
        self.interval_water_source_kwh[0] += source_heat_kw * dt_hours;
        self.interval_water_source_kwh[1] += source_cool_kw * dt_hours;
        self.interval_water_delivered_kwh[0] += heat_kw * dt_hours;
        self.interval_water_delivered_kwh[1] += cool_kw * dt_hours;
        // Plant proof is physical source operation in heating service,
        // independent of whether a saturated water volume can absorb
        // additional heat. Cooling-only heat-pump operation cannot prove
        // the hot-water plant. CDL alone interprets this raw status.
        self.last_hot_water_plant_proven = (self.source_running[0]
            && (heat_pump_mode == 1.0 || heat_pump_mode == 3.0))
            || self.source_running[1];
        self.last_water_supply_c = [
            clamp(
                water_supply_c[0]
                    + (source_heat_kw - heat_kw) * dt_hours / water_capacity_kwh_per_k[0],
                35.0,
                60.0,
            ),
            clamp(
                water_supply_c[1]
                    + (cool_kw - source_cool_kw) * dt_hours / water_capacity_kwh_per_k[1],
                4.0,
                17.0,
            ),
        ];
        self.last_water_return_c = water_return_c;
        self.last_water_flow_kg_s = water_flow_kg_s;
        self.last_water_dp_pa = water_dp_pa;
        self.last_water_pump_running = actual_pump_speed.map(|speed| speed > 0.0);
        let hp_heat_used = if plant_heat > 0.0 {
            source_heat_kw * hp_heat / plant_heat
        } else {
            0.0
        };
        let boiler_used = source_heat_kw - hp_heat_used;
        let hp_cool_used = if plant_cool > 0.0 {
            source_cool_kw * hp_cool / plant_cool
        } else {
            0.0
        };
        let chiller_used = source_cool_kw - hp_cool_used;
        let fan_kw = actual_fan_speed
            .iter()
            .map(|s| 2.0 * clamp(*s, 0.0, 1.0).powi(3))
            .sum::<f64>();
        let water_pump_kw = 3.0 * (actual_pump_speed[0].powi(3) + actual_pump_speed[1].powi(3));
        // The graph selects the service mode and thermal dispatch. This
        // physical coefficient only adjusts compressor work when both
        // services run: zero is independent compressors, one preserves
        // the former full overlap of the smaller compressor load.
        let part_load = if hp_available_capacity_kw > 0.0 {
            ((hp_heat_used + hp_cool_used) / hp_available_capacity_kw).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let joint_part_load = self
            .scenario
            .heat_pump_joint_performance
            .as_ref()
            .map(|map| {
                heat_pump_joint_value(
                    map,
                    outdoor_c,
                    water_supply_c[0],
                    water_supply_c[1],
                    part_load,
                    if hp_heat_used > 0.0 && hp_cool_used > 0.0 {
                        Some(hp_heat_used / (hp_heat_used + hp_cool_used))
                    } else {
                        None
                    },
                )
            });
        let (
            cop_multiplier,
            hot_water_cop_multiplier,
            chilled_water_cop_multiplier,
            effective_heating_cop,
            effective_cooling_cop,
        ) = if let Some(value) = joint_part_load.as_ref() {
            (1.0, 1.0, 1.0, value.heating_cop, value.cooling_cop)
        } else {
            let part_multiplier =
                part_load_cop_multiplier(&self.scenario.heat_pump_part_load_performance, part_load);
            let hot_multiplier = water_cop_multiplier(
                &self.scenario.heat_pump_hot_water_cop_performance,
                water_supply_c[0],
            );
            let chilled_multiplier = water_cop_multiplier(
                &self.scenario.heat_pump_chilled_water_cop_performance,
                water_supply_c[1],
            );
            (
                part_multiplier,
                hot_multiplier,
                chilled_multiplier,
                heating_cop * part_multiplier * hot_multiplier,
                cooling_cop * part_multiplier * chilled_multiplier,
            )
        };
        let heat_pump_electric_kw =
            if heat_pump_mode == 3.0 && hp_heat_used > 0.0 && hp_cool_used > 0.0 {
                if let Some(cop) = joint_part_load
                    .as_ref()
                    .and_then(|value| value.simultaneous_cop)
                {
                    (hp_heat_used + hp_cool_used) / cop
                } else {
                    heat_pump_compressor_work_kw(
                        hp_heat_used,
                        hp_cool_used,
                        effective_heating_cop,
                        effective_cooling_cop,
                        true,
                        self.scenario.heat_recovery_compressor_savings_fraction,
                    )
                }
            } else {
                heat_pump_compressor_work_kw(
                    hp_heat_used,
                    hp_cool_used,
                    effective_heating_cop,
                    effective_cooling_cop,
                    false,
                    self.scenario.heat_recovery_compressor_savings_fraction,
                )
            };
        let heat_pump_ambient_exchange_kw = hp_heat_used - hp_cool_used - heat_pump_electric_kw;
        let electric_kw = heat_pump_electric_kw + chiller_used / 3.5 + fan_kw + water_pump_kw;
        let gas_kw = boiler_used / 0.9;
        let electric_kwh = electric_kw * dt_hours;
        let gas_kwh = gas_kw * dt_hours;
        self.electricity_kwh += electric_kwh;
        self.gas_kwh += gas_kwh;
        if (self.cursor + 1) % steps_per_weather == 0 {
            // Derive interval mean power from the single cumulative energy
            // totalizer, so chart and CSV energy cannot drift from summary.
            let recorded_electric_kw =
                (self.electricity_kwh - self.recorded_electricity_kwh) / 0.25;
            let recorded_gas_kw = (self.gas_kwh - self.recorded_gas_kwh) / 0.25;
            self.recorded_electricity_kwh = self.electricity_kwh;
            self.recorded_gas_kwh = self.gas_kwh;
            let mut row = SimulationRow {
                index,
                schedule_occupied: if w.occupied { 1.0 } else { 0.0 },
                preconditioning_mode: if preconditioning { 1.0 } else { 0.0 },
                zone_occupied: self
                    .zone_occupied
                    .map(|occupied| if occupied { 1.0 } else { 0.0 }),
                zone_presence_raw: self
                    .raw_occupancy
                    .map(|occupied| if occupied { 1.0 } else { 0.0 }),
                zone_occupant_count_actual: self.actual_occupant_count,
                zone_airflow: delivered_flow,
                zone_discharge_c: discharge_c,
                outdoor_c,
                avg_zone_c: self.temps.iter().sum::<f64>() / 10.0,
                zone_c: self.temps,
                cool_kw,
                heat_kw,
                cool_request_kw: cool_request,
                heat_request_kw: heat_request,
                electric_kw: recorded_electric_kw,
                gas_kw: recorded_gas_kw,
                requested_airflow: requested_total.iter().sum::<f64>(),
                delivered_airflow: delivered_by_ahu.iter().sum::<f64>(),
                ahu_1_supply_actual_c: actual_supply_c[0],
                ahu_2_supply_actual_c: actual_supply_c[1],
                ahu_1_supply_setpoint_c: supply_setpoint[0],
                ahu_2_supply_setpoint_c: supply_setpoint[1],
                ahu_1_duct_pressure: duct_pressure[0],
                ahu_2_duct_pressure: duct_pressure[1],
                ahu_1_outdoor_air_fraction: outdoor_air_fraction[0],
                ahu_2_outdoor_air_fraction: outdoor_air_fraction[1],
                ahu_1_intake_opening: intake_opening[0],
                ahu_2_intake_opening: intake_opening[1],
                ahu_1_heating_coil_valve_position: heating_coil_command[0],
                ahu_2_heating_coil_valve_position: heating_coil_command[1],
                ahu_1_cooling_coil_valve_position: cooling_coil_command[0],
                ahu_2_cooling_coil_valve_position: cooling_coil_command[1],
                ahu_1_outdoor_airflow: outdoor_airflow[0],
                ahu_2_outdoor_airflow: outdoor_airflow[1],
                ahu_1_mixed_air_c: mixed_air_c[0],
                ahu_2_mixed_air_c: mixed_air_c[1],
                heat_pump_heating_used_kw: hp_heat_used,
                heat_pump_cooling_used_kw: hp_cool_used,
                boiler_used_kw: boiler_used,
                chiller_used_kw: chiller_used,
                ..Default::default()
            };
            for ahu in 0..2 {
                row.set_measure(
                    &format!("ahu_{}_heating_coil_valve_actual_position", ahu + 1),
                    heating_coil_valve[ahu],
                );
                row.set_measure(
                    &format!("ahu_{}_cooling_coil_valve_actual_position", ahu + 1),
                    cooling_coil_valve[ahu],
                );
            }
            row.water_interval_start_c = self.interval_water_start_c;
            row.water_interval_end_c = self.last_water_supply_c;
            row.water_interval_source_kwh = self.interval_water_source_kwh;
            row.water_interval_delivered_kwh = self.interval_water_delivered_kwh;
            row.water_capacity_kwh_per_k = water_capacity_kwh_per_k;
            self.interval_water_source_kwh = [0.0; 2];
            self.interval_water_delivered_kwh = [0.0; 2];
            row.ahu_1_return_air_c = physical_return_air_c[0];
            row.ahu_2_return_air_c = physical_return_air_c[1];
            let zone_rh = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map(|zone| {
                relative_humidity_from_ratio(self.temps[zone], self.zone_humidity_ratio[zone])
            });
            row.zone_relative_humidity = zone_rh;
            row.zone_condensate_kg_s = self.interval_zone_condensate_kg.map(|kg| kg / 900.0);
            row.zone_condensation_heat_kw = self
                .interval_zone_condensation_heat_kwh
                .map(|kwh| kwh / 0.25);
            self.interval_zone_condensate_kg = [0.0; 10];
            self.interval_zone_condensation_heat_kwh = [0.0; 10];
            for ahu in 0..2 {
                row.set_measure(
                    &format!("ahu_{}_return_air_relative_humidity", ahu + 1),
                    return_rh_input[ahu],
                );
            }
            row.ahu_1_measured_mixed_air_c = self.last_measured_mixed_air_c[0];
            row.ahu_2_measured_mixed_air_c = self.last_measured_mixed_air_c[1];
            row.zone_co2_ppm = self.co2_ppm;
            row.zone_occupant_count_sensor = self.raw_occupant_count;
            row.zone_occupant_sensible_heat_kw = self.occupant_sensible_heat_kw;
            row.ahu_1_actual_fan_speed = actual_fan_speed[0];
            row.ahu_2_actual_fan_speed = actual_fan_speed[1];
            row.ahu_1_measured_fan_speed = self.last_measured_fan_speed[0];
            row.ahu_2_measured_fan_speed = self.last_measured_fan_speed[1];
            for ahu in 0..2 {
                row.set_measure(
                    &format!("ahu_{}_actual_outdoor_damper_position", ahu + 1),
                    actual_outdoor_damper_position[ahu],
                );
                row.set_measure(
                    &format!("ahu_{}_actual_return_damper_position", ahu + 1),
                    actual_return_damper_position[ahu],
                );
                row.set_measure(
                    &format!("ahu_{}_measured_outdoor_damper_position", ahu + 1),
                    self.last_measured_outdoor_damper_position[ahu],
                );
                row.set_measure(
                    &format!("ahu_{}_measured_return_damper_position", ahu + 1),
                    self.last_measured_return_damper_position[ahu],
                );
                row.set_measure(
                    &format!("ahu_{}_heating_coil_valve_measured_position", ahu + 1),
                    self.last_measured_heating_coil_valve_position[ahu],
                );
                row.set_measure(
                    &format!("ahu_{}_cooling_coil_valve_measured_position", ahu + 1),
                    self.last_measured_cooling_coil_valve_position[ahu],
                );
            }
            row.zone_actual_damper_position = actual_damper_position;
            row.zone_actual_reheat_valve_position = actual_reheat_valve_position;
            row.zone_measured_reheat_valve_position = self.last_zone_measured_reheat_valve_position;
            row.heat_pump_electric_kw = heat_pump_electric_kw;
            row.heat_pump_ambient_exchange_kw = heat_pump_ambient_exchange_kw;
            row.heat_pump_available_capacity_kw = hp_available_capacity_kw;
            row.heat_pump_heating_available_capacity_kw = hp_heating_available_capacity_kw;
            row.heat_pump_cooling_available_capacity_kw = hp_cooling_available_capacity_kw;
            row.heat_pump_part_load_fraction = part_load;
            row.heat_pump_cop_multiplier = cop_multiplier;
            row.heat_pump_hot_water_cop_multiplier = hot_water_cop_multiplier;
            row.heat_pump_chilled_water_cop_multiplier = chilled_water_cop_multiplier;
            row.heat_pump_heating_cop = effective_heating_cop;
            row.heat_pump_cooling_cop = effective_cooling_cop;
            for (name, available) in ["heat_pump", "boiler", "chiller"]
                .into_iter()
                .zip(equipment_status)
            {
                row.set_measure(
                    &format!("{name}_available"),
                    if available { 1.0 } else { 0.0 },
                );
            }
            for (name, running) in ["heat_pump", "boiler", "chiller"]
                .into_iter()
                .zip(self.source_running)
            {
                row.set_measure(&format!("{name}_running"), if running { 1.0 } else { 0.0 });
            }
            row.hot_water_plant_proven = if self.last_hot_water_plant_proven {
                1.0
            } else {
                0.0
            };
            for (equipment, fault) in ["heat_pump", "boiler", "chiller"]
                .into_iter()
                .zip(source_start_faults.iter())
            {
                row.set_measure(
                    &format!("plant_{equipment}_start_fault"),
                    if *fault { 1.0 } else { 0.0 },
                );
            }
            for ahu in 0..2 {
                let prefix = format!("ahu_{}", ahu + 1);
                let measures = [
                    ("heating_coil_request_kw", coil_heat_request[ahu]),
                    ("cooling_coil_request_kw", coil_cool_request[ahu]),
                    (
                        "heating_coil_delivered_kw",
                        coil_heat_deliverable[ahu] * heat_availability,
                    ),
                    (
                        "cooling_coil_delivered_kw",
                        coil_cool_deliverable[ahu] * cool_availability,
                    ),
                    ("cooling_coil_condensate_kg_s", coil_condensate_kg_s[ahu]),
                    (
                        "supply_air_relative_humidity",
                        relative_humidity_from_ratio(
                            actual_supply_c[ahu],
                            supply_humidity_ratio[ahu],
                        ),
                    ),
                    (
                        "mixed_air_enthalpy_kj_kg",
                        moist_air_enthalpy_kj_kg(mixed_air_c[ahu], mixed_humidity_ratio[ahu]),
                    ),
                    (
                        "supply_air_enthalpy_kj_kg",
                        moist_air_enthalpy_kj_kg(actual_supply_c[ahu], supply_humidity_ratio[ahu]),
                    ),
                ];
                for (name, value) in measures {
                    row.set_measure(&format!("{prefix}_{name}"), value);
                }
            }
            let water_measures = [
                ("hot_water_supply_temp_c", water_supply_c[0]),
                ("hot_water_supply_setpoint_c", water_setpoint_c[0]),
                ("hot_water_return_temp_c", water_return_c[0]),
                ("chilled_water_supply_temp_c", water_supply_c[1]),
                ("chilled_water_supply_setpoint_c", water_setpoint_c[1]),
                ("chilled_water_return_temp_c", water_return_c[1]),
                (
                    "hot_water_flow_kg_s",
                    measured_water_flow(
                        water_flow_kg_s[0],
                        "hot",
                        elapsed_hours,
                        &self.scenario.water_flow_sensor_events,
                    ),
                ),
                (
                    "chilled_water_flow_kg_s",
                    measured_water_flow(
                        water_flow_kg_s[1],
                        "chilled",
                        elapsed_hours,
                        &self.scenario.water_flow_sensor_events,
                    ),
                ),
                ("hot_water_actual_flow_kg_s", water_flow_kg_s[0]),
                ("chilled_water_actual_flow_kg_s", water_flow_kg_s[1]),
                ("hot_water_bypass_flow_kg_s", hot_bypass_flow),
                ("chilled_water_bypass_flow_kg_s", chilled_bypass_flow),
                ("ahu_1_hot_water_flow_kg_s", coil_hot_flow[0]),
                ("ahu_2_hot_water_flow_kg_s", coil_hot_flow[1]),
                ("ahu_1_chilled_water_flow_kg_s", coil_chilled_flow[0]),
                ("ahu_2_chilled_water_flow_kg_s", coil_chilled_flow[1]),
                ("hot_water_reheat_flow_kg_s", reheat_flow.iter().sum()),
                ("hot_water_differential_pressure_pa", water_dp_pa[0]),
                ("chilled_water_differential_pressure_pa", water_dp_pa[1]),
                ("hot_water_pump_speed", hot_water_pump_speed),
                ("chilled_water_pump_speed", chilled_water_pump_speed),
                (
                    "hot_water_pump_running",
                    if actual_pump_speed[0] > 0.0 { 1.0 } else { 0.0 },
                ),
                (
                    "chilled_water_pump_running",
                    if actual_pump_speed[1] > 0.0 { 1.0 } else { 0.0 },
                ),
                (
                    "hot_water_enable",
                    if hot_water_enabled { 1.0 } else { 0.0 },
                ),
                (
                    "chilled_water_enable",
                    if chilled_water_enabled { 1.0 } else { 0.0 },
                ),
                ("water_pump_kw", water_pump_kw),
            ];
            for (name, value) in water_measures {
                row.set_measure(name, value);
            }
            if !traces.is_empty() {
                row.traces = traces;
            }
            result = Some(row);
        }

        self.cursor += 1;
        Ok(result)
    }
    /// Completed integration intervals.
    pub fn progress(&self) -> usize {
        self.cursor
    }
    /// Configured integration interval in seconds.
    pub fn effective_step_seconds(&self) -> usize {
        self.effective_step_seconds
    }
    /// Total integration intervals in the supplied weather year.
    pub fn total(&self) -> usize {
        self.weather.len() * (900 / self.effective_step_seconds)
    }
    /// Whether all weather intervals have completed.
    pub fn done(&self) -> bool {
        self.cursor == self.total()
    }
    /// Accumulated recorded energy and comfort metrics.
    pub fn summary(&self) -> SimulationTotals {
        simulation_totals(self)
    }
}
