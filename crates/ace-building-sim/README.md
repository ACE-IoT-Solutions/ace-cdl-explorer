# Native building simulation

`ace-building-sim` owns the fixed four-pipe teaching plant: ten VAV zones, two AHUs
and one central plant. Controls execute through `cdl-exec`. Construction accepts a
canonical `cdl_model::Document`, `Vec<WeatherRecord>` and `Scenario`; failures are
native `SimError` values. The constructor and stepping API require no JSON strings,
`JsValue`, browser host or Arrow libraries.

```rust
use ace_building_sim::{Scenario, SimError, Simulation, SimulationChunk, WeatherRecord};
fn first_chunk(document: cdl_model::Document, weather: Vec<WeatherRecord>, scenario: Scenario)
    -> Result<SimulationChunk, SimError>
{
    let mut sim = Simulation::new(document, weather, scenario)?;
    sim.step_chunk(672)
}
```

Weather contains 35,040 quarter-hour records. The requested scenario timestep is
aligned to controller timing. Each bounded chunk advances at most 672 internal
steps, returning progress, four cumulative summary totals, and typed
`SimulationRow` records at completed quarter-hour boundaries. Zone arrays retain
length ten; water-loop arrays retain length two. Trace targets are supplied as
`Vec<String>`. The browser wrapper and Arrow IPC encoder live in `ace-cdl-wasm`.

`baseline` and `verify_annual_balance` are repository-only binaries in the
unpublished `ace-building-sim-tools` package. The baseline runner
retains the historical JSON serialization/parsing boundary so its artifacts can be
compared byte for byte with pre-migration output. The former `obc_verify` runner has moved to the reusable `cdl-verify` crate and
`cdl verify` CLI; the Python script retains independent funnel comparisons.

Control points are resolved once in the constructor from the single `points.rs`
table. The physics loop reads and writes typed records through those handles.
The optional `ts` feature derives frontend contracts. The repository tools package's `generate-types` binary
exports scenario, row, summary and diagnostic types plus the native column lists;
`--check` detects drift without rewriting files. The browser compatibility suite
loads real pre-migration exports and verifies a completed G36 year against the
original browser. The project v1 format and migrations remain in the compatibility
shape.

Version 0.1.1 exposes a controller-free `Plant`: construct it from weather
and `Scenario`, call `observe()`, then `step(&PlantCommands, dt_seconds)`. Its interval
is fixed by Scenario. `step_reported` accepts separate supervisory reporting fields.
`BuildingController` supplies the optional native graph binding adapter; the existing
`Simulation` composes these responsibilities with unchanged recording/tracing order.
Enable `checkpoint` to use versioned JSON checkpoints through the controller's state.
See `docs/controller-state-and-plant-api.md` for a complete composition/handover example.

Rust 1.85 is the minimum supported version. Original project code is MIT licensed;
LICENSE includes complete upstream terms. Repository-dependent differential tests
are enabled only in the full source checkout; published archives also contain
self-contained math/API tests and need no Explorer files or original-runtime oracle.

A controller-free native step needs no graph or JavaScript:

```rust
use ace_building_sim::{Plant,PlantCommands,Scenario,Weather,WeatherRecord};
let sample = WeatherRecord {
    weather: Weather { outdoor_c: 12.0, solar_kw: 0.0, internal_kw: 0.0, occupied: false },
    relative_humidity: Some(0.5),
};
let mut plant = Plant::new(vec![sample;35_040],Scenario::default())?;
let observation = plant.observe()?;
let result = plant.step(&PlantCommands::default(),observation.step_seconds as f64)?;
assert_eq!(plant.progress(),1);
# Ok::<(),ace_building_sim::SimError>(())
```
