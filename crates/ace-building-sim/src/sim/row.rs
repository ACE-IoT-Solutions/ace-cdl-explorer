//! Quarter-hour output records shared by native and browser callers.
use serde::Serialize;
use std::collections::BTreeMap;
/// A complete recorded interval with fixed topology arrays and scalar measurements.
#[derive(Clone, Debug, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct SimulationRow {
    pub ahu_1_actual_fan_speed: f64,
    pub ahu_1_actual_outdoor_damper_position: f64,
    pub ahu_1_actual_return_damper_position: f64,
    pub ahu_1_chilled_water_flow_kg_s: f64,
    pub ahu_1_cooling_coil_condensate_kg_s: f64,
    pub ahu_1_cooling_coil_delivered_kw: f64,
    pub ahu_1_cooling_coil_request_kw: f64,
    pub ahu_1_cooling_coil_valve_actual_position: f64,
    pub ahu_1_cooling_coil_valve_measured_position: f64,
    pub ahu_1_cooling_coil_valve_position: f64,
    pub ahu_1_duct_pressure: f64,
    pub ahu_1_heating_coil_delivered_kw: f64,
    pub ahu_1_heating_coil_request_kw: f64,
    pub ahu_1_heating_coil_valve_actual_position: f64,
    pub ahu_1_heating_coil_valve_measured_position: f64,
    pub ahu_1_heating_coil_valve_position: f64,
    pub ahu_1_hot_water_flow_kg_s: f64,
    pub ahu_1_intake_opening: f64,
    pub ahu_1_measured_fan_speed: f64,
    pub ahu_1_measured_mixed_air_c: f64,
    pub ahu_1_measured_outdoor_damper_position: f64,
    pub ahu_1_measured_return_damper_position: f64,
    pub ahu_1_mixed_air_c: f64,
    pub ahu_1_mixed_air_enthalpy_kj_kg: f64,
    pub ahu_1_outdoor_air_fraction: f64,
    pub ahu_1_outdoor_airflow: f64,
    pub ahu_1_return_air_c: f64,
    pub ahu_1_return_air_relative_humidity: f64,
    pub ahu_1_supply_actual_c: f64,
    pub ahu_1_supply_air_enthalpy_kj_kg: f64,
    pub ahu_1_supply_air_relative_humidity: f64,
    pub ahu_1_supply_setpoint_c: f64,
    pub ahu_2_actual_fan_speed: f64,
    pub ahu_2_actual_outdoor_damper_position: f64,
    pub ahu_2_actual_return_damper_position: f64,
    pub ahu_2_chilled_water_flow_kg_s: f64,
    pub ahu_2_cooling_coil_condensate_kg_s: f64,
    pub ahu_2_cooling_coil_delivered_kw: f64,
    pub ahu_2_cooling_coil_request_kw: f64,
    pub ahu_2_cooling_coil_valve_actual_position: f64,
    pub ahu_2_cooling_coil_valve_measured_position: f64,
    pub ahu_2_cooling_coil_valve_position: f64,
    pub ahu_2_duct_pressure: f64,
    pub ahu_2_heating_coil_delivered_kw: f64,
    pub ahu_2_heating_coil_request_kw: f64,
    pub ahu_2_heating_coil_valve_actual_position: f64,
    pub ahu_2_heating_coil_valve_measured_position: f64,
    pub ahu_2_heating_coil_valve_position: f64,
    pub ahu_2_hot_water_flow_kg_s: f64,
    pub ahu_2_intake_opening: f64,
    pub ahu_2_measured_fan_speed: f64,
    pub ahu_2_measured_mixed_air_c: f64,
    pub ahu_2_measured_outdoor_damper_position: f64,
    pub ahu_2_measured_return_damper_position: f64,
    pub ahu_2_mixed_air_c: f64,
    pub ahu_2_mixed_air_enthalpy_kj_kg: f64,
    pub ahu_2_outdoor_air_fraction: f64,
    pub ahu_2_outdoor_airflow: f64,
    pub ahu_2_return_air_c: f64,
    pub ahu_2_return_air_relative_humidity: f64,
    pub ahu_2_supply_actual_c: f64,
    pub ahu_2_supply_air_enthalpy_kj_kg: f64,
    pub ahu_2_supply_air_relative_humidity: f64,
    pub ahu_2_supply_setpoint_c: f64,
    pub avg_zone_c: f64,
    pub boiler_available: f64,
    pub boiler_running: f64,
    pub boiler_used_kw: f64,
    pub chilled_water_actual_flow_kg_s: f64,
    pub chilled_water_bypass_flow_kg_s: f64,
    pub chilled_water_differential_pressure_pa: f64,
    pub chilled_water_enable: f64,
    pub chilled_water_flow_kg_s: f64,
    pub chilled_water_pump_running: f64,
    pub chilled_water_pump_speed: f64,
    pub chilled_water_return_temp_c: f64,
    pub chilled_water_supply_setpoint_c: f64,
    pub chilled_water_supply_temp_c: f64,
    pub chiller_available: f64,
    pub chiller_running: f64,
    pub chiller_used_kw: f64,
    pub cool_kw: f64,
    pub cool_request_kw: f64,
    pub delivered_airflow: f64,
    pub electric_kw: f64,
    pub gas_kw: f64,
    pub heat_kw: f64,
    pub heat_pump_ambient_exchange_kw: f64,
    pub heat_pump_available: f64,
    pub heat_pump_available_capacity_kw: f64,
    pub heat_pump_chilled_water_cop_multiplier: f64,
    pub heat_pump_cooling_available_capacity_kw: f64,
    pub heat_pump_cooling_cop: f64,
    pub heat_pump_cooling_used_kw: f64,
    pub heat_pump_cop_multiplier: f64,
    pub heat_pump_electric_kw: f64,
    pub heat_pump_heating_available_capacity_kw: f64,
    pub heat_pump_heating_cop: f64,
    pub heat_pump_heating_used_kw: f64,
    pub heat_pump_hot_water_cop_multiplier: f64,
    pub heat_pump_part_load_fraction: f64,
    pub heat_pump_running: f64,
    pub heat_request_kw: f64,
    pub hot_water_actual_flow_kg_s: f64,
    pub hot_water_bypass_flow_kg_s: f64,
    pub hot_water_differential_pressure_pa: f64,
    pub hot_water_enable: f64,
    pub hot_water_flow_kg_s: f64,
    pub hot_water_plant_proven: f64,
    pub hot_water_pump_running: f64,
    pub hot_water_pump_speed: f64,
    pub hot_water_reheat_flow_kg_s: f64,
    pub hot_water_return_temp_c: f64,
    pub hot_water_supply_setpoint_c: f64,
    pub hot_water_supply_temp_c: f64,
    pub index: usize,
    pub outdoor_c: f64,
    pub plant_boiler_start_fault: f64,
    pub plant_chiller_start_fault: f64,
    pub plant_heat_pump_start_fault: f64,
    pub preconditioning_mode: f64,
    pub requested_airflow: f64,
    pub schedule_occupied: f64,
    pub water_capacity_kwh_per_k: [f64; 2],
    pub water_interval_delivered_kwh: [f64; 2],
    pub water_interval_end_c: [f64; 2],
    pub water_interval_source_kwh: [f64; 2],
    pub water_interval_start_c: [f64; 2],
    pub water_pump_kw: f64,
    pub zone_actual_damper_position: [f64; 10],
    pub zone_actual_reheat_valve_position: [f64; 10],
    pub zone_airflow: [f64; 10],
    pub zone_c: [f64; 10],
    pub zone_co2_ppm: [f64; 10],
    pub zone_condensate_kg_s: [f64; 10],
    pub zone_condensation_heat_kw: [f64; 10],
    pub zone_discharge_c: [f64; 10],
    pub zone_measured_reheat_valve_position: [f64; 10],
    pub zone_occupant_count_actual: [f64; 10],
    pub zone_occupant_count_sensor: [f64; 10],
    pub zone_occupant_sensible_heat_kw: [f64; 10],
    pub zone_occupied: [f64; 10],
    pub zone_presence_raw: [f64; 10],
    pub zone_relative_humidity: [f64; 10],
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<_>", optional))]
    pub traces: BTreeMap<String, f64>,
}
impl SimulationRow {
    pub(crate) fn set_measure(&mut self, name: &str, value: f64) {
        match name {
            "ahu_1_actual_fan_speed" => self.ahu_1_actual_fan_speed = value,
            "ahu_1_actual_outdoor_damper_position" => {
                self.ahu_1_actual_outdoor_damper_position = value
            }
            "ahu_1_actual_return_damper_position" => {
                self.ahu_1_actual_return_damper_position = value
            }
            "ahu_1_chilled_water_flow_kg_s" => self.ahu_1_chilled_water_flow_kg_s = value,
            "ahu_1_cooling_coil_condensate_kg_s" => self.ahu_1_cooling_coil_condensate_kg_s = value,
            "ahu_1_cooling_coil_delivered_kw" => self.ahu_1_cooling_coil_delivered_kw = value,
            "ahu_1_cooling_coil_request_kw" => self.ahu_1_cooling_coil_request_kw = value,
            "ahu_1_cooling_coil_valve_actual_position" => {
                self.ahu_1_cooling_coil_valve_actual_position = value
            }
            "ahu_1_cooling_coil_valve_measured_position" => {
                self.ahu_1_cooling_coil_valve_measured_position = value
            }
            "ahu_1_cooling_coil_valve_position" => self.ahu_1_cooling_coil_valve_position = value,
            "ahu_1_duct_pressure" => self.ahu_1_duct_pressure = value,
            "ahu_1_heating_coil_delivered_kw" => self.ahu_1_heating_coil_delivered_kw = value,
            "ahu_1_heating_coil_request_kw" => self.ahu_1_heating_coil_request_kw = value,
            "ahu_1_heating_coil_valve_actual_position" => {
                self.ahu_1_heating_coil_valve_actual_position = value
            }
            "ahu_1_heating_coil_valve_measured_position" => {
                self.ahu_1_heating_coil_valve_measured_position = value
            }
            "ahu_1_heating_coil_valve_position" => self.ahu_1_heating_coil_valve_position = value,
            "ahu_1_hot_water_flow_kg_s" => self.ahu_1_hot_water_flow_kg_s = value,
            "ahu_1_intake_opening" => self.ahu_1_intake_opening = value,
            "ahu_1_measured_fan_speed" => self.ahu_1_measured_fan_speed = value,
            "ahu_1_measured_mixed_air_c" => self.ahu_1_measured_mixed_air_c = value,
            "ahu_1_measured_outdoor_damper_position" => {
                self.ahu_1_measured_outdoor_damper_position = value
            }
            "ahu_1_measured_return_damper_position" => {
                self.ahu_1_measured_return_damper_position = value
            }
            "ahu_1_mixed_air_c" => self.ahu_1_mixed_air_c = value,
            "ahu_1_mixed_air_enthalpy_kj_kg" => self.ahu_1_mixed_air_enthalpy_kj_kg = value,
            "ahu_1_outdoor_air_fraction" => self.ahu_1_outdoor_air_fraction = value,
            "ahu_1_outdoor_airflow" => self.ahu_1_outdoor_airflow = value,
            "ahu_1_return_air_c" => self.ahu_1_return_air_c = value,
            "ahu_1_return_air_relative_humidity" => self.ahu_1_return_air_relative_humidity = value,
            "ahu_1_supply_actual_c" => self.ahu_1_supply_actual_c = value,
            "ahu_1_supply_air_enthalpy_kj_kg" => self.ahu_1_supply_air_enthalpy_kj_kg = value,
            "ahu_1_supply_air_relative_humidity" => self.ahu_1_supply_air_relative_humidity = value,
            "ahu_1_supply_setpoint_c" => self.ahu_1_supply_setpoint_c = value,
            "ahu_2_actual_fan_speed" => self.ahu_2_actual_fan_speed = value,
            "ahu_2_actual_outdoor_damper_position" => {
                self.ahu_2_actual_outdoor_damper_position = value
            }
            "ahu_2_actual_return_damper_position" => {
                self.ahu_2_actual_return_damper_position = value
            }
            "ahu_2_chilled_water_flow_kg_s" => self.ahu_2_chilled_water_flow_kg_s = value,
            "ahu_2_cooling_coil_condensate_kg_s" => self.ahu_2_cooling_coil_condensate_kg_s = value,
            "ahu_2_cooling_coil_delivered_kw" => self.ahu_2_cooling_coil_delivered_kw = value,
            "ahu_2_cooling_coil_request_kw" => self.ahu_2_cooling_coil_request_kw = value,
            "ahu_2_cooling_coil_valve_actual_position" => {
                self.ahu_2_cooling_coil_valve_actual_position = value
            }
            "ahu_2_cooling_coil_valve_measured_position" => {
                self.ahu_2_cooling_coil_valve_measured_position = value
            }
            "ahu_2_cooling_coil_valve_position" => self.ahu_2_cooling_coil_valve_position = value,
            "ahu_2_duct_pressure" => self.ahu_2_duct_pressure = value,
            "ahu_2_heating_coil_delivered_kw" => self.ahu_2_heating_coil_delivered_kw = value,
            "ahu_2_heating_coil_request_kw" => self.ahu_2_heating_coil_request_kw = value,
            "ahu_2_heating_coil_valve_actual_position" => {
                self.ahu_2_heating_coil_valve_actual_position = value
            }
            "ahu_2_heating_coil_valve_measured_position" => {
                self.ahu_2_heating_coil_valve_measured_position = value
            }
            "ahu_2_heating_coil_valve_position" => self.ahu_2_heating_coil_valve_position = value,
            "ahu_2_hot_water_flow_kg_s" => self.ahu_2_hot_water_flow_kg_s = value,
            "ahu_2_intake_opening" => self.ahu_2_intake_opening = value,
            "ahu_2_measured_fan_speed" => self.ahu_2_measured_fan_speed = value,
            "ahu_2_measured_mixed_air_c" => self.ahu_2_measured_mixed_air_c = value,
            "ahu_2_measured_outdoor_damper_position" => {
                self.ahu_2_measured_outdoor_damper_position = value
            }
            "ahu_2_measured_return_damper_position" => {
                self.ahu_2_measured_return_damper_position = value
            }
            "ahu_2_mixed_air_c" => self.ahu_2_mixed_air_c = value,
            "ahu_2_mixed_air_enthalpy_kj_kg" => self.ahu_2_mixed_air_enthalpy_kj_kg = value,
            "ahu_2_outdoor_air_fraction" => self.ahu_2_outdoor_air_fraction = value,
            "ahu_2_outdoor_airflow" => self.ahu_2_outdoor_airflow = value,
            "ahu_2_return_air_c" => self.ahu_2_return_air_c = value,
            "ahu_2_return_air_relative_humidity" => self.ahu_2_return_air_relative_humidity = value,
            "ahu_2_supply_actual_c" => self.ahu_2_supply_actual_c = value,
            "ahu_2_supply_air_enthalpy_kj_kg" => self.ahu_2_supply_air_enthalpy_kj_kg = value,
            "ahu_2_supply_air_relative_humidity" => self.ahu_2_supply_air_relative_humidity = value,
            "ahu_2_supply_setpoint_c" => self.ahu_2_supply_setpoint_c = value,
            "avg_zone_c" => self.avg_zone_c = value,
            "boiler_available" => self.boiler_available = value,
            "boiler_running" => self.boiler_running = value,
            "boiler_used_kw" => self.boiler_used_kw = value,
            "chilled_water_actual_flow_kg_s" => self.chilled_water_actual_flow_kg_s = value,
            "chilled_water_bypass_flow_kg_s" => self.chilled_water_bypass_flow_kg_s = value,
            "chilled_water_differential_pressure_pa" => {
                self.chilled_water_differential_pressure_pa = value
            }
            "chilled_water_enable" => self.chilled_water_enable = value,
            "chilled_water_flow_kg_s" => self.chilled_water_flow_kg_s = value,
            "chilled_water_pump_running" => self.chilled_water_pump_running = value,
            "chilled_water_pump_speed" => self.chilled_water_pump_speed = value,
            "chilled_water_return_temp_c" => self.chilled_water_return_temp_c = value,
            "chilled_water_supply_setpoint_c" => self.chilled_water_supply_setpoint_c = value,
            "chilled_water_supply_temp_c" => self.chilled_water_supply_temp_c = value,
            "chiller_available" => self.chiller_available = value,
            "chiller_running" => self.chiller_running = value,
            "chiller_used_kw" => self.chiller_used_kw = value,
            "cool_kw" => self.cool_kw = value,
            "cool_request_kw" => self.cool_request_kw = value,
            "delivered_airflow" => self.delivered_airflow = value,
            "electric_kw" => self.electric_kw = value,
            "gas_kw" => self.gas_kw = value,
            "heat_kw" => self.heat_kw = value,
            "heat_pump_ambient_exchange_kw" => self.heat_pump_ambient_exchange_kw = value,
            "heat_pump_available" => self.heat_pump_available = value,
            "heat_pump_available_capacity_kw" => self.heat_pump_available_capacity_kw = value,
            "heat_pump_chilled_water_cop_multiplier" => {
                self.heat_pump_chilled_water_cop_multiplier = value
            }
            "heat_pump_cooling_available_capacity_kw" => {
                self.heat_pump_cooling_available_capacity_kw = value
            }
            "heat_pump_cooling_cop" => self.heat_pump_cooling_cop = value,
            "heat_pump_cooling_used_kw" => self.heat_pump_cooling_used_kw = value,
            "heat_pump_cop_multiplier" => self.heat_pump_cop_multiplier = value,
            "heat_pump_electric_kw" => self.heat_pump_electric_kw = value,
            "heat_pump_heating_available_capacity_kw" => {
                self.heat_pump_heating_available_capacity_kw = value
            }
            "heat_pump_heating_cop" => self.heat_pump_heating_cop = value,
            "heat_pump_heating_used_kw" => self.heat_pump_heating_used_kw = value,
            "heat_pump_hot_water_cop_multiplier" => self.heat_pump_hot_water_cop_multiplier = value,
            "heat_pump_part_load_fraction" => self.heat_pump_part_load_fraction = value,
            "heat_pump_running" => self.heat_pump_running = value,
            "heat_request_kw" => self.heat_request_kw = value,
            "hot_water_actual_flow_kg_s" => self.hot_water_actual_flow_kg_s = value,
            "hot_water_bypass_flow_kg_s" => self.hot_water_bypass_flow_kg_s = value,
            "hot_water_differential_pressure_pa" => self.hot_water_differential_pressure_pa = value,
            "hot_water_enable" => self.hot_water_enable = value,
            "hot_water_flow_kg_s" => self.hot_water_flow_kg_s = value,
            "hot_water_plant_proven" => self.hot_water_plant_proven = value,
            "hot_water_pump_running" => self.hot_water_pump_running = value,
            "hot_water_pump_speed" => self.hot_water_pump_speed = value,
            "hot_water_reheat_flow_kg_s" => self.hot_water_reheat_flow_kg_s = value,
            "hot_water_return_temp_c" => self.hot_water_return_temp_c = value,
            "hot_water_supply_setpoint_c" => self.hot_water_supply_setpoint_c = value,
            "hot_water_supply_temp_c" => self.hot_water_supply_temp_c = value,
            "outdoor_c" => self.outdoor_c = value,
            "plant_boiler_start_fault" => self.plant_boiler_start_fault = value,
            "plant_chiller_start_fault" => self.plant_chiller_start_fault = value,
            "plant_heat_pump_start_fault" => self.plant_heat_pump_start_fault = value,
            "preconditioning_mode" => self.preconditioning_mode = value,
            "requested_airflow" => self.requested_airflow = value,
            "schedule_occupied" => self.schedule_occupied = value,
            "water_pump_kw" => self.water_pump_kw = value,
            _ => unreachable!("unknown simulation measurement {name}"),
        }
    }
    /// Read a scalar measurement by its published column name.
    pub fn measure(&self, name: &str) -> Option<f64> {
        match name {
            "ahu_1_actual_fan_speed" => Some(self.ahu_1_actual_fan_speed),
            "ahu_1_actual_outdoor_damper_position" => {
                Some(self.ahu_1_actual_outdoor_damper_position)
            }
            "ahu_1_actual_return_damper_position" => Some(self.ahu_1_actual_return_damper_position),
            "ahu_1_chilled_water_flow_kg_s" => Some(self.ahu_1_chilled_water_flow_kg_s),
            "ahu_1_cooling_coil_condensate_kg_s" => Some(self.ahu_1_cooling_coil_condensate_kg_s),
            "ahu_1_cooling_coil_delivered_kw" => Some(self.ahu_1_cooling_coil_delivered_kw),
            "ahu_1_cooling_coil_request_kw" => Some(self.ahu_1_cooling_coil_request_kw),
            "ahu_1_cooling_coil_valve_actual_position" => {
                Some(self.ahu_1_cooling_coil_valve_actual_position)
            }
            "ahu_1_cooling_coil_valve_measured_position" => {
                Some(self.ahu_1_cooling_coil_valve_measured_position)
            }
            "ahu_1_cooling_coil_valve_position" => Some(self.ahu_1_cooling_coil_valve_position),
            "ahu_1_duct_pressure" => Some(self.ahu_1_duct_pressure),
            "ahu_1_heating_coil_delivered_kw" => Some(self.ahu_1_heating_coil_delivered_kw),
            "ahu_1_heating_coil_request_kw" => Some(self.ahu_1_heating_coil_request_kw),
            "ahu_1_heating_coil_valve_actual_position" => {
                Some(self.ahu_1_heating_coil_valve_actual_position)
            }
            "ahu_1_heating_coil_valve_measured_position" => {
                Some(self.ahu_1_heating_coil_valve_measured_position)
            }
            "ahu_1_heating_coil_valve_position" => Some(self.ahu_1_heating_coil_valve_position),
            "ahu_1_hot_water_flow_kg_s" => Some(self.ahu_1_hot_water_flow_kg_s),
            "ahu_1_intake_opening" => Some(self.ahu_1_intake_opening),
            "ahu_1_measured_fan_speed" => Some(self.ahu_1_measured_fan_speed),
            "ahu_1_measured_mixed_air_c" => Some(self.ahu_1_measured_mixed_air_c),
            "ahu_1_measured_outdoor_damper_position" => {
                Some(self.ahu_1_measured_outdoor_damper_position)
            }
            "ahu_1_measured_return_damper_position" => {
                Some(self.ahu_1_measured_return_damper_position)
            }
            "ahu_1_mixed_air_c" => Some(self.ahu_1_mixed_air_c),
            "ahu_1_mixed_air_enthalpy_kj_kg" => Some(self.ahu_1_mixed_air_enthalpy_kj_kg),
            "ahu_1_outdoor_air_fraction" => Some(self.ahu_1_outdoor_air_fraction),
            "ahu_1_outdoor_airflow" => Some(self.ahu_1_outdoor_airflow),
            "ahu_1_return_air_c" => Some(self.ahu_1_return_air_c),
            "ahu_1_return_air_relative_humidity" => Some(self.ahu_1_return_air_relative_humidity),
            "ahu_1_supply_actual_c" => Some(self.ahu_1_supply_actual_c),
            "ahu_1_supply_air_enthalpy_kj_kg" => Some(self.ahu_1_supply_air_enthalpy_kj_kg),
            "ahu_1_supply_air_relative_humidity" => Some(self.ahu_1_supply_air_relative_humidity),
            "ahu_1_supply_setpoint_c" => Some(self.ahu_1_supply_setpoint_c),
            "ahu_2_actual_fan_speed" => Some(self.ahu_2_actual_fan_speed),
            "ahu_2_actual_outdoor_damper_position" => {
                Some(self.ahu_2_actual_outdoor_damper_position)
            }
            "ahu_2_actual_return_damper_position" => Some(self.ahu_2_actual_return_damper_position),
            "ahu_2_chilled_water_flow_kg_s" => Some(self.ahu_2_chilled_water_flow_kg_s),
            "ahu_2_cooling_coil_condensate_kg_s" => Some(self.ahu_2_cooling_coil_condensate_kg_s),
            "ahu_2_cooling_coil_delivered_kw" => Some(self.ahu_2_cooling_coil_delivered_kw),
            "ahu_2_cooling_coil_request_kw" => Some(self.ahu_2_cooling_coil_request_kw),
            "ahu_2_cooling_coil_valve_actual_position" => {
                Some(self.ahu_2_cooling_coil_valve_actual_position)
            }
            "ahu_2_cooling_coil_valve_measured_position" => {
                Some(self.ahu_2_cooling_coil_valve_measured_position)
            }
            "ahu_2_cooling_coil_valve_position" => Some(self.ahu_2_cooling_coil_valve_position),
            "ahu_2_duct_pressure" => Some(self.ahu_2_duct_pressure),
            "ahu_2_heating_coil_delivered_kw" => Some(self.ahu_2_heating_coil_delivered_kw),
            "ahu_2_heating_coil_request_kw" => Some(self.ahu_2_heating_coil_request_kw),
            "ahu_2_heating_coil_valve_actual_position" => {
                Some(self.ahu_2_heating_coil_valve_actual_position)
            }
            "ahu_2_heating_coil_valve_measured_position" => {
                Some(self.ahu_2_heating_coil_valve_measured_position)
            }
            "ahu_2_heating_coil_valve_position" => Some(self.ahu_2_heating_coil_valve_position),
            "ahu_2_hot_water_flow_kg_s" => Some(self.ahu_2_hot_water_flow_kg_s),
            "ahu_2_intake_opening" => Some(self.ahu_2_intake_opening),
            "ahu_2_measured_fan_speed" => Some(self.ahu_2_measured_fan_speed),
            "ahu_2_measured_mixed_air_c" => Some(self.ahu_2_measured_mixed_air_c),
            "ahu_2_measured_outdoor_damper_position" => {
                Some(self.ahu_2_measured_outdoor_damper_position)
            }
            "ahu_2_measured_return_damper_position" => {
                Some(self.ahu_2_measured_return_damper_position)
            }
            "ahu_2_mixed_air_c" => Some(self.ahu_2_mixed_air_c),
            "ahu_2_mixed_air_enthalpy_kj_kg" => Some(self.ahu_2_mixed_air_enthalpy_kj_kg),
            "ahu_2_outdoor_air_fraction" => Some(self.ahu_2_outdoor_air_fraction),
            "ahu_2_outdoor_airflow" => Some(self.ahu_2_outdoor_airflow),
            "ahu_2_return_air_c" => Some(self.ahu_2_return_air_c),
            "ahu_2_return_air_relative_humidity" => Some(self.ahu_2_return_air_relative_humidity),
            "ahu_2_supply_actual_c" => Some(self.ahu_2_supply_actual_c),
            "ahu_2_supply_air_enthalpy_kj_kg" => Some(self.ahu_2_supply_air_enthalpy_kj_kg),
            "ahu_2_supply_air_relative_humidity" => Some(self.ahu_2_supply_air_relative_humidity),
            "ahu_2_supply_setpoint_c" => Some(self.ahu_2_supply_setpoint_c),
            "avg_zone_c" => Some(self.avg_zone_c),
            "boiler_available" => Some(self.boiler_available),
            "boiler_running" => Some(self.boiler_running),
            "boiler_used_kw" => Some(self.boiler_used_kw),
            "chilled_water_actual_flow_kg_s" => Some(self.chilled_water_actual_flow_kg_s),
            "chilled_water_bypass_flow_kg_s" => Some(self.chilled_water_bypass_flow_kg_s),
            "chilled_water_differential_pressure_pa" => {
                Some(self.chilled_water_differential_pressure_pa)
            }
            "chilled_water_enable" => Some(self.chilled_water_enable),
            "chilled_water_flow_kg_s" => Some(self.chilled_water_flow_kg_s),
            "chilled_water_pump_running" => Some(self.chilled_water_pump_running),
            "chilled_water_pump_speed" => Some(self.chilled_water_pump_speed),
            "chilled_water_return_temp_c" => Some(self.chilled_water_return_temp_c),
            "chilled_water_supply_setpoint_c" => Some(self.chilled_water_supply_setpoint_c),
            "chilled_water_supply_temp_c" => Some(self.chilled_water_supply_temp_c),
            "chiller_available" => Some(self.chiller_available),
            "chiller_running" => Some(self.chiller_running),
            "chiller_used_kw" => Some(self.chiller_used_kw),
            "cool_kw" => Some(self.cool_kw),
            "cool_request_kw" => Some(self.cool_request_kw),
            "delivered_airflow" => Some(self.delivered_airflow),
            "electric_kw" => Some(self.electric_kw),
            "gas_kw" => Some(self.gas_kw),
            "heat_kw" => Some(self.heat_kw),
            "heat_pump_ambient_exchange_kw" => Some(self.heat_pump_ambient_exchange_kw),
            "heat_pump_available" => Some(self.heat_pump_available),
            "heat_pump_available_capacity_kw" => Some(self.heat_pump_available_capacity_kw),
            "heat_pump_chilled_water_cop_multiplier" => {
                Some(self.heat_pump_chilled_water_cop_multiplier)
            }
            "heat_pump_cooling_available_capacity_kw" => {
                Some(self.heat_pump_cooling_available_capacity_kw)
            }
            "heat_pump_cooling_cop" => Some(self.heat_pump_cooling_cop),
            "heat_pump_cooling_used_kw" => Some(self.heat_pump_cooling_used_kw),
            "heat_pump_cop_multiplier" => Some(self.heat_pump_cop_multiplier),
            "heat_pump_electric_kw" => Some(self.heat_pump_electric_kw),
            "heat_pump_heating_available_capacity_kw" => {
                Some(self.heat_pump_heating_available_capacity_kw)
            }
            "heat_pump_heating_cop" => Some(self.heat_pump_heating_cop),
            "heat_pump_heating_used_kw" => Some(self.heat_pump_heating_used_kw),
            "heat_pump_hot_water_cop_multiplier" => Some(self.heat_pump_hot_water_cop_multiplier),
            "heat_pump_part_load_fraction" => Some(self.heat_pump_part_load_fraction),
            "heat_pump_running" => Some(self.heat_pump_running),
            "heat_request_kw" => Some(self.heat_request_kw),
            "hot_water_actual_flow_kg_s" => Some(self.hot_water_actual_flow_kg_s),
            "hot_water_bypass_flow_kg_s" => Some(self.hot_water_bypass_flow_kg_s),
            "hot_water_differential_pressure_pa" => Some(self.hot_water_differential_pressure_pa),
            "hot_water_enable" => Some(self.hot_water_enable),
            "hot_water_flow_kg_s" => Some(self.hot_water_flow_kg_s),
            "hot_water_plant_proven" => Some(self.hot_water_plant_proven),
            "hot_water_pump_running" => Some(self.hot_water_pump_running),
            "hot_water_pump_speed" => Some(self.hot_water_pump_speed),
            "hot_water_reheat_flow_kg_s" => Some(self.hot_water_reheat_flow_kg_s),
            "hot_water_return_temp_c" => Some(self.hot_water_return_temp_c),
            "hot_water_supply_setpoint_c" => Some(self.hot_water_supply_setpoint_c),
            "hot_water_supply_temp_c" => Some(self.hot_water_supply_temp_c),
            "outdoor_c" => Some(self.outdoor_c),
            "plant_boiler_start_fault" => Some(self.plant_boiler_start_fault),
            "plant_chiller_start_fault" => Some(self.plant_chiller_start_fault),
            "plant_heat_pump_start_fault" => Some(self.plant_heat_pump_start_fault),
            "preconditioning_mode" => Some(self.preconditioning_mode),
            "requested_airflow" => Some(self.requested_airflow),
            "schedule_occupied" => Some(self.schedule_occupied),
            "water_pump_kw" => Some(self.water_pump_kw),
            _ => None,
        }
    }
}
