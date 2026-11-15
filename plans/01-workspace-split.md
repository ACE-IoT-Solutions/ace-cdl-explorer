# WP1: Workspace and in-place split

**Depends on:** nothing. **Size:** M. **Behaviour change:** none.

## Goal

Make the seams visible before anything moves across them. After this package the runtime is still
one crate that produces the same wasm, but each concern lives in its own module with its own tests,
and the workspace exists for later crates to join.

## Scope

In: Cargo workspace, module split of `runtime/src/lib.rs`, test relocation, CI guard rails.
Out: any new public API, any change to JSON shapes, any change to `compiled.rs` internals.

## Tasks

### Workspace
- [ ] Add a root `Cargo.toml` with `[workspace]`, `members = ["runtime"]`, shared
      `[workspace.package]` (edition, license, repository) and `[workspace.dependencies]`.
- [ ] Reserve `crates/` for the new crates. Keep `runtime/` at its current path until WP6 so
      `scripts/build-wasm.sh` and the workflows keep working.
- [ ] Update `Swatinem/rust-cache` `workspaces:` in the three workflows that build Rust, and confirm
      `target/` moves to the workspace root without breaking `.gitignore` or `build-wasm.sh`.

### Module split (move-only)
Target layout inside `runtime/src/`, with today's line ranges:

| Module | From `lib.rs` | Contents |
|---|---|---|
| `model.rs` | 13–95 | `Connector`, `Parameter`, `Equation`, `Connection`, `Block`, `split_path` |
| `validate.rs` | 97–399 | `known_type`, `validate_block`, `order`, `validate` |
| `exec/interp.rs` | 401–1300 | `Plan`, `Executor`, `param`, `bool_param`, PID helpers |
| `exec/compiled.rs` | `compiled.rs` | unchanged |
| `exec/step.rs` | 2696–2792 | `gcd`, `sampled_step` |
| `sim/weather.rs` | 1306–1445 | `Weather`, `WeatherRecord`, `MEASURE_COLUMNS` |
| `sim/heat_pump.rs` | 1599–1955 | performance curves and joint map |
| `sim/scenario.rs` | 1956–2470 | `Scenario`, faults, sensor events, defaults |
| `sim/psychro.rs` | 2467–2612 | humidity, enthalpy, coil and hydraulics helpers |
| `sim/contract.rs` | 2793–2998 | `is_g36_graph`, G36 and conventional control contracts |
| `sim/mod.rs` | 2613–2695, 3000–5400 | `Simulation` state and stepping |
| `arrow_out.rs` | 1447–1597 | `arrow_batch` |
| `wasm_api.rs` | 5401–5419 + `#[wasm_bindgen]` attributes | `validate_json`, `execute_json` |

- [ ] Move in the order above, one module per commit, re-exporting from `lib.rs` so the public
      surface (`Block`, `validate`, `Executor`, `CompiledExecutor`, `Simulation`) is unchanged.
- [ ] `Simulation::advance_chunk` (about 1,800 lines) moves as a whole. Do not refactor it here.
- [ ] Mark every item that crosses a module boundary `pub(crate)`; record the list in the PR. That
      list is the starting point for the public APIs designed in WP2 to WP4.

### Tests
- [ ] Split the 286 inline tests into per-module files (`exec/tests.rs`, `sim/tests.rs`, and so on)
      as `#[cfg(test)]` child modules. They use private functions, so they cannot move to the
      top-level `tests/` directory yet.
- [ ] Keep the `include_str!` paths to `web/public/examples` and `verification/` working.
- [ ] Record the test count before and after; it must stay 286 (plus the ignored benchmark).

### Guard rails
- [ ] Add a CI step that fails when a crate under `crates/` has `wasm-bindgen`, `arrow-*`, or a sim
      crate in its `cargo tree`. It is a no-op until WP2 and protects every later package.
- [ ] Add `cargo fmt --check` and `cargo clippy -- -D warnings` to `runtime-checks.yml` if the tree
      is clean; otherwise file the warnings as a follow-up and add the checks as non-blocking.

## Acceptance

- `lib.rs` is under 200 lines and contains only `mod`, `pub use`, and crate docs.
- No source file other than `sim/mod.rs` exceeds 2,000 lines.
- The wasm binary exports the same symbols (`node scripts/smoke-wasm.mjs` passes untouched).
- Annual totals from `verify_annual_balance` and all `verify-obc.py` cases are bit-identical to `main`.

## Risks

- **Large move diffs hide accidental edits.** Reviewers should check with
  `git diff --color-moved=dimmed-zebra`; anything not shown as moved needs a reason.
- **Merge conflicts with ongoing G36 work in `lib.rs`.** Land this package in a quiet window and
  rebase open branches straight after.
