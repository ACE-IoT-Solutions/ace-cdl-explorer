# Prototype controller state and plant boundary

These APIs ship in version **0.1.1** of `cdl-blocks`, `cdl-exec` and
`ace-building-sim`. Enable `cdl-exec/serde` for serialized checkpoints, or
`ace-building-sim/checkpoint` when using the building adapter.

## Controller checkpoints

Enable `cdl-exec`'s optional `serde` feature, which also enables `cdl-blocks/serde`.
For the building adapter, enable `ace-building-sim/checkpoint` instead; it forwards
the same feature. Default core builds retain their prior dependency boundary.

`Kernel::snapshot()` produces a `BlockSnapshot` with family-specific `BlockMemory`:
PID integrals/derivative history/reset flags, sampled phases/held memory, delay
values, edge/hysteresis/latch state, timer ages and moving-mean buffers. It excludes
slot addresses and immutable coefficients. Configuration identity is checked on
restore; this is an opaque versioned identity, not a public parameter-editing API.
Comparator memory is included even when the existing event-rollback convention
omits that family. Event iteration behavior itself is unchanged.

`State::snapshot() -> Result<StateSnapshot, SnapshotError>` includes those block payloads,
canonical connector values (including held outputs), input-assignment flags and a
SHA-256 identity of the entire canonical Document. `State::restore(&snapshot)`
validates first and commits atomically. Recompile the same Document, instantiate,
then restore: numeric slot order and per-process program IDs may differ. Existing
handles for the target program remain valid. All aliases must agree, constants
must match their configured values, and unknown/missing entries, configuration
changes, nonfinite values or unknown schema versions are rejected.

Both schemas currently use version 1. Serde encodes state reals as decimal strings
parsed with Rust's correctly rounded float parser. This preserves every finite f64
and signed zero across JSON round trips without globally changing the application's
JSON parser behavior. Boolean/integer metadata retain their native representations.
Changing the meaning/layout of stored memory requires an explicit schema migration
or version bump. These are trusted runtime checkpoints, not authentication tokens.

```rust,ignore
let saved = serde_json::to_string(&controller.state().snapshot()?)?;
let mut replacement = BuildingController::new(&document)?;
replacement.state_mut().restore(&serde_json::from_str(&saved)?)?;
```

A controller checkpoint does not contain plant thermal state, weather position or
the host's clock. Keep the physical plant running during an online controller swap;
a whole-process restart must restore those host/plant concerns separately.

## Explicit bumpless initialization

Exact restore requires identical configuration. For a retuned or replacement PID,
assign its current measured inputs, then call `State::track_output(instance_path,
actuator_output)`. The block owns the initialization calculation. PI/PID integrals
are chosen to reproduce that output, derivative history starts at the current error
(zero derivative contribution), and an already-high reset signal is acknowledged
without synthesizing a rising edge. A zero-duration evaluation at unchanged inputs
preserves the tracked output within floating-point precision; subsequent nonzero
steps integrate normally. Targets outside limits, ill-conditioned/unrepresentable
values, missing inputs, and unsupported P/PD/stateless families return errors.

```rust,ignore
replacement.set_inputs(&observation.sensors)?;
replacement.state_mut().track_output("building.ahu_1.controller", actual_command)?;
let frame = replacement.step(&observation.sensors, 0.0)?;
```

The path must identify the actual PI/PID instance in the compiled graph. Tracking
one PID does not migrate unrelated timers, latches or controller supervisory modes.
Use an exact checkpoint for an identical controller; orchestration decides which
outputs need tracking for a different controller.

## Public physical plant

`Plant::new(weather, scenario)` accepts no Document, registry or executor. It retains
the fixed 10-zone/2-AHU/3-source teaching topology and 35040 quarter-hour weather
samples. Scenario selects a fixed integration interval: 1, 15, 30, 60, 300 or 900 seconds.
All step durations must match that interval; arbitrary variable stepping is not
silently approximated. Use `BuildingController::aligned_step` before construction
when coupling a sampled/timed CDL controller.

- `observe(&mut self)` returns `PlantObservation` with time, interval and typed
  `PlantSensors`. Weather/fault/presence preparation happens once per interval;
  repeated observations return the cached values without advancing time.
- `step(&PlantCommands, dt_seconds)` advances only physics with default reporting.
- `step_reported(&PlantCommands, &ReportingContext, dt_seconds)` additionally records
  supervisory status. Confirmed occupancy and preconditioning are reporting fields;
  physical presence, moisture and comfort accounting continue to use the physical
  occupancy model. Invalid commands/durations do not consume observations or advance
  the plant. Finite out-of-range actuator requests retain existing physical clipping.
- `PlantStep` returns time, completion and an optional quarter-hour `SimulationRow`.
  Call `summary()` for recorded totals. `observe`/`step` reject a completed weather
  year; `done()` permits the caller to finish its loop explicitly.

`BuildingController` is the optional native point-binding adapter. It owns a compiled
CDL state, accepts `PlantSensors`, and produces `ControllerStep { commands, reporting,
traces }`. `set_inputs` assigns observations without stepping for tracking; `state`
and `state_mut` expose checkpoint/tracking access. The existing `Simulation` remains
the closed-loop convenience wrapper and retains graph validation, tracing, chunking,
recording cadence and browser behavior. Plant-only rows carry no graph traces; the
closed-loop wrapper attaches selected traces as before.

```rust,ignore
let mut controller = BuildingController::new(&document)?;
let scenario = Scenario {
    step_seconds: controller.aligned_step(900)?,
    ..Scenario::default()
};
let mut plant = Plant::new(weather, scenario)?;
while !plant.done() {
    let observation = plant.observe()?;
    let dt = observation.step_seconds as f64;
    let frame = controller.step(&observation.sensors, dt)?;
    let result = plant.step_reported(&frame.commands, &frame.reporting, dt)?;
    // Consume result.row at completed quarter-hour boundaries.
}
```

## Verification

`tests/checkpoints.rs` in the integration harness checkpoints every one of the 143
pinned cases through JSON, restores into a fresh compilation and compares subsequent
outputs and final memory exactly. Separate tests cover comparator hysteresis,
signed zero, invalid/aliased/constant checkpoints, atomic failure and PI/PID tracking.
`ace-building-sim/tests/plant_api.rs` compares a day of both control graphs against
external plant/controller composition, tests observation idempotence and validation,
and hands off the complete G36 controller after JSON restoration while checking
every actuator/reporting output.

Both annual native snapshots after extraction match the frozen original bytes:
conventional SHA-256 c1da451a4718190f06701317e9c1529070f5f5b0713d11d91560088a9202c445,
G36 SHA-256 ebd4aa7cd33cc7e0b64562e93d59a08d8867c0416f919bb35aa8573cbfb8abb4.

Final verification for this prototype: 393 workspace tests pass (three existing
manual benchmarks ignored); six checkpoint regressions and five plant/API regressions
pass, including the full G36 command handoff. All 143 cases agree with the independent
pyfunnel check. Four browser tests pass, including full-year G36 summary parity.
Core strict Clippy, warning-denied docs, Rust 1.85 all-feature checks, wasm build/smoke,
generated-contract checks, 220 graph tests/typecheck and the frontend build pass.

## One-second operation (0.1.2)

Version 0.1.2 adds this extension. Version 0.1.1 requires at least 15 seconds.

Set `Scenario { step_seconds: 1, ..Scenario::default() }` before constructing the
plant, then call `plant.step(&commands, 1.0)` for each physical second. Observations
advance every second; weather still comes from the current quarter-hour sample and
rows remain available every 900 steps. `total()` counts integration steps, so a
one-second year has 31,536,000 steps. The default remains 900 seconds.

`BuildingController::aligned_step(1)` and the `Simulation` wrapper also accept this
explicit one-second mode. Controller event periods/shifts must be whole seconds;
subsecond events are rejected. Larger timestep requests retain their previous
alignment behavior.
