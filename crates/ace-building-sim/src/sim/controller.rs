//! Optional native adapter for composing CDL controllers with the public plant.
use super::*;
/// Native graph controller with pre-resolved building IO handles.
pub struct BuildingController {
    inner: Controls,
}
/// Commands, reporting status and selected traces from one controller evaluation.
pub struct ControllerStep {
    /// Physical actuator requests.
    pub commands: PlantCommands,
    /// Supervisory status used in interval reporting.
    pub reporting: ReportingContext,
    /// Selected internal graph signals.
    pub traces: std::collections::BTreeMap<String, f64>,
}
impl BuildingController {
    /// Compile a controller against the existing fixed building IO contract.
    pub fn new(document: &cdl_model::Document) -> Result<Self, SimError> {
        let graph: Block = serde_json::from_value(
            cdl_compat::to_value(document).map_err(|e| SimError(format!("{e:?}")))?,
        )
        .map_err(|e| SimError(e.to_string()))?;
        Ok(Self {
            inner: Controls::new(document, &graph).map_err(SimError)?,
        })
    }
    /// Resolve an integration interval aligned to controller sampling/timing.
    pub fn aligned_step(&self, requested: usize) -> Result<usize, SimError> {
        self.inner.aligned_step(requested).map_err(SimError)
    }
    /// Assign measured inputs without advancing the controller. Use before
    /// State::track_output when initializing an alternate controller for handover.
    pub fn set_inputs(&mut self, sensors: &PlantSensors) -> Result<(), SimError> {
        self.inner.set_inputs(sensors).map_err(SimError)
    }
    /// Evaluate commands from observations for the supplied step duration.
    pub fn step(
        &mut self,
        sensors: &PlantSensors,
        dt_seconds: f64,
    ) -> Result<ControllerStep, SimError> {
        let (outputs, traces) = self
            .inner
            .execute(sensors, dt_seconds, true)
            .map_err(SimError)?;
        let (commands, reporting) = outputs.into_plant();
        Ok(ControllerStep {
            commands,
            reporting,
            traces,
        })
    }
    /// Select internal graph signals for ControllerStep::traces.
    pub fn set_traces(&mut self, targets: &[String]) -> Result<(), SimError> {
        self.inner.set_traces(targets).map_err(SimError)
    }
    /// Access controller state for checkpoints and read-only inspection.
    pub fn state(&self) -> &cdl_exec::State {
        self.inner.state()
    }
    /// Access controller state for checkpoint restoration or explicit PID tracking.
    pub fn state_mut(&mut self) -> &mut cdl_exec::State {
        self.inner.state_mut()
    }
}
