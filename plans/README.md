# Modularization work plans

These plans split `ace-cdl-explorer` into a Cargo workspace whose core crates can be used by
anyone working with CDL and CXF, while the Explorer app keeps working at every step.

Architecture diagrams (current and target): https://claude.ai/artifact/4MgGAedJPaZSgLJTR2WQGD

## Decisions already made

| Topic | Decision |
|---|---|
| Canonical model | The in-memory model follows ASHRAE 231 / CXF (classes, instances, typed connectors, parameters with units). The `python-cdl` simplified JSON becomes a compatibility format read and written by an adapter. |
| Equations | Free-form `equations` are not part of the standard. They move behind an off-by-default `equations` Cargo feature and are represented as extension blocks. |
| Crate names | `cdl-model`, `cdl-blocks`, `cdl-exec`, `cxf`, `cdl-compat`, `cdl-verify`, `cdl-cli` are working names. Final names are chosen after the architecture has been built and tested (before WP7 publishes anything). |

## Work packages

| # | Plan | Depends on | Size | Outcome |
|---|---|---|---|---|
| 1 | [Workspace and in-place split](01-workspace-split.md) | none | M | `lib.rs` broken into modules, no behaviour change |
| 2 | [Canonical model](02-canonical-model.md) | 1 | L | `cdl-model` (S231-aligned) and `cdl-compat` (legacy JSON adapter) |
| 3 | [Block registry](03-block-registry.md) | 2 | L | `cdl-blocks`: one definition per elementary block |
| 4 | [Executor](04-executor.md) | 3 | L | `cdl-exec`: one engine, equations behind a feature |
| 5 | [CXF crate](05-cxf.md) | 2 | M | `cxf`: JSON-LD import and export in Rust |
| 6 | [App re-plumb](06-app-replumb.md) | 4, 5 | M | `ace-building-sim`, `ace-cdl-wasm`, generated TS types |
| 7 | [Verification harness and CLI](07-verify-and-cli.md) | 4, 5 | M | `cdl-verify`, `cdl-cli`, first publish |
| 8 | [Python bindings](08-python-bindings.md) | 5, 7 | M | Optional: shared validator and CXF reader for `python-cdl` and `ace-cdl-bog` |

WP5 can run in parallel with WP3 and WP4 once WP2 has merged. Sizes are relative (M is roughly
half of L); nobody has estimated them in days.

```
WP1 ── WP2 ─┬─ WP3 ── WP4 ─┬─ WP6
            └─ WP5 ────────┼─ WP7 ── WP8
                           └─ (WP6 and WP7 both need WP4 and WP5)
```

## Rules that apply to every package

- **The gate stays green.** Every PR passes the existing checks unchanged in meaning:
  `cargo test --release`, `verify_annual_balance`, `python scripts/verify-obc.py`,
  `sh scripts/build-wasm.sh`, `node scripts/smoke-wasm.mjs`, `npm run test:cxf`, `npm run build`,
  and the "regenerate examples, expect no diff" step in `runtime-checks.yml`.
- **Behaviour changes are separate PRs.** A PR either moves code or changes behaviour, not both.
  Annual energy totals and OBC traces are the regression oracle; a move-only PR must not change them.
- **Dependency direction is one way.** Core crates (`cdl-model`, `cdl-blocks`, `cdl-exec`, `cxf`,
  `cdl-compat`, `cdl-verify`) never depend on `wasm-bindgen`, `arrow-*`, or anything under
  `ace-building-sim`. CI enforces this with a `cargo tree` check added in WP1.
- **Core crates build for native and `wasm32-unknown-unknown`.** CI builds both from WP2 onward.
- **Squash-merge feature branches**, one PR per checklist group where practical.

## Facts the plans rely on

Measured on `main` at `73f562f`:

- `runtime/src/lib.rs` is 24,170 lines: lines 1–5420 are code, 5421–24170 are 286 inline tests.
- `known_type` lists 43 short block names. Five have no class in the pinned `CXF-Core.jsonld`
  (`LimPID`, `Filter`, `LimRateLimiter`, `Gain`, `Delay`). Twelve are ambiguous across the
  Reals / Integers / Logical packages (for example `Add`, `Switch`, `Constant`, `Greater`).
- `CXF-Core.jsonld` defines 132 elementary block classes.
- `four_pipe_g36.json` has 7,077 leaf blocks; 3,666 already use fully qualified
  `Buildings.Controls.OBC.CDL.*` names and the rest use short names. No bundled example uses `equations`.
- `CompiledExecutor::try_new` returns `None` on an unsupported leaf, and the simulation then falls
  back to the interpreted `Executor`.
