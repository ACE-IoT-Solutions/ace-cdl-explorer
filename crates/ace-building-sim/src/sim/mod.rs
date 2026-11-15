mod contract;
mod heat_pump;
mod psychro;
mod row;
mod scenario;
pub use row::SimulationRow;
mod weather;
#[cfg(all(test, ace_building_sim_repository_tests))]
use crate::exec::step::sampled_step;
#[cfg(all(test, ace_building_sim_repository_tests))]
use crate::exec::CompiledExecutor;
mod points;
use crate::model::*;
use crate::validate::validate;
use contract::*;
pub use heat_pump::*;
use points::{ControlInputs, Controls};
use psychro::*;
pub use scenario::*;
use serde::Serialize;
#[cfg(all(test, ace_building_sim_repository_tests))]
use serde_json::{json, Value};
#[cfg(all(test, ace_building_sim_repository_tests))]
use std::collections::HashMap;

pub use weather::{Weather, WeatherRecord, MEASURE_COLUMNS};

#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SimulationTotals {
    pub electricity_kwh: f64,
    pub gas_kwh: f64,
    pub unmet_zone_hours: f64,
    pub occupied_unmet_zone_hours: f64,
}

mod controller;
pub use controller::{BuildingController, ControllerStep};
mod io;
pub use io::*;
mod plant;
pub use plant::{Plant, PlantObservation, PlantStep};
/// Closed-loop composition of a controller and the public physical plant.
pub struct Simulation {
    plant: Plant,
    compiled: Controls,
    trace_targets: Vec<String>,
    #[cfg(all(test, ace_building_sim_repository_tests))]
    graph: Block,
    #[cfg(all(test, ace_building_sim_repository_tests))]
    oracle: Option<cdl_runtime_oracle::Executor>,
}
#[cfg(all(test, ace_building_sim_repository_tests))]
impl std::ops::Deref for Simulation {
    type Target = Plant;
    fn deref(&self) -> &Plant {
        &self.plant
    }
}
#[cfg(all(test, ace_building_sim_repository_tests))]
impl std::ops::DerefMut for Simulation {
    fn deref_mut(&mut self) -> &mut Plant {
        &mut self.plant
    }
}
fn simulation_totals(sim: &Plant) -> SimulationTotals {
    SimulationTotals {
        electricity_kwh: sim.electricity_kwh,
        gas_kwh: sim.gas_kwh,
        unmet_zone_hours: sim.unmet_hours,
        occupied_unmet_zone_hours: sim.occupied_unmet_hours,
    }
}

/// Deterministic per-zone presence, sampled with the 15-minute weather data.
/// Arrival, departure, and occasional midday absences differ by zone.
fn raw_zone_occupancy(weather_index: usize, zone: usize, scheduled: bool) -> bool {
    if !scheduled {
        return false;
    }
    let quarter = weather_index % 96;
    let day = weather_index / 96;
    quarter >= 31 + zone % 5
        && quarter < 70 - zone % 4
        && !(zone % 3 == 0 && (48..52).contains(&quarter))
        && !((day + zone) % 7 == 0 && (56..60).contains(&quarter))
}

fn ahu_return_value(values: &[f64; 10], airflow: &[f64; 10], ahu: usize) -> f64 {
    let start = ahu * 5;
    let total_airflow: f64 = airflow[start..start + 5].iter().sum();
    if total_airflow > 0.0 {
        (start..start + 5)
            .map(|zone| values[zone] * airflow[zone])
            .sum::<f64>()
            / total_airflow
    } else {
        values[start..start + 5].iter().sum::<f64>() / 5.0
    }
}

fn ahu_return_enthalpy_kj_kg(
    temperatures: &[f64; 10],
    humidity_ratios: &[f64; 10],
    airflow: &[f64; 10],
    ahu: usize,
) -> f64 {
    let zone_enthalpy = std::array::from_fn(|zone| {
        moist_air_enthalpy_kj_kg(temperatures[zone], humidity_ratios[zone])
    });
    ahu_return_value(&zone_enthalpy, airflow, ahu)
}

impl Simulation {
    pub fn new(
        document: cdl_model::Document,
        records: Vec<WeatherRecord>,
        scenario: Scenario,
    ) -> Result<Self, SimError> {
        let diagnostics = cdl_model::validate(&document);
        if !diagnostics.is_empty() {
            return Err(SimError(
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }
        let graph: Block = serde_json::from_value(
            cdl_compat::to_value(&document).map_err(|errors| SimError(format!("{errors:?}")))?,
        )
        .map_err(|error| SimError(error.to_string()))?;
        let mut errors = validate(&graph);
        errors.extend(g36_control_contract(&graph));
        errors.extend(conventional_control_contract(&graph));
        errors.extend(undriven_building_outputs(&graph));
        if !errors.is_empty() {
            return Err(SimError::from_str(&errors.join("; ")));
        }
        if graph
            .blocks
            .iter()
            .filter(|b| b.block_type == "VAVBoxController")
            .count()
            != 10
            || !graph.blocks.iter().any(|b| b.block_type == "PlantDispatch")
            || graph
                .blocks
                .iter()
                .filter(|b| b.block_type == "AHUController")
                .count()
                != 2
        {
            return Err(SimError::from_str(
                "Building simulation requires ten VAV, two AHU, and one plant composite",
            ));
        }
        let compiled = Controls::new(&document, &graph).map_err(SimError)?;
        let mut plant = Plant::new(records, scenario)?;
        plant.effective_step_seconds = compiled
            .aligned_step(plant.scenario.step_seconds)
            .map_err(SimError)?;
        Ok(Self {
            plant,
            compiled,
            trace_targets: Vec::new(),
            #[cfg(all(test, ace_building_sim_repository_tests))]
            graph,
            #[cfg(all(test, ace_building_sim_repository_tests))]
            oracle: None,
        })
    }
    pub fn step_chunk(&mut self, count: usize) -> Result<SimulationChunk, SimError> {
        let rows = self.advance_chunk(count)?;
        Ok(SimulationChunk {
            rows,
            progress: self.plant.cursor,
            total: self.total(),
            done: self.done(),
            summary: self.plant.summary(),
        })
    }
    pub fn trace_targets(&self) -> &[String] {
        &self.trace_targets
    }
    pub fn progress(&self) -> usize {
        self.plant.cursor
    }
    pub fn effective_step_seconds(&self) -> usize {
        self.plant.effective_step_seconds
    }
    pub fn total(&self) -> usize {
        self.plant.weather.len() * (900 / self.plant.effective_step_seconds)
    }
    pub fn done(&self) -> bool {
        self.plant.cursor == self.total()
    }
    pub fn summary(&self) -> SimulationTotals {
        self.plant.summary()
    }

    fn advance_chunk(&mut self, count: usize) -> Result<Vec<SimulationRow>, SimError> {
        let mut rows = Vec::new();
        let end = (self.plant.cursor + count.min(672)).min(self.plant.total());
        while self.plant.cursor < end {
            self.plant.ensure_prepared()?;
            let control_inputs = self.plant.sensors();
            let dt_seconds = self.plant.effective_step_seconds as f64;
            let record_traces =
                (self.plant.cursor + 1) % (900 / self.plant.effective_step_seconds) == 0;

            #[cfg(not(all(test, ace_building_sim_repository_tests)))]
            let (controls, traces) = self
                .compiled
                .execute(control_inputs, dt_seconds, record_traces)
                .map_err(|e| SimError::from_str(&e))?;
            #[cfg(all(test, ace_building_sim_repository_tests))]
            let (controls, traces) = if let Some(oracle) = &mut self.oracle {
                oracle.trace_targets.clone_from(&self.trace_targets);
                oracle.trace_values.clear();
                oracle.skip_traces = !record_traces;
                let controls = oracle
                    .execute(
                        &self.graph,
                        &self.compiled.legacy_inputs(control_inputs),
                        dt_seconds,
                        "",
                    )
                    .map_err(|e| SimError::from_str(&e))?;
                let mut traces = std::collections::BTreeMap::new();
                if record_traces {
                    for target in &self.trace_targets {
                        let value = oracle.trace_values.get(target).ok_or_else(|| {
                            SimError::from_str(&format!(
                                "Selected graph signal {target} has no value"
                            ))
                        })?;
                        let numeric = value
                            .as_f64()
                            .or_else(|| value.as_bool().map(|flag| if flag { 1.0 } else { 0.0 }))
                            .ok_or_else(|| {
                                SimError::from_str(
                                    "Selected graph signal is not numeric or Boolean",
                                )
                            })?;
                        traces.insert(target.clone(), numeric);
                    }
                }
                (self.compiled.oracle_outputs(&controls)?, traces)
            } else {
                self.compiled
                    .execute(control_inputs, dt_seconds, record_traces)
                    .map_err(|e| SimError::from_str(&e))?
            };

            let (commands, reporting) = controls.into_plant();
            if let Some(row) = self.plant.advance_commands(&commands, &reporting, traces)? {
                rows.push(row);
            }
        }
        Ok(rows)
    }
    pub fn set_traces(&mut self, targets: Vec<String>) -> Result<(), SimError> {
        self.compiled
            .set_traces(&targets)
            .map_err(|e| SimError::from_str(&e))?;
        #[cfg(all(test, ace_building_sim_repository_tests))]
        if let Some(oracle) = &mut self.oracle {
            oracle.trace_targets = targets.clone();
        }
        self.trace_targets = targets;
        Ok(())
    }
}

#[cfg(all(test, ace_building_sim_repository_tests))]
mod tests;

/// Native simulation failure, independent of browser and serialization APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimError(pub String);
impl SimError {
    fn from_str(message: &str) -> Self {
        Self(message.to_owned())
    }
}
impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SimError {}

/// Recorded quarter-hour rows and progress for a bounded execution chunk.
#[derive(Serialize)]
pub struct SimulationChunk {
    pub rows: Vec<SimulationRow>,
    pub progress: usize,
    pub total: usize,
    pub done: bool,
    pub summary: SimulationTotals,
}

#[cfg(all(test, ace_building_sim_repository_tests))]
impl Simulation {
    fn new_test(graph: &str, weather: &str, scenario: &str) -> Result<Self, SimError> {
        let document = cdl_compat::from_json(graph, Default::default())
            .map_err(|e| SimError(format!("{e:?}")))?;
        Self::new(
            document,
            serde_json::from_str(weather).map_err(|e| SimError(e.to_string()))?,
            serde_json::from_str(scenario).map_err(|e| SimError(e.to_string()))?,
        )
    }
    fn step_chunk_json(&mut self, count: usize) -> Result<String, SimError> {
        serde_json::to_string(&self.step_chunk(count)?).map_err(|e| SimError(e.to_string()))
    }
    fn summary_json(&self) -> String {
        serde_json::to_string(&self.summary()).unwrap()
    }
    fn set_traces_json(&mut self, targets: &str) -> Result<(), SimError> {
        self.set_traces(serde_json::from_str(targets).map_err(|e| SimError(e.to_string()))?)
    }
}
