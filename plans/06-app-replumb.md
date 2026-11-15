# WP6: App re-plumb

**Depends on:** WP4, WP5. **Size:** M. **Behaviour change:** exported CXF files change slightly (see WP5).

## Goal

Turn what is left of `runtime/` into two app-specific crates on top of the core, and remove the
hand-maintained copies on the TypeScript side.

- `ace-building-sim`: the four-pipe plant, `Scenario`, faults, control contracts. Native Rust API.
- `ace-cdl-wasm`: `wasm-bindgen` exports and Arrow IPC encoding. Nothing else.

## Tasks

### `ace-building-sim`
- [ ] Move `sim/*` from WP1 into `crates/ace-building-sim`. Constructor takes a `cdl-model`
      `Document`, weather and `Scenario` as Rust values and returns `Result<_, SimError>`; no JSON
      strings and no `JsValue`.
- [ ] Resolve every control point to a `cdl-exec` port handle once in the constructor. The roughly
      70 string-keyed input inserts and the `output("plant_boiler_kw")` style reads in
      `advance_chunk` become handle reads. Collect the names in one `points.rs` table so the
      contract between plant and controls is in a single reviewable place.
- [ ] Keep the fixed 10 VAV / 2 AHU / 1 plant topology. Generalising it is not part of this plan.
- [ ] Row output becomes a typed struct (or column builder), not `serde_json::Value`. Arrow encoding
      moves out; the sim crate has no `arrow-*` dependency.
- [ ] Move `baseline` and `verify_annual_balance` binaries here.

### `ace-cdl-wasm`
- [ ] Move `wasm_api.rs` and `arrow_out.rs` here; rename the directory from `runtime/` and update
      `scripts/build-wasm.sh`, `verify-wasm-table.py`, `smoke-wasm.mjs` and the workflows.
- [ ] Keep the existing JS surface (`Simulation`, `validate_json`, `execute_json`) so
      `runtime.worker.ts` needs no change in the first PR.
- [ ] Add `cxf_import(json) -> project graph JSON` and `cxf_export(graph JSON) -> json`, backed by
      the `cxf` and `cdl-compat` crates.
- [ ] Add `block_catalog() -> json` from the WP3 catalog export and `aligned_step(graph, requested)`.
- [ ] Return validation diagnostics as structured JSON (`path`, `code`, `message`).

### Web
- [ ] Replace `web/src/lib/cxf.ts` with calls into wasm. Keep `web/tests/cxf.test.ts`, pointed at
      the wasm functions, as an end-to-end check.
- [ ] Generate `blockCatalog.ts` content from `block_catalog()` at build time; delete the
      "keep this list in sync" comment and the hand-written entries. `blockHelp.ts` text that the
      standard file does not provide stays as an overlay keyed by class.
- [ ] Generate the TS types for `Scenario`, row columns and diagnostics from Rust. Recommended:
      `ts-rs` derive behind a `ts` feature on `ace-building-sim`, with a CI step that fails when the
      generated file is stale. Delete the hand-written copies in `types.ts`.
- [ ] Delete `effectiveStepForGraph` from `types.ts`; call `aligned_step`. Keep
      `effectiveStep.test.ts` as a test of the wasm function.
- [ ] Leave the editor's in-memory graph in the simplified JSON shape, and leave the project v1 file
      format and the 101 migrations alone. They read and write through `cdl-compat`.

### Docs
- [ ] Update `README.md`, `docs/runtime-and-model.md` and `docs/cxf-conformance.md` for the new
      layout and for which CI check now covers what.

## Acceptance

- `runtime/` no longer exists; the workspace has the crates shown in the target diagram.
- `web/src/lib` has no hand-written copy of `Scenario`, the block list or step alignment.
- A full-year G36 run in the browser gives the same four summary totals as before this package.
- Saved projects from before this package load and run (test with `projectStorage.test.ts` fixtures
  and at least one real exported project file).
- Wasm bundle size is recorded before and after; an increase over 15% needs an explanation.

## Follow-ups this package deliberately leaves open

- **Editor on the canonical model.** Moving the editor to classes and instances (with a "make
  unique" action for per-instance edits) and a project v2 format is the natural next step for
  interop, and it is large enough to plan separately once this lands.
- **Migrations.** The 101 `*Migration.ts` files could become a versioned registry. Not needed for
  modularization.
- **Example graphs on standard classes.** Changing the Python generators in `scripts/` to emit fully
  qualified standard classes would shrink the `ace:` extension list from WP3.
