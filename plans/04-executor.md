# WP4: Executor

**Depends on:** WP3. **Size:** L. **Behaviour change:** none intended.

## Goal

Create `cdl-exec`: compile a `cdl-model` document once into a flat program and step it. One engine
replaces the interpreted `Executor` and the slot-based `CompiledExecutor`, and it is usable from
native Rust with no wasm or Arrow dependency.

## Public API (proposed)

```rust
let program = Program::compile(&document, registry())?;      // flattens composites, allocates slots
let mut state = program.instantiate();
state.set_input("room_temp", 22.5)?;                         // or by pre-resolved PortHandle
state.step(dt_seconds)?;
let y = state.output("damper_position")?;                    // typed: Real | Integer | Boolean
let handle = program.trace("AHU1.economizer.enable.y")?;     // any internal connector
let step = program.aligned_step(requested_seconds);          // replaces sampled_step + TS copy
```

- Inputs and outputs can be resolved to handles once, so a host such as the building simulator does
  no string lookups in its inner loop.
- Values cross the API typed; internally everything is an `f64` slot as today.
- Errors are a typed enum that implements `std::error::Error`, not `String`.

## Tasks

- [ ] Compiler: flatten the instance tree, order with `cdl-model`'s evaluation order, allocate slots,
      call each block's `build`. Start from `CompiledExecutor::compile` (compiled.rs 382–500).
- [ ] Stepping, including the event handling in `execute` / `restore_event_state` /
      `execute_once` (compiled.rs 939–1100). Read this code closely before porting; it carries the
      timed-transition behaviour the OBC traces depend on.
- [ ] Tracing by instance path, replacing `trace_targets` in both executors.
- [ ] `aligned_step`: port `sampled_step` (lib.rs 2703–2792) using `event_periods` from the registry.
      The TS `effectiveStepForGraph` is deleted in WP6 in favour of this.
- [ ] **Equations feature.** Add `equations` (off by default) that registers an extension block
      evaluating the annotation carried from WP2 with `meval`. Without the feature, compiling a
      document that contains such a block fails with a clear diagnostic. The Explorer's wasm build
      enables the feature so `python-cdl` examples still run.
- [ ] Evaluate `meval` 0.2 (old, unmaintained as far as we know) against alternatives while it is
      isolated behind the feature. Changing it is optional.
- [ ] Switch `Simulation` and `execute_json` to `cdl-exec`. Remove the fallback path.
- [ ] Keep the old interpreted `Executor` in a `dev-dependencies`-only test crate for one release
      as a differential oracle, then delete it. Delete `compiled.rs`.
- [ ] Benchmark: port `benchmark_compiled_vs_dynamic` to compare the new engine with the old
      compiled path on `four_pipe_g36.json`.
- [ ] Move tests that only need the public API into `crates/cdl-exec/tests/`.

## Acceptance

- All 286 existing tests pass, or have a ported equivalent listed in the PR.
- Every case in `verification/suite.json` passes with unchanged tolerances.
- Annual totals from `verify_annual_balance` for both bundled buildings match `main` exactly. If
  floating-point evaluation order makes exact equality impossible, the difference is explained
  block by block and approved before merging.
- Full-year G36 run is no slower than the current compiled path by more than 5% natively and in wasm.
- A 30-line native example in the crate README compiles and runs a PI loop with no JS toolchain.
- `cargo tree -p cdl-exec` (default features) shows no `meval`, `wasm-bindgen` or `arrow-*`.

## Risks

- **Dispatch cost.** `Box<dyn Step>` per block may be slower than the current enum `match`. If the
  benchmark shows it, generate an enum over registry entries with a macro; the `Step` trait stays.
- **Hidden reliance on the interpreter.** Any graph that only ran through the fallback path will
  now fail to compile unless its blocks are in the registry. The WP2 acceptance check (all example
  leaves resolve) plus the equations feature should cover this; confirm with saved user projects
  if any are available.
