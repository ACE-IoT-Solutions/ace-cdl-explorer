use super::heat_pump::*;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct EquipmentOutage {
    pub equipment: String,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct TerminalFault {
    pub zone: usize,
    pub kind: String,
    #[serde(default)]
    pub leak_fraction: Option<f64>,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct TerminalValveSensorEvent {
    pub zone: usize,
    pub bias: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct AhuDamperFault {
    pub ahu: usize,
    pub damper: String,
    pub stuck_position: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct AhuCoilValveFault {
    pub ahu: usize,
    pub service: String,
    pub stuck_position: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct AhuDamperSensorEvent {
    pub ahu: usize,
    pub damper: String,
    pub bias: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct AhuCoilValveSensorEvent {
    pub ahu: usize,
    pub service: String,
    pub bias: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct AhuMixedAirSensorEvent {
    pub ahu: usize,
    pub bias_c: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct OutdoorTemperatureEvent {
    pub delta_c: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct AirQualityEvent {
    pub pollutant: String,
    pub value: Option<f64>,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct ClockOffsetEvent {
    pub start_hour: f64,
    pub utc_offset_hours: f64,
}

pub(crate) fn local_day_seconds(
    elapsed_hours: f64,
    base_offset_hours: f64,
    events: &[ClockOffsetEvent],
) -> f64 {
    let utc_offset_hours = events
        .iter()
        .filter(|event| elapsed_hours >= event.start_hour)
        .max_by(|a, b| a.start_hour.total_cmp(&b.start_hour))
        .map_or(base_offset_hours, |event| event.utc_offset_hours);
    ((elapsed_hours + utc_offset_hours) * 3600.0).rem_euclid(86_400.0)
}

pub(crate) fn ahu_damper_position(
    faults: &[AhuDamperFault],
    ahu: usize,
    damper: &str,
    hour: f64,
    command: f64,
) -> f64 {
    faults
        .iter()
        .rev()
        .find(|fault| {
            fault.ahu == ahu
                && fault.damper == damper
                && hour >= fault.start_hour
                && hour < fault.start_hour + fault.duration_hours
        })
        .map_or(command, |fault| fault.stuck_position)
}

pub(crate) fn ahu_coil_valve_position(
    faults: &[AhuCoilValveFault],
    ahu: usize,
    service: &str,
    hour: f64,
    position: f64,
) -> f64 {
    faults
        .iter()
        .rev()
        .find(|fault| {
            fault.ahu == ahu
                && fault.service == service
                && hour >= fault.start_hour
                && hour < fault.start_hour + fault.duration_hours
        })
        .map_or(position, |fault| fault.stuck_position)
}

pub(crate) fn terminal_fault_active(
    faults: &[TerminalFault],
    zone: usize,
    kind: &str,
    hour: f64,
) -> bool {
    faults.iter().any(|fault| {
        fault.zone == zone
            && fault.kind == kind
            && hour >= fault.start_hour
            && hour < fault.start_hour + fault.duration_hours
    })
}

pub(crate) fn terminal_reheat_leak_fraction(
    faults: &[TerminalFault],
    zone: usize,
    hour: f64,
) -> Option<f64> {
    faults
        .iter()
        .rev()
        .find(|fault| {
            fault.zone == zone
                && fault.kind == "reheat_valve_leak"
                && hour >= fault.start_hour
                && hour < fault.start_hour + fault.duration_hours
        })
        .map(|fault| fault.leak_fraction.unwrap_or(0.3))
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct EquipmentStartupSeconds {
    #[serde(default)]
    pub heat_pump: f64,
    #[serde(default)]
    pub boiler: f64,
    #[serde(default)]
    pub chiller: f64,
}

impl Default for EquipmentStartupSeconds {
    fn default() -> Self {
        Self {
            heat_pump: 0.0,
            boiler: 0.0,
            chiller: 0.0,
        }
    }
}

pub(crate) fn equipment_available(outages: &[EquipmentOutage], equipment: &str, hour: f64) -> bool {
    !outages.iter().any(|outage| {
        outage.equipment == equipment
            && hour >= outage.start_hour
            && hour < outage.start_hour + outage.duration_hours
    })
}

pub(crate) fn measured_water_flow(
    actual_kg_s: f64,
    loop_name: &str,
    hour: f64,
    events: &[WaterFlowSensorEvent],
) -> f64 {
    let bias = events
        .iter()
        .rev()
        .find(|event| {
            event.loop_name == loop_name
                && hour >= event.start_hour
                && hour < event.start_hour + event.duration_hours
        })
        .map_or(0.0, |event| event.bias_kg_s);
    (actual_kg_s + bias).max(0.0)
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct Scenario {
    #[serde(default = "default_ua")]
    pub zone_ua_kw_per_k: f64,
    #[serde(default = "default_cap")]
    pub zone_capacity_kwh_per_k: f64,
    #[serde(default = "default_terminal_design_flow")]
    pub terminal_design_flow: [f64; 10],
    #[serde(default)]
    pub terminal_reheat_valve_stroke_seconds: [f64; 10],
    #[serde(default)]
    pub ahu_heating_coil_valve_stroke_seconds: [f64; 2],
    #[serde(default)]
    pub ahu_cooling_coil_valve_stroke_seconds: [f64; 2],
    #[serde(default = "default_ahu_fan_design_airflow")]
    pub ahu_fan_design_airflow: [f64; 2],
    #[serde(default)]
    pub ahu_fan_startup_seconds: [f64; 2],
    #[serde(default = "default_ahu_damper_stroke_seconds")]
    pub ahu_damper_stroke_seconds: f64,
    #[serde(default = "default_water_pump_design_flow")]
    pub water_pump_design_flow_kg_s: [f64; 2],
    #[serde(default = "default_water_volume")]
    pub water_volume_kg: [f64; 2],
    #[serde(default)]
    pub water_pump_startup_seconds: [f64; 2],
    #[serde(default = "default_water_pump_shutoff_pressure")]
    pub water_pump_shutoff_pressure_pa: [f64; 2],
    #[serde(default = "default_water_bypass_conductance")]
    pub water_bypass_conductance: [f64; 2],
    #[serde(default = "default_water_coil_conductance")]
    pub water_coil_conductance: [f64; 2],
    #[serde(default = "default_hot_reheat_conductance")]
    pub hot_reheat_conductance: f64,
    #[serde(default = "default_zone_air_volume")]
    pub zone_air_volume_m3: f64,
    #[serde(default)]
    pub zone_air_volume_m3_by_zone: Option<[f64; 10]>,
    #[serde(default = "default_zone_infiltration")]
    pub zone_infiltration_m3_s: f64,
    #[serde(default)]
    pub zone_infiltration_m3_s_by_zone: Option<[f64; 10]>,
    #[serde(default = "default_open_window_infiltration")]
    pub open_window_additional_infiltration_m3_s: f64,
    #[serde(default)]
    pub open_window_additional_infiltration_m3_s_by_zone: Option<[f64; 10]>,
    #[serde(default = "default_zone_occupants")]
    pub design_occupants_per_zone: f64,
    #[serde(default)]
    pub zone_design_occupants: Option<[f64; 10]>,
    #[serde(default = "default_co2_generation")]
    pub co2_generation_lps_per_person: f64,
    #[serde(default = "default_occupant_sensible_heat")]
    pub occupant_sensible_heat_kw_per_person: f64,
    #[serde(default = "default_outdoor_co2")]
    pub outdoor_co2_ppm: f64,
    #[serde(default)]
    pub outdoor_relative_humidity: Option<f64>,
    #[serde(default)]
    pub return_air_relative_humidity: Option<f64>,
    #[serde(default = "default_occupant_moisture")]
    pub occupant_moisture_kg_h_per_person: f64,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(type = "0 | 1 | 2"))]
    pub pollution_operator: i64,
    #[serde(default = "default_pollution_on_duration")]
    pub pollution_on_duration_s: f64,
    #[serde(default)]
    pub local_utc_offset_hours: f64,
    #[serde(default)]
    pub clock_offset_events: Vec<ClockOffsetEvent>,
    #[serde(default)]
    pub outdoor_pm25_ug_m3: Option<f64>,
    #[serde(default)]
    pub outdoor_ozone_ppm: Option<f64>,
    #[serde(default)]
    pub outdoor_aqi: Option<f64>,
    #[serde(default)]
    pub air_quality_events: Vec<AirQualityEvent>,
    #[serde(default = "default_heat_pump_capacity")]
    pub heat_pump_capacity_kw: f64,
    #[serde(default = "default_heat_recovery_savings_fraction")]
    pub heat_recovery_compressor_savings_fraction: f64,
    #[serde(default = "default_heat_pump_performance")]
    pub heat_pump_performance: Vec<HeatPumpPerformancePoint>,
    #[serde(default = "default_heat_pump_part_load_performance")]
    pub heat_pump_part_load_performance: Vec<HeatPumpPartLoadPoint>,
    #[serde(default = "default_hot_water_cop_performance")]
    pub heat_pump_hot_water_cop_performance: Vec<HeatPumpWaterCopPoint>,
    #[serde(default = "default_chilled_water_cop_performance")]
    pub heat_pump_chilled_water_cop_performance: Vec<HeatPumpWaterCopPoint>,
    #[serde(default)]
    pub heat_pump_joint_performance: Option<HeatPumpJointMap>,
    #[serde(default)]
    pub equipment_outages: Vec<EquipmentOutage>,
    #[serde(default)]
    pub equipment_start_failures: Vec<EquipmentOutage>,
    #[serde(default)]
    pub fan_faults: Vec<EquipmentOutage>,
    #[serde(default)]
    pub fan_speed_sensor_events: Vec<FanSpeedSensorEvent>,
    #[serde(default)]
    pub water_pump_faults: Vec<WaterPumpFault>,
    #[serde(default)]
    pub water_flow_sensor_events: Vec<WaterFlowSensorEvent>,
    #[serde(default)]
    pub terminal_faults: Vec<TerminalFault>,
    #[serde(default)]
    pub terminal_valve_sensor_events: Vec<TerminalValveSensorEvent>,
    #[serde(default)]
    pub ahu_damper_faults: Vec<AhuDamperFault>,
    #[serde(default)]
    pub ahu_coil_valve_faults: Vec<AhuCoilValveFault>,
    #[serde(default)]
    pub ahu_damper_sensor_events: Vec<AhuDamperSensorEvent>,
    #[serde(default)]
    pub ahu_coil_valve_sensor_events: Vec<AhuCoilValveSensorEvent>,
    #[serde(default)]
    pub ahu_mixed_air_sensor_events: Vec<AhuMixedAirSensorEvent>,
    #[serde(default)]
    pub outdoor_temperature_events: Vec<OutdoorTemperatureEvent>,
    #[serde(default)]
    pub window_openings: Vec<WindowOpening>,
    #[serde(default)]
    pub occupancy_overrides: Vec<ZoneOverride>,
    #[serde(default)]
    pub occupancy_sensor_events: Vec<OccupancySensorEvent>,
    #[serde(default)]
    pub occupant_count_events: Vec<OccupantCountEvent>,
    #[serde(default)]
    pub occupancy_count_sensor_events: Vec<OccupantCountEvent>,
    #[serde(default)]
    pub alarm_acknowledgements: Vec<ZoneOverride>,
    #[serde(default)]
    pub ahu_alarm_acknowledgements: Vec<AhuOverride>,
    #[serde(default)]
    pub plant_alarm_acknowledgements: Vec<OperatorPress>,
    #[serde(default)]
    pub equipment_startup_seconds: EquipmentStartupSeconds,
    #[serde(default = "default_disabled_hour")]
    pub heat_pump_fault_reset_hour: f64,
    #[serde(default = "default_disabled_hour")]
    pub boiler_fault_reset_hour: f64,
    #[serde(default = "default_disabled_hour")]
    pub chiller_fault_reset_hour: f64,
    #[serde(default = "default_boiler_capacity")]
    pub boiler_capacity_kw: f64,
    #[serde(default = "default_chiller_capacity")]
    pub chiller_capacity_kw: f64,
    #[serde(default = "default_step_seconds")]
    #[cfg_attr(feature = "ts", ts(type = "15 | 30 | 60 | 300 | 900"))]
    pub step_seconds: usize,
    #[serde(default)]
    pub freeze_test_ahu: usize,
    #[serde(default = "default_disabled_hour")]
    pub freeze_test_start_hour: f64,
    #[serde(default)]
    pub freeze_test_duration_hours: f64,
    #[serde(default = "default_disabled_hour")]
    pub freeze_test_reset_hour: f64,
}

impl Scenario {
    pub(crate) fn air_volume(&self, zone: usize) -> f64 {
        self.zone_air_volume_m3_by_zone
            .map_or(self.zone_air_volume_m3, |values| values[zone])
    }

    pub(crate) fn infiltration(&self, zone: usize) -> f64 {
        self.zone_infiltration_m3_s_by_zone
            .map_or(self.zone_infiltration_m3_s, |values| values[zone])
    }

    pub(crate) fn open_window_infiltration(&self, zone: usize) -> f64 {
        self.open_window_additional_infiltration_m3_s_by_zone
            .map_or(self.open_window_additional_infiltration_m3_s, |values| {
                values[zone]
            })
    }

    pub(crate) fn design_occupants(&self, zone: usize) -> f64 {
        self.zone_design_occupants
            .map_or(self.design_occupants_per_zone, |counts| counts[zone])
    }
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct WindowOpening {
    pub zone: usize,
    pub start_hour: f64,
    pub duration_hours: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct ZoneOverride {
    pub zone: usize,
    pub start_hour: f64,
    pub duration_hours: f64,
}
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct OccupancySensorEvent {
    pub zone: usize,
    pub present: bool,
    pub start_hour: f64,
    pub duration_hours: f64,
}
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct OccupantCountEvent {
    pub zone: usize,
    pub count: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct AhuOverride {
    pub ahu: usize,
    pub start_hour: f64,
    pub duration_hours: f64,
}
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct OperatorPress {
    pub start_hour: f64,
    pub duration_hours: f64,
}
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct WaterPumpFault {
    pub loop_name: String,
    pub start_hour: f64,
    pub duration_hours: f64,
}
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct WaterFlowSensorEvent {
    pub loop_name: String,
    pub bias_kg_s: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(optional_fields = nullable))]
pub struct FanSpeedSensorEvent {
    pub ahu: usize,
    pub bias: f64,
    pub start_hour: f64,
    pub duration_hours: f64,
}
fn default_disabled_hour() -> f64 {
    -1.0
}
fn default_step_seconds() -> usize {
    900
}
fn default_ua() -> f64 {
    0.15
}
fn default_cap() -> f64 {
    5.0
}
fn default_zone_air_volume() -> f64 {
    250.0
}
fn default_zone_infiltration() -> f64 {
    0.01
}
fn default_terminal_design_flow() -> [f64; 10] {
    [0.95; 10]
}
fn default_ahu_fan_design_airflow() -> [f64; 2] {
    [5.0; 2]
}
fn default_ahu_damper_stroke_seconds() -> f64 {
    300.0
}
fn default_water_pump_design_flow() -> [f64; 2] {
    [4.31; 2]
}
fn default_water_volume() -> [f64; 2] {
    [5_000.0; 2]
}
fn default_water_pump_shutoff_pressure() -> [f64; 2] {
    [80_000.0; 2]
}
fn default_water_bypass_conductance() -> [f64; 2] {
    [0.2; 2]
}
fn default_water_coil_conductance() -> [f64; 2] {
    [0.4, 0.8]
}
fn default_hot_reheat_conductance() -> f64 {
    0.4
}
fn default_open_window_infiltration() -> f64 {
    0.2
}
pub(crate) fn default_zone_occupants() -> f64 {
    8.0
}
fn default_co2_generation() -> f64 {
    0.004
}
fn default_occupant_sensible_heat() -> f64 {
    0.075
}
fn default_outdoor_co2() -> f64 {
    420.0
}
fn default_occupant_moisture() -> f64 {
    0.06
}
fn default_pollution_on_duration() -> f64 {
    3600.0
}
fn default_heat_pump_capacity() -> f64 {
    80.0
}
fn default_heat_recovery_savings_fraction() -> f64 {
    1.0
}
fn default_boiler_capacity() -> f64 {
    100.0
}
fn default_chiller_capacity() -> f64 {
    100.0
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            zone_ua_kw_per_k: default_ua(),
            zone_capacity_kwh_per_k: default_cap(),
            terminal_design_flow: default_terminal_design_flow(),
            terminal_reheat_valve_stroke_seconds: Default::default(),
            ahu_heating_coil_valve_stroke_seconds: Default::default(),
            ahu_cooling_coil_valve_stroke_seconds: Default::default(),
            ahu_fan_design_airflow: default_ahu_fan_design_airflow(),
            ahu_fan_startup_seconds: Default::default(),
            ahu_damper_stroke_seconds: default_ahu_damper_stroke_seconds(),
            water_pump_design_flow_kg_s: default_water_pump_design_flow(),
            water_volume_kg: default_water_volume(),
            water_pump_startup_seconds: Default::default(),
            water_pump_shutoff_pressure_pa: default_water_pump_shutoff_pressure(),
            water_bypass_conductance: default_water_bypass_conductance(),
            water_coil_conductance: default_water_coil_conductance(),
            hot_reheat_conductance: default_hot_reheat_conductance(),
            zone_air_volume_m3: default_zone_air_volume(),
            zone_air_volume_m3_by_zone: Default::default(),
            zone_infiltration_m3_s: default_zone_infiltration(),
            zone_infiltration_m3_s_by_zone: Default::default(),
            open_window_additional_infiltration_m3_s: default_open_window_infiltration(),
            open_window_additional_infiltration_m3_s_by_zone: Default::default(),
            design_occupants_per_zone: default_zone_occupants(),
            zone_design_occupants: Default::default(),
            co2_generation_lps_per_person: default_co2_generation(),
            occupant_sensible_heat_kw_per_person: default_occupant_sensible_heat(),
            outdoor_co2_ppm: default_outdoor_co2(),
            outdoor_relative_humidity: Default::default(),
            return_air_relative_humidity: Default::default(),
            occupant_moisture_kg_h_per_person: default_occupant_moisture(),
            pollution_operator: Default::default(),
            pollution_on_duration_s: default_pollution_on_duration(),
            local_utc_offset_hours: Default::default(),
            clock_offset_events: Default::default(),
            outdoor_pm25_ug_m3: Default::default(),
            outdoor_ozone_ppm: Default::default(),
            outdoor_aqi: Default::default(),
            air_quality_events: Default::default(),
            heat_pump_capacity_kw: default_heat_pump_capacity(),
            heat_recovery_compressor_savings_fraction: default_heat_recovery_savings_fraction(),
            heat_pump_performance: default_heat_pump_performance(),
            heat_pump_part_load_performance: default_heat_pump_part_load_performance(),
            heat_pump_hot_water_cop_performance: default_hot_water_cop_performance(),
            heat_pump_chilled_water_cop_performance: default_chilled_water_cop_performance(),
            heat_pump_joint_performance: Default::default(),
            equipment_outages: Default::default(),
            equipment_start_failures: Default::default(),
            fan_faults: Default::default(),
            fan_speed_sensor_events: Default::default(),
            water_pump_faults: Default::default(),
            water_flow_sensor_events: Default::default(),
            terminal_faults: Default::default(),
            terminal_valve_sensor_events: Default::default(),
            ahu_damper_faults: Default::default(),
            ahu_coil_valve_faults: Default::default(),
            ahu_damper_sensor_events: Default::default(),
            ahu_coil_valve_sensor_events: Default::default(),
            ahu_mixed_air_sensor_events: Default::default(),
            outdoor_temperature_events: Default::default(),
            window_openings: Default::default(),
            occupancy_overrides: Default::default(),
            occupancy_sensor_events: Default::default(),
            occupant_count_events: Default::default(),
            occupancy_count_sensor_events: Default::default(),
            alarm_acknowledgements: Default::default(),
            ahu_alarm_acknowledgements: Default::default(),
            plant_alarm_acknowledgements: Default::default(),
            equipment_startup_seconds: Default::default(),
            heat_pump_fault_reset_hour: default_disabled_hour(),
            boiler_fault_reset_hour: default_disabled_hour(),
            chiller_fault_reset_hour: default_disabled_hour(),
            boiler_capacity_kw: default_boiler_capacity(),
            chiller_capacity_kw: default_chiller_capacity(),
            step_seconds: default_step_seconds(),
            freeze_test_ahu: Default::default(),
            freeze_test_start_hour: default_disabled_hour(),
            freeze_test_duration_hours: Default::default(),
            freeze_test_reset_hour: default_disabled_hour(),
        }
    }
}
