//! Native plant/controller boundary, independent of graph names and execution.
use super::{points, SimError};
pub use points::{
    AhuCommands, AhuInputs, ControlInputs as PlantSensors, EquipmentInputs, GlobalCommands,
    GlobalInputs, PollutionInputs, ZoneCommands, ZoneInputs,
};
use serde::{Deserialize, Serialize};
/// Physical actuator requests for the fixed three-source/two-AHU/ten-zone plant.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlantCommands {
    /// Water plant, source/interlock and pump commands.
    pub global: GlobalCommands,
    /// Commands for the two air handlers.
    pub ahu: [AhuCommands; 2],
    /// Commands for the ten terminal units.
    pub zone: [ZoneCommands; 10],
}
/// Controller status used for reporting, not actuator motion or physical occupancy.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportingContext {
    /// Controller-reported preconditioning mode.
    pub preconditioning_mode: bool,
    /// Controller-confirmed occupancy reported alongside the physical presence model.
    pub occupancy_confirmed: [bool; 10],
}
impl PlantCommands {
    pub(super) fn validate(&self) -> Result<(), SimError> {
        if !self.global.valid()
            || self.ahu.iter().any(|v| !v.valid())
            || self.zone.iter().any(|v| !v.valid())
            || ![0., 1., 2., 3.].contains(&self.global.plant_heat_pump_requested_mode)
        {
            return Err(SimError(
                "plant commands must be finite; heat pump mode must be 0, 1, 2 or 3".into(),
            ));
        }
        Ok(())
    }
}
impl points::ControlOutputs {
    pub(super) fn into_plant(self) -> (PlantCommands, ReportingContext) {
        let reporting = ReportingContext {
            preconditioning_mode: self.global.preconditioning_mode,
            occupancy_confirmed: std::array::from_fn(|i| self.zone[i].occupancy_confirmed),
        };
        (
            PlantCommands {
                global: self.global.commands,
                ahu: self.ahu.map(|a| a.commands),
                zone: self.zone.map(|z| z.commands),
            },
            reporting,
        )
    }
}
