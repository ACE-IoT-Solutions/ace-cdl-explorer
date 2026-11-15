//! Fixed plant/control contract. All external point names are defined here.
//! Names are resolved once; the physics loop uses typed records and port handles.
use super::SimError;
use crate::model::Block;
use cdl_blocks::Registry;
use cdl_exec::{PortHandle, Program, Scalar, State};
use cdl_model::{DataType, Document};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
/// Sensor readings, calendar and environmental inputs for the fixed building.
pub struct ControlInputs {
    pub global: GlobalInputs,
    pub ahu: [AhuInputs; 2],
    pub zone: [ZoneInputs; 10],
    pub equipment: [EquipmentInputs; 3],
    pub pollution: [PollutionInputs; 3],
}
#[derive(Default)]
pub(super) struct ControlOutputs {
    pub global: GlobalOutputs,
    pub ahu: [AhuOutputs; 2],
    pub zone: [ZoneOutputs; 10],
}
#[derive(Clone, Copy)]
struct InputPoint {
    suffix: &'static str,
    read: fn(&ControlInputs, usize) -> Scalar,
}
#[derive(Clone, Copy)]
struct OutputPoint {
    suffix: &'static str,
    write: fn(&mut ControlOutputs, usize, Scalar) -> Result<(), SimError>,
    boolean: bool,
}
trait PointValue: Sized {
    const BOOLEAN: bool;
    fn scalar(self) -> Scalar;
    fn from_scalar(value: Scalar) -> Result<Self, SimError>;
}
impl PointValue for f64 {
    const BOOLEAN: bool = false;
    fn scalar(self) -> Scalar {
        Scalar::Real(self)
    }
    fn from_scalar(value: Scalar) -> Result<Self, SimError> {
        match value {
            Scalar::Real(v) if v.is_finite() => Ok(v),
            Scalar::Integer(v) => Ok(v as f64),
            _ => Err(SimError("Control output must be a finite number".into())),
        }
    }
}
impl PointValue for bool {
    const BOOLEAN: bool = true;
    fn scalar(self) -> Scalar {
        Scalar::Boolean(self)
    }
    fn from_scalar(value: Scalar) -> Result<Self, SimError> {
        match value {
            Scalar::Boolean(v) => Ok(v),
            Scalar::Real(v) if v.is_finite() => Ok(v != 0.0),
            Scalar::Integer(v) => Ok(v != 0),
            _ => Err(SimError(
                "Control output must be Boolean or a finite number".into(),
            )),
        }
    }
}
macro_rules! input_group {
    ($kind:ident, |$values:ident, $index:ident| $group:expr, { $($field:ident: $ty:ty => $name:literal),* $(,)? }) => {
        #[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
        #[doc=concat!("Sensor group `",stringify!($kind),"`.")]
        pub struct $kind { $(#[doc=concat!("Signal `",$name,"`.")] pub $field: $ty,)* }
        impl $kind {
            const POINTS: &'static [InputPoint] = &[$(InputPoint {
                suffix: $name,
                read: |$values: &ControlInputs, $index: usize| ($group).$field.scalar(),
            },)*];
        }
    }
}
macro_rules! output_group {
    ($kind:ident, $commands:ident, { $($report:ident: $rty:ty => $rname:literal),* }, |$values:ident, $index:ident| $group:expr, { $($field:ident: $ty:ty => $name:literal),* $(,)? }) => {
        #[doc=concat!("Actuator commands for `",stringify!($kind),"`.")]
        #[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
        pub struct $commands { $(#[doc=concat!("Command `",$name,"`.")] pub $field: $ty,)* }
        impl $commands {pub(super) fn valid(&self)->bool {
            let finite=|v:Scalar|match v {Scalar::Real(n)=>n.is_finite(),_=>true};
            true $(&& finite(self.$field.scalar()))*
        }}
        #[derive(Default)]
        pub(super) struct $kind {pub commands:$commands, $(pub $report:$rty,)*}
        impl std::ops::Deref for $kind {type Target=$commands; fn deref(&self)->&$commands {&self.commands}}
        impl std::ops::DerefMut for $kind {fn deref_mut(&mut self)->&mut $commands {&mut self.commands}}
        impl $kind {
            const POINTS: &'static [OutputPoint] = &[$(OutputPoint {
                suffix: $name, boolean: <$ty as PointValue>::BOOLEAN,
                write: |$values: &mut ControlOutputs, $index: usize, value: Scalar| {
                    ($group).$field = <$ty as PointValue>::from_scalar(value)?; Ok(())
                },
            },)* $(OutputPoint {
                suffix:$rname,boolean:<$rty as PointValue>::BOOLEAN,
                write: |$values:&mut ControlOutputs,$index:usize,value:Scalar| {
                    ($group).$report=<$rty as PointValue>::from_scalar(value)?;Ok(())
                },
            },)*];
        }
    }
}

input_group!(GlobalInputs, |values, _index| values.global, {
    chilled_water_differential_pressure_pa: f64 => "chilled_water_differential_pressure_pa",
    chilled_water_flow_kg_s: f64 => "chilled_water_flow_kg_s",
    chilled_water_pump_running: bool => "chilled_water_pump_running",
    chilled_water_return_temp_c: f64 => "chilled_water_return_temp_c",
    chilled_water_supply_temp_c: f64 => "chilled_water_supply_temp_c",
    cool_request_kw: f64 => "cool_request_kw",
    heat_pump_available_capacity_kw: f64 => "heat_pump_available_capacity_kw",
    heat_pump_cooling_available_capacity_kw: f64 => "heat_pump_cooling_available_capacity_kw",
    heat_pump_heating_available_capacity_kw: f64 => "heat_pump_heating_available_capacity_kw",
    heat_request_kw: f64 => "heat_request_kw",
    hot_water_differential_pressure_pa: f64 => "hot_water_differential_pressure_pa",
    hot_water_flow_kg_s: f64 => "hot_water_flow_kg_s",
    hot_water_plant_proven: bool => "hot_water_plant_proven",
    hot_water_pump_running: bool => "hot_water_pump_running",
    hot_water_return_temp_c: f64 => "hot_water_return_temp_c",
    hot_water_supply_temp_c: f64 => "hot_water_supply_temp_c",
    local_day_seconds: f64 => "local_day_seconds",
    outdoor_air_enthalpy_kj_kg: f64 => "outdoor_air_enthalpy_kj_kg",
    outdoor_c: f64 => "outdoor_c",
    plant_alarm_acknowledge: bool => "plant_alarm_acknowledge",
    pollution_on_duration_s: f64 => "pollution_on_duration_s",
    pollution_operator: f64 => "pollution_operator",
    schedule_occupied: bool => "schedule_occupied",
    seconds_until_occupied: f64 => "seconds_until_occupied",
});

input_group!(AhuInputs, |values, index| values.ahu[index], {
    actual_cooling_coil_valve_position: f64 => "actual_cooling_coil_valve_position",
    actual_fan_speed: f64 => "actual_fan_speed",
    actual_heating_coil_valve_position: f64 => "actual_heating_coil_valve_position",
    actual_outdoor_damper_position: f64 => "actual_outdoor_damper_position",
    actual_return_damper_position: f64 => "actual_return_damper_position",
    actual_supply_air_temp: f64 => "actual_supply_air_temp",
    alarm_acknowledge: bool => "alarm_acknowledge",
    duct_pressure: f64 => "duct_pressure",
    flow_request: f64 => "flow_request",
    freeze_reset: bool => "freeze_reset",
    freeze_stat_trip: bool => "freeze_stat_trip",
    mixed_air_temp: f64 => "mixed_air_temp",
    outdoor_airflow: f64 => "outdoor_airflow",
    return_air_enthalpy_kj_kg: f64 => "return_air_enthalpy_kj_kg",
    return_air_temp: f64 => "return_air_temp",
    supply_fan_proven: bool => "supply_fan_proven",
});

input_group!(ZoneInputs, |values, index| values.zone[index], {
    actual_damper_position: f64 => "actual_damper_position",
    actual_reheat_valve_position: f64 => "actual_reheat_valve_position",
    alarm_acknowledge: bool => "alarm_acknowledge",
    co2_ppm: f64 => "co2_ppm",
    discharge_temp: f64 => "discharge_temp",
    low_airflow_acknowledge: bool => "low_airflow_acknowledge",
    measured_airflow: f64 => "measured_airflow",
    occupancy_count: f64 => "occupancy_count",
    occupied: bool => "occupied",
    local_override: bool => "override",
    room_temp: f64 => "room_temp",
    supply_pressure: f64 => "supply_pressure",
    window_closed: bool => "window_closed",
});

input_group!(EquipmentInputs, |values, index| values.equipment[index], {
    available: bool => "available",
    fault_reset: bool => "fault_reset",
    running: bool => "running",
});

input_group!(PollutionInputs, |values, index| values.pollution[index], {
    available: bool => "available",
    value: f64 => "",
});

output_group!(GlobalOutputs, GlobalCommands, {preconditioning_mode: bool => "preconditioning_mode"}, |values, _index| values.global, {
    plant_boiler_kw: f64 => "plant_boiler_kw",
    plant_boiler_start_command: bool => "plant_boiler_start_command",
    plant_boiler_start_fault: bool => "plant_boiler_start_fault",
    plant_chilled_water_enable: bool => "plant_chilled_water_enable",
    plant_chilled_water_pump_speed: f64 => "plant_chilled_water_pump_speed",
    plant_chilled_water_supply_setpoint_c: f64 => "plant_chilled_water_supply_setpoint_c",
    plant_chiller_kw: f64 => "plant_chiller_kw",
    plant_chiller_start_command: bool => "plant_chiller_start_command",
    plant_chiller_start_fault: bool => "plant_chiller_start_fault",
    plant_heat_pump_cooling_kw: f64 => "plant_heat_pump_cooling_kw",
    plant_heat_pump_heating_kw: f64 => "plant_heat_pump_heating_kw",
    plant_heat_pump_requested_mode: f64 => "plant_heat_pump_requested_mode",
    plant_heat_pump_start_command: bool => "plant_heat_pump_start_command",
    plant_heat_pump_start_fault: bool => "plant_heat_pump_start_fault",
    plant_hot_water_enable: bool => "plant_hot_water_enable",
    plant_hot_water_pump_speed: f64 => "plant_hot_water_pump_speed",
    plant_hot_water_supply_setpoint_c: f64 => "plant_hot_water_supply_setpoint_c",
});

output_group!(AhuOutputs, AhuCommands, {}, |values, index| values.ahu[index], {
    cooling_coil_valve_position: f64 => "cooling_coil_valve_position",
    fan_speed: f64 => "fan_speed",
    heating_coil_valve_position: f64 => "heating_coil_valve_position",
    outdoor_air_damper_position: f64 => "outdoor_air_damper_position",
    return_air_damper_position: f64 => "return_air_damper_position",
    supply_air_temp: f64 => "supply_air_temp",
});

output_group!(ZoneOutputs, ZoneCommands, {occupancy_confirmed: bool => "occupancy_confirmed"}, |values, index| values.zone[index], {
    airflow_request: f64 => "airflow_request",
    damper_position: f64 => "damper_position",
    reheat_valve_position: f64 => "reheat_valve_position",
});

/// Required output names shared by handle resolution and plant contract checks.
pub(super) fn output_names(g36: bool) -> Vec<String> {
    let mut names: Vec<_> = GlobalOutputs::POINTS
        .iter()
        .map(|p| name("", p.suffix))
        .collect();
    for index in 1..=2 {
        names.extend(
            AhuOutputs::POINTS
                .iter()
                .map(|p| name(&format!("ahu_{index}"), p.suffix)),
        );
    }
    for index in 1..=10 {
        let prefix = format!("zone_{index:02}");
        names.extend(ZoneOutputs::POINTS.iter().map(|p| name(&prefix, p.suffix)));
        // G36's mode status is required for its contract, although the physics
        // consumes the separate confirmed-occupancy and actuator points.
        if g36 {
            names.push(name(&prefix, "conditioning_state"));
        }
    }
    names
}

struct InputBinding {
    handle: PortHandle,
    read: fn(&ControlInputs, usize) -> Scalar,
    index: usize,
    #[cfg(all(test, ace_building_sim_repository_tests))]
    name: String,
}
struct OutputBinding {
    handle: PortHandle,
    write: fn(&mut ControlOutputs, usize, Scalar) -> Result<(), SimError>,
    index: usize,
    #[cfg(all(test, ace_building_sim_repository_tests))]
    name: String,
}
pub(super) struct Controls {
    program: Program,
    state: State,
    inputs: Vec<InputBinding>,
    outputs: Vec<OutputBinding>,
    traces: Vec<(String, PortHandle)>,
}
fn error(error: impl std::fmt::Display) -> SimError {
    SimError(error.to_string())
}
fn name(prefix: &str, suffix: &str) -> String {
    if prefix.is_empty() {
        suffix.into()
    } else if suffix.is_empty() {
        prefix.into()
    } else {
        format!("{prefix}_{suffix}")
    }
}
impl Controls {
    pub fn new(document: &Document, graph: &Block) -> Result<Self, String> {
        Self::compile(document, graph).map_err(|e| e.to_string())
    }
    fn compile(document: &Document, graph: &Block) -> Result<Self, SimError> {
        let program =
            Program::compile(document, &Registry::new(document.library.clone())).map_err(error)?;
        let input_names: HashSet<&str> = graph.inputs.iter().map(|p| p.name.as_str()).collect();
        let mut inputs = Vec::new();
        let mut bind_inputs =
            |points: &[InputPoint], prefix: &str, index: usize| -> Result<(), SimError> {
                for point in points {
                    let name = name(prefix, point.suffix);
                    if input_names.contains(name.as_str()) {
                        inputs.push(InputBinding {
                            handle: program.input(&name).map_err(error)?,
                            read: point.read,
                            index,
                            #[cfg(all(test, ace_building_sim_repository_tests))]
                            name,
                        });
                    }
                }
                Ok(())
            };
        bind_inputs(GlobalInputs::POINTS, "", 0)?;
        for index in 0..2 {
            bind_inputs(AhuInputs::POINTS, &format!("ahu_{}", index + 1), index)?;
        }
        for index in 0..10 {
            bind_inputs(ZoneInputs::POINTS, &format!("zone_{:02}", index + 1), index)?;
        }
        for (index, prefix) in ["heat_pump", "boiler", "chiller"].iter().enumerate() {
            bind_inputs(EquipmentInputs::POINTS, prefix, index)?;
        }
        for (index, prefix) in ["pollution_pm25", "pollution_ozone", "pollution_aqi"]
            .iter()
            .enumerate()
        {
            bind_inputs(PollutionInputs::POINTS, prefix, index)?;
        }
        // Reject unsupported raw points instead of silently assigning zero.
        if inputs.len() != input_names.len() {
            let missing = graph
                .inputs
                .iter()
                .find(|p| {
                    program
                        .input(&p.name)
                        .is_ok_and(|h| !inputs.iter().any(|binding| binding.handle == h))
                })
                .expect("unbound input");
            return Err(SimError(format!("Missing physical input {}", missing.name)));
        }
        let mut outputs = Vec::new();
        let mut bind_outputs =
            |points: &[OutputPoint], prefix: &str, index: usize| -> Result<(), SimError> {
                for point in points {
                    let name = name(prefix, point.suffix);
                    let handle = program
                        .output(&name)
                        .map_err(|_| SimError(format!("Missing {name} output")))?;
                    if !point.boolean && *handle.data_type() == DataType::Boolean {
                        return Err(SimError(format!("Control output {name} must be numeric")));
                    }
                    outputs.push(OutputBinding {
                        handle,
                        write: point.write,
                        index,
                        #[cfg(all(test, ace_building_sim_repository_tests))]
                        name,
                    });
                }
                Ok(())
            };
        bind_outputs(GlobalOutputs::POINTS, "", 0)?;
        for index in 0..2 {
            bind_outputs(AhuOutputs::POINTS, &format!("ahu_{}", index + 1), index)?;
        }
        for index in 0..10 {
            bind_outputs(
                ZoneOutputs::POINTS,
                &format!("zone_{:02}", index + 1),
                index,
            )?;
        }
        let state = program.instantiate();
        Ok(Self {
            program,
            state,
            inputs,
            outputs,
            traces: Vec::new(),
        })
    }
    pub fn aligned_step(&self, requested: usize) -> Result<usize, String> {
        self.program
            .aligned_step(requested as u64)
            .map(|v| v as usize)
            .map_err(|e| e.to_string())
    }
    pub fn set_traces(&mut self, targets: &[String]) -> Result<(), String> {
        let traces = targets
            .iter()
            .map(|name| {
                self.program
                    .trace(name)
                    .map(|h| (name.clone(), h))
                    .map_err(|_| format!("Selected graph signal {name} has no value"))
            })
            .collect::<Result<_, _>>()?;
        self.traces = traces;
        Ok(())
    }
    pub(super) fn state(&self) -> &State {
        &self.state
    }
    pub(super) fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }
    pub(super) fn set_inputs(&mut self, inputs: &ControlInputs) -> Result<(), String> {
        // Validate the complete input batch first without allocating per step.
        for binding in &self.inputs {
            coerce(
                (binding.read)(inputs, binding.index),
                binding.handle.data_type(),
            )
            .map_err(|e| e.to_string())?;
        }
        for binding in &self.inputs {
            let value = coerce(
                (binding.read)(inputs, binding.index),
                binding.handle.data_type(),
            )
            .map_err(|e| e.to_string())?;
            self.state
                .set(&binding.handle, value)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn execute(
        &mut self,
        inputs: &ControlInputs,
        dt: f64,
        record_traces: bool,
    ) -> Result<(ControlOutputs, BTreeMap<String, f64>), String> {
        self.step(inputs, dt, record_traces)
            .map_err(|e| e.to_string())
    }
    fn step(
        &mut self,
        inputs: &ControlInputs,
        dt: f64,
        record_traces: bool,
    ) -> Result<(ControlOutputs, BTreeMap<String, f64>), SimError> {
        self.set_inputs(inputs).map_err(SimError)?;
        self.state.step(dt).map_err(error)?;
        let mut outputs = ControlOutputs::default();
        for binding in &self.outputs {
            (binding.write)(
                &mut outputs,
                binding.index,
                self.state.get(&binding.handle).map_err(error)?,
            )?;
        }
        let traces = if record_traces {
            self.traces
                .iter()
                .map(|(name, handle)| {
                    let value = self.state.get(handle).map_err(error)?;
                    let numeric = match value {
                        Scalar::Boolean(v) => {
                            if v {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        other => f64::from_scalar(other)?,
                    };
                    Ok((name.clone(), numeric))
                })
                .collect::<Result<_, SimError>>()?
        } else {
            BTreeMap::new()
        };
        Ok((outputs, traces))
    }
}
// Preserve the old host's Boolean/numeric input coercion at this plant boundary.
// The reusable executor remains strictly typed.
fn coerce(value: Scalar, ty: &DataType) -> Result<Scalar, SimError> {
    match ty {
        DataType::Boolean => Ok(Scalar::Boolean(bool::from_scalar(value)?)),
        DataType::Integer => {
            let v = match value {
                Scalar::Boolean(_) => return Err(SimError("Invalid Integer input".into())),
                other => f64::from_scalar(other)?,
            };
            if v.fract() != 0.0 {
                return Err(SimError("Invalid Integer input".into()));
            }
            Ok(Scalar::Integer(v as i64))
        }
        _ => Ok(Scalar::Real(match value {
            Scalar::Boolean(v) => {
                if v {
                    1.0
                } else {
                    0.0
                }
            }
            other => f64::from_scalar(other)?,
        })),
    }
}
#[cfg(all(test, ace_building_sim_repository_tests))]
impl Controls {
    pub fn legacy_inputs(
        &self,
        inputs: &ControlInputs,
    ) -> std::collections::HashMap<String, serde_json::Value> {
        self.inputs
            .iter()
            .map(|b| {
                let value = match (b.read)(inputs, b.index) {
                    Scalar::Real(v) => serde_json::json!(v),
                    Scalar::Integer(v) => serde_json::json!(v),
                    Scalar::Boolean(v) => serde_json::json!(v),
                };
                (b.name.clone(), value)
            })
            .collect()
    }
    pub fn oracle_outputs(
        &self,
        values: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<ControlOutputs, SimError> {
        let mut outputs = ControlOutputs::default();
        for binding in &self.outputs {
            let value = values
                .get(&binding.name)
                .ok_or_else(|| SimError(format!("Missing {} output", binding.name)))?;
            let scalar = if let Some(v) = value.as_bool() {
                Scalar::Boolean(v)
            } else {
                Scalar::Real(
                    value
                        .as_f64()
                        .ok_or_else(|| SimError(format!("Invalid {} output", binding.name)))?,
                )
            };
            (binding.write)(&mut outputs, binding.index, scalar)?;
        }
        Ok(outputs)
    }
}
