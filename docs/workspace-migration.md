# Workspace migration evidence

This tracks implementation of `plans/` without releasing or deploying the app.
The complete goal remains WP1–WP7, with WP8 subject to the demand check in its plan.
Production remains undeployed. Registry publication progress is recorded below.

## WP1: workspace and runtime module split

Baseline: `73f562fc99528181c24cea8b24d81c8a18cdb41f`.

The root Cargo workspace owns the lockfile and target directory. `runtime/` retains its
package name, exports, binaries, and wasm output location. Dependencies and package
edition/repository metadata are shared at the root. The repository has no project
license file, so selecting a publishing license remains a WP7 decision; the workspace
does not invent a license grant.

`runtime/src/lib.rs` now contains only module declarations, public re-exports and crate
documentation (14 lines). The simulation step body moved intact. All Rust
source files except `sim/mod.rs` are under 2,000 lines. Tests are child modules alongside
execution, step alignment, validation, heat-pump and psychrometric modules. Remaining
simulation regression tests share their helpers under `sim/tests/` and are subdivided
at function boundaries to stay below the source-size limit. Fixture paths use
`CARGO_MANIFEST_DIR` so further module moves do not break them.

The original source actually has 286 test functions, including **two** ignored manual
benchmarks (the plan describes only one ignored benchmark). The baseline result was
284 passed, 0 failed, 2 ignored. All 286 test annotations and both ignore annotations
are preserved. All 297 test/helper bodies were compared against the original source,
allowing only whitespace and fixture-path changes. The 12 extracted production modules
were also compared, allowing only imports, module declarations, visibility, whitespace,
and rustfmt trailing commas. The compiled implementation below its imports is unchanged.

CI now checks formatting and the dependency boundary. The dependency check examines
normal and build dependencies transitively, across all targets and all features;
development-only oracle dependencies are allowed. Offline Cargo workspace tests prove
that transitive and wasm-specific Arrow dependencies are rejected and development-only
dependencies are accepted. Core native/wasm builds are added when WP2 creates the first
core crates.

### Clippy follow-up

Clippy is advisory, as WP1 permits for an initially unclean tree. Both the original and
split sources were checked with `cargo clippy --all-targets -- -D warnings` using the
same compiler. The original reports 13 library warnings and 37 including tests; the
split reports the same 13 library warnings and 24 including tests. Categories are:
type complexity; identical conditional bodies; manual `is_multiple_of`; index-based
loops; Boolean `assert_eq!`; cloned singleton slices; and, in the original tests,
field reassignment after `Default`. Fix these in a separate cleanup and then make
Clippy blocking. None is suppressed in source during the move.

### Cross-module visibility inventory

These items are crate-visible rather than new public API. This inventory includes
existing compiled-executor visibility and the imports needed by sibling test modules.
Private helpers used only within their own module remain private.

| Module under `runtime/src` | Crate-visible items |
|---|---|
| `arrow_out.rs` | `arrow_batch` |
| `exec/compiled.rs` | `set_traces` |
| `exec/interp.rs` | `param`, `bool_param`, `PidControllerType`, `with_integral`, `with_derivative`, `pid_controller_type`, `clamp`, `trace_targets`, `trace_values`, `skip_traces` |
| `exec/mod.rs` | `interp`, `step` |
| `exec/step.rs` | `sampled_step` |
| `model.rs` | `source`, `target`, `split_path` |
| `sim/contract.rs` | `is_g36_graph`, `undriven_building_outputs`, `g36_control_contract`, `conventional_control_contract` |
| `sim/heat_pump.rs` | `valid_heat_pump_joint_map`, `heat_pump_joint_value`, `default_heat_pump_performance`, `default_heat_pump_part_load_performance`, `default_hot_water_cop_performance`, `default_chilled_water_cop_performance`, `water_cop_multiplier`, `water_capacity_multiplier`, `valid_water_cop_curve`, `part_load_cop_multiplier`, `heat_pump_performance`, `heat_pump_compressor_work_kw` |
| `sim/mod.rs` | `weather` |
| `sim/psychro.rs` | `humidity_ratio`, `relative_humidity_from_ratio`, `moist_air_enthalpy_kj_kg`, `temperature_from_moist_air_enthalpy_c`, `settle_zone_condensation`, `cooling_coil_request_kw`, `cooling_coil_leaving_state`, `outdoor_air_enthalpy_kj_kg`, `water_loop_hydraulics` |
| `sim/scenario.rs` | `local_day_seconds`, `ahu_damper_position`, `ahu_coil_valve_position`, `terminal_fault_active`, `terminal_reheat_leak_fraction`, `equipment_available`, `measured_water_flow`, `air_volume`, `infiltration`, `open_window_infiltration`, `design_occupants`, `default_zone_occupants` |
| `sim/weather.rs` | `WeatherRecord`, `MEASURE_COLUMNS`, `weather`, `relative_humidity` |
| `validate.rs` | `known_type`, `order` |

### Verification

WP1 runtime parity is verified. License metadata remains deferred to WP7, as explained
above. Do not infer completion of WP2–WP7 from these results.

- Original and split release suites: 284 passed, 0 failed, 2 ignored each. The final source also passes release test compilation.
- Regenerating bundled examples: no diff.
- Source move audit: all production and test bodies preserved as described above.
- Core dependency guard: passes (no core crates yet); all three guard integration tests pass.
- Formatting and whitespace checks: pass.
- OBC comparison: all 143 cases pass for both executors; 286 output CSV files are byte-identical to the original runtime.
- CXF tests: 9 passed.
- Web graph tests: 220 passed; TypeScript and Vite production build pass.
- Wasm build: passes, including the externref table check. Bundle size is 1,760,804 bytes (SHA-256 `bc5cbc132f2c233aaaee47c99341cdb750f052ab592a4b2e7f72d2d053311149`). The unchanged wasm smoke test passes.
- Original annual balance verifier: both bundled buildings pass all 35,040 intervals.
- Split annual balance verifier: both bundled buildings pass all 35,040 intervals.
- G36 full-year output: byte-identical, including sampled rows and all four summary totals. SHA-256: `ebd4aa7cd33cc7e0b64562e93d59a08d8867c0416f919bb35aa8573cbfb8abb4`.
- Conventional building full-year output: byte-identical, including sampled rows and all four summary totals. SHA-256: `c1da451a4718190f06701317e9c1529070f5f5b0713d11d91560088a9202c445`.

### Reproducing the parity comparison

Build `baseline` and `obc_verify` from the baseline revision in a separate worktree,
using its checked-in lockfile. Build the same binaries from the current workspace.
For each binary pair, use the same current examples and verification fixtures:

```sh
baseline four_pipe_building.json /tmp/building-output.json 900
baseline four_pipe_g36.json /tmp/g36-output.json 900
obc_verify verification/suite.json /tmp/obc-traces
```

Use different output paths for the two revisions. Compare the complete baseline JSON
files byte for byte (sampled rows plus summary), and compare both the CSV filenames
and their contents. The baseline binary records every fourth weather row by default;
`verify_annual_balance` independently checks every weather interval. Keep the
`verify-obc.py` pyfunnel check as the independent comparison against reference traces.

## WP2: canonical model and legacy adapter

Implemented `cdl-model` and `cdl-compat`, with the design and worked fixture in
`docs/canonical-model-design.md` and `crates/cdl-integration-tests/examples/cooling_coil.rs`.
The workspace now builds the core crates natively and for wasm in CI.

The model provides separate classes/instances, typed values, arrays, enumeration
references, units, declaration/assignment metadata, structured diagnostics, stable
ordering and flattened instance paths. Array extents use `u64` so their serialized
meaning does not depend on native versus wasm pointer width. Algebraic-loop handling
uses a class property rather than block-name checks in the model.

The adapter reads the pinned core interfaces for name resolution. Standard class
interfaces/default declarations are shared; per-instance connector/parameter metadata
and assignments remain independent. Inline composites remain independent by default,
with opt-in deduplication. Changed values and metadata are exported from the model,
including deletion. Namespaced properties, aliases, mixed edge spellings, numeric
representations and compact unit-IRI spellings round-trip without a whole-graph
snapshot. Equations are carried by explicit extension classes and annotations.
The generated extension inventory is `crates/cdl-compat/EXTENSIONS.md`.

`Simulation::new`, `validate_json` and `execute_json` now parse through the adapter,
validate the canonical structure and then construct the existing legacy `Block`.
Existing per-block validation and both execution engines remain until WP3/WP4.
No simulator stepping, physical equations or executor kernels changed in WP2.

### Verification

WP2 implementation and runtime parity checks pass, including the final array-width
and metadata ownership changes. WP3–WP7 are still outstanding; these results do not
establish parity of the future replacement executor or simulator architecture.

- All 192 discovered/selected graphs round-trip exactly and validate, including every
  one of the 143 verification suite selections.
- All 7,077 G36 leaves resolve uniquely: 6,540 standard and 537 explicit ACE extensions.
- 19 structural model tests and 18 adapter tests cover wiring, class recursion,
  sharing, diagnostics, metadata edits/deletion, alias collisions, numeric literals,
  equations, dimensions and unit-IRI preservation. Both README examples are doctested.
- The native CoolingCoil example resolves six children and eleven wires and validates.
- The runtime release suite passes with 284 passing and two ignored benchmarks.
- OBC verification passes all 143 cases for both existing engines.
- Both buildings pass annual balance verification for all 35,040 weather intervals.
- Native full-year snapshots, including rows and summaries, match baseline byte for
  byte (same hashes recorded under WP1).
- Wasm build and unchanged smoke tests pass; core native/wasm builds pass.
- CXF/web graph tests, TypeScript checks and Vite build pass.
- Core crates pass strict Clippy, `deny(missing_docs)`, and `cargo doc`; the existing
  runtime Clippy cleanup remains advisory as recorded under WP1.
- Production dependency trees contain no wasm bindings, Arrow or simulator crates.
  `cdl-model` has no production JSON-parser dependency; serde_json is test-only.
  Both crates declare Rust 1.85, matching the highest declared dependency MSRV.

### Boundaries for the next packages

The interface decoder is deliberately not a general CXF importer; WP5 moves this
format-specific code and bundled resources into `cxf` and completes its fixture tests.
The current include path reaches the repository schema, so crate publishing/packaging
is not yet ready and remains a WP7 deliverable.

WP3 must audit omitted legacy parameter defaults before canonical execution or CXF
export goes live. Some required standard declarations have no default in the pinned
core (for example Constant.k), whereas the old executors supply legacy fallback values.
The registry must materialize those legacy choices as explicit canonical assignments
or classify the differing behavior as an ACE extension. This must not be inferred from
successful JSON round trips alone. The existing runtime continues using its original
fallback values after conversion back to `Block`.

The intermediate wasm contains the new canonical model/adapter and the complete
legacy runtime. It also embeds the 722,726-byte pinned core for interface resolution.
Its raw size has grown by about 68% from the WP1 1,760,804 bytes to 2,960,311 bytes (624,584 bytes gzip; SHA-256
`3a558cdfdef6543740b2d901bf88021d8bcbcce3702d453b06e2c0aba15b0759`); additional code
includes the model's serde machinery, structural validation and both format conversions.
This is an explained intermediate increase, not the final WP6 bundle result. Record
and reassess final size after the interpreter and redundant frontend implementations
are removed. No production deployment has been performed.

## WP3: registry and elementary behavior

`cdl-blocks` now owns the 43 legacy behavior families, elementary parameter rules,
and slot kernels ported from the production compiled executor. Runtime validation
delegates those rules through a temporary typed-value bridge. Existing composite
execution remains selected until WP4 cutover; no simulator math changed.

The registry implements 52 of the 132 pinned standard interfaces. It claims exact
typed standard IRIs only; altered interfaces and legacy names have explicit ACE
identifiers. Hosts inject the authoritative model Library instead of relying on a
global registry with a second interface copy. The serializable catalog adds labels,
groups and implementation flags, and the format adapter provides JSON encoding.
Production `cdl-blocks` depends only on `cdl-model`.

The compatibility reader now materializes the actual builder-selected fallbacks as
typed assignments. This resolves the missing-standard-default issue recorded under
WP2: for example, omitted legacy Constant.k becomes an explicit zero. Untouched
implicit assignments stay absent in legacy JSON, while edits emit visible bindings.
Dependent hold/reset fallbacks use supplied values, and invalid configurations do
not gain defaults that would make them valid. Native standard instances require
bindings where the pinned declaration has no default.

Verification for this change:

- All 43 families match both existing engines over input and state transitions.
- All 7,077 G36 leaves and 20 elementary verification CSV cases match both engines.
- Interface conformance checks pass for all 52 registered standard classes, including
  parameter-access types and catalog coverage; incompatible/foreign IRIs are rejected.
- All 192 graph selections continue to round-trip exactly and structurally validate.
- The existing runtime suite passes: 284 tests, two ignored benchmarks.
- All 143 OBC cases pass for both existing engines with unchanged tolerances.
- Both buildings pass 35,040-interval air/water/energy balance verification.
- Native/wasm core builds, strict core Clippy, doctests and dependency checks pass.

The original event-checkpoint set omits Greater/Less despite their hysteresis state.
The port preserves this convention deliberately; changing it requires a distinct
behavioral regression. UnitDelay and Pre remain checkpointed as in the original
executor's separate delay/previous-state containers.

## WP4: canonical executor

`cdl-exec` compiles canonical documents into a flat numeric-slot program, aliases
composite wires, uses the model's stable evaluation order, and owns independent
kernel state. It exposes typed, program-scoped input/output/trace handles and central
event alignment. `Simulation` and `execute_json` now use that engine through a thin
legacy host adapter. The simulator has a required executor, with no production
fallback. Its Rust tests may explicitly select the frozen interpreted oracle.

Both original engines and their validation/alignment helpers are retained in the
unpublished `cdl-runtime-oracle`, referenced only through dev-dependencies. Legacy
DTOs live in `cdl-compat`; they do not pull in the simulator or wasm. The 82 old
executor tests are ported to `cdl-exec/tests/legacy`, exercising the canonical API
through the actual host adapter against the interpreted oracle. Their fixture paths
  and the unsupported-class diagnostic assertion were updated; expectations and
tolerances are retained. This harness enables equations because the legacy corpus
contains documentary equations. Default-feature tests separately check their rejection.

The optional `equations` feature is off by default and retains `meval` semantics for
free-form extensions, including ordered assignments and min/max. The Explorer opts
in. Known builtin behavior takes precedence over documentary equations as before.
Default production dependency trees contain no `meval`, wasm, Arrow or simulator.
Replacing the expression parser is deferred until equivalent syntax/numerics are
demonstrated; isolating the current parser avoids an unrelated behavior change.

Verification:

- Direct comparisons match every output of all 143 suite traces against both old
  engines; all 43 families and 7,077 G36 leaves retain their direct kernel checks.
- All 284 original runtime tests pass before relocating the 82 executor tests;
  both old ignored benchmarks are preserved. No numeric expectation was relaxed.
- After relocation, 203 simulator/runtime tests and 81 ported executor tests pass,
  with their two original ignored benchmarks retained. The new performance benchmark
  is a third explicitly invoked/ignored test. The legacy harness also uses minor
  Clippy cleanups (equivalent assertions and borrowed slices); behavior is unchanged.
- State/handle isolation, typed input errors, Pre nonconvergence rollback, event
  alignment, standard declaration drift and equations feature boundaries are tested.
- Both native full-year snapshots match the WP1 files byte for byte, with the same
  SHA-256 hashes recorded above, including rows and summaries.
- Full-year native G36 comparison: baseline 170.265 s, candidate 156.092 s, ratio
  0.917. Sparse row and complete summary objects match exactly.
- Full-year wasm G36 comparison using the same Rust compiler/build settings:
  baseline 223.301 s, candidate 213.370 s, ratio 0.956; complete summaries match
  exactly. Both performance gates satisfy the five-percent maximum regression.
- The 10,000-frame direct native G36 benchmark also passes (ratio 0.887), with
  exact output comparison over its first 100 frames.
- Unchanged wasm smoke tests and all 143 external OBC/funnel cases pass after
  cutover. The two legacy invocation shapes now both use the canonical engine;
  independent old-engine comparisons live in the core differential tests.
- The native PI example is doctested; strict core Clippy, docs and dependency guards
  pass. `cdl-exec` supports wasm without a wasm-bindgen dependency.

The cutover wasm is 3,052,708 bytes raw / 640,439 bytes gzip9 (SHA-256
`e8e290f354fa7db09118ea9746314847ea84b73557b9fe4ee7e0d2993a53beb8`). It is about
3.1% larger than WP2 despite removing both old executors: canonical compilation,
typed bindings, format/model serde and inlined kernel dispatch are now included.
The embedded core remains 722,726 bytes. This remains an intermediate bundle; WP6
removes redundant host/frontend conversions and records the final size. No production
deployment has occurred.

After the additional declaration-drift checks and CXF resource relocation, the current
intermediate wasm is 3,077,059 bytes raw / 644,018 bytes gzip9 (SHA-256
`cdf5a49d6baa8b41d46b18e82f7e11063c8d874469c268fda90d776cebdd6c1a`). Full-year
performance figures above describe the cutover artifact; WP6 repeats bundle and
performance measurements for the final app packaging. Current wasm smoke checks,
all 220 graph tests, nine frontend CXF tests, TypeScript checks and Vite build pass.

## WP5: Rust CXF codec

`cxf` owns the pinned core, its upstream license, and compact JSON-LD document I/O.
Its normal dependencies remain only `cdl-model`, `serde` and `serde_json`.
`CoreLibrary` provides `Library::core()` with the default `bundled-core` feature;
`read_core_library` supports caller-supplied declarations with the feature disabled.
The two paths agree on all 132 pinned elementary interfaces.

The reader handles explicit/automatic root selection, class and instance records,
local declarations, nested composites, constants, bindings, both connection property
spellings and instance-only interface additions. Ambiguous direction returns a
structured diagnostic. The documented `http://example.org#` class-prefix repair is
retained. Standard interface deviations become explicit ACE variants, preserving
both authoritative standard declarations and old editor behavior. Saved exports
with interface-only composite headers remain readable and executable.

The writer references standard elementary classes and emits user/extension class
templates separately from instances. It writes the slash QUDT schema namespace;
both historical forms are accepted on input. Unit IRIs remain intact; compatibility
JSON alone supplies display symbols. Native annotations, typed values, dimensions,
class defaults, instance bindings and their independently editable metadata survive
normalized round trips. Internal legacy adapter fields are reserved and omitted.

Nine upstream fixtures are pinned by path and SHA-256 in the fixture manifest.
Rust tests cover all fixture fixed points, both buildings, malformed inputs,
metadata removal, declarations absent from standard interfaces, typed values,
array expressions, direct boundary wiring and old TypeScript export execution.
The 12 frontend CXF checks include comparison of every TypeScript-recognized
CoolingCoil field with the Rust compatibility projection, plus npm `jsonld`
expansion and structural-reference checks for Rust exports of all nine fixtures
and both buildings. The latter now runs in pull-request CI. API, extension fields,
provenance and reviewed core-update policy are documented in the crate README.

The frontend still uses its TypeScript CXF adapter until WP6. Its temporary enum
parameter hint fix and the compatibility compound-unit display fix change metadata
interpretation rather than kernel arithmetic. Validation passes: the all-feature release workspace tests (including 203 runtime
tests, 81 relocated executor tests and all CXF regressions), all 143 OBC cases
against pyfunnel through both compatibility entry points, 220 frontend graph tests,
12 CXF interoperability tests, TypeScript/Vite build, strict core clippy, dependency
guards and their three tests, native/wasm builds without bundled core, wasm build
and all smoke checks. The three workspace ignores remain the explicit performance
gate and the two previously documented tests; no new ignored test was added.
The intermediate wasm artifact is 3,094,575 bytes (648,201 gzip level 9), SHA-256
`4006df92172af2b310f197bd7b552c03d1911e50ee3f96f8d9088814ac63053d`.
The embedded-core explanation above still applies; WP6 must record the final bundle
and repeat full-year browser parity after app integration.

## WP6 in progress: native simulator and browser boundary

The former `runtime/` sources are now `crates/ace-building-sim` and
`crates/ace-cdl-wasm`. The native constructor accepts a canonical document, weather
records and scenario, and returns `SimError`. Native stepping returns typed chunks,
summary totals and a 144-field `SimulationRow` record (plus optional traces).
Fixed zone/water arrays preserve the existing topology. Arrow reads these records
directly in the wasm crate; the native dependency guard rejects Arrow and wasm
packages. The existing 201 native tests and two relocated transport tests retain
coverage of the former runtime's 203 tests.

The browser keeps its Simulation methods and execution/validation calls. Module
imports, wasm build paths, native runners, verification script and CI paths use
the new crates. The original ignored runtime build cache was preserved outside the
repository at `/tmp/ace-cdl-original-runtime-build-wp6`; the frozen baseline binary
is now `/tmp/ace-cdl-original-runtime-build-wp6/target/release/baseline`.

Validation of this boundary change: 366 all-feature release workspace tests pass,
with the same three explicit ignores; strict core clippy and dependency checks,
all 220 frontend graph tests, 12 CXF tests, TypeScript/Vite build, wasm build/export
check and all wasm smoke checks pass. All 143 OBC cases pass through both legacy
entry points against pyfunnel. A full-year conventional native snapshot matches
the original byte for byte with SHA-256
`c1da451a4718190f06701317e9c1529070f5f5b0713d11d91560088a9202c445`.
Its baseline runner deliberately retains the old JSON serialization/parsing boundary;
native typed rows avoid that conversion. Summary totals also match exactly.

The intermediate wasm is 3,151,404 bytes (652,708 gzip level 9), SHA-256
`1c06ff8f1a149e6fd17da899bb369a1c032f2667aa026d8ba3919de98e7226a5`.
Subsequent WP6 integration replaces per-step control dictionaries with typed point
records. `sim/points.rs` owns the global, source, AHU, zone and pollution mappings;
constructors resolve the 45 conventional or 191 G36 inputs and the 70 consumed
outputs to program-scoped handles. Contract validation derives its required output
names from the same table. Unrecognized physical inputs fail construction. The
legacy graph is retained only in tests for the independent frozen executor.

The wasm boundary now exports CXF import/export, authoritative catalog JSON,
alignment and structured diagnostics. Supported inherited defaults are reified
through the actual kernel builder for the simplified editor projection; arbitrary
parent expressions remain inspectable. Canonical CXF I/O retains declaration
expressions. The frontend calls these exports for CXF and alignment; the old
TypeScript implementations and core-fetch path are removed. The prior CoolingCoil
projection is frozen as a comparison fixture, and Node tests initialize the same
wasm module used by the browser. The application initializes wasm before mounting,
with a loading message and readable failure handling. Graph v1 and migrations are
unchanged.

`cdl_exec::aligned_step` inspects timing without compiling the graph, so incomplete
editor wiring does not prevent a timestep display. It shares event-period rules
and one alignment implementation with `Program::aligned_step`. Positive, negative
and incomplete-wiring cases are covered. Structured validation is rendered as
readable messages by the existing UI.

Latest verification: 371 all-feature release workspace tests pass, with the same
three explicit ignores; strict core/wasm clippy, dependency checks, 220 frontend
graph tests, 14 CXF interoperability tests, TypeScript/Vite build and the wasm
smoke suite pass. Smoke checks now compare the first native simulation interval
before/after CXF conversion for both buildings, as well as catalog coverage,
alignment and structured error payloads. A fresh full-year native G36 snapshot
matches the original byte for byte, SHA-256
`ebd4aa7cd33cc7e0b64562e93d59a08d8867c0416f919bb35aa8573cbfb8abb4`.

A full-year wasm replay against the frozen original package preserves all four
summary totals exactly: baseline 269.897 seconds, candidate 125.152 seconds,
ratio 0.464. The benchmark package was captured before the later timing-inspection
API extraction; physical/control stepping is unchanged. This is a Node wasm check;
final browser UI and saved-project execution gates remain open. The captured
package path is recorded in `/tmp/ace-cdl-wp6-benchmark-pkg-path` and its log in
`/tmp/ace-cdl-wp6-browser-annual.log`.

The current wasm is 3,574,678 bytes (745,464 gzip level 9), SHA-256
`82b68f81dd2fac046f4a68f2748c224a0ef761acfc19bfae5273a593bb0715d1`.
The additional roughly 0.42 MB since the native/wasm split makes the general CXF
reader/writer and full catalog/model serialization reachable through exported
wasm functions. Together with the embedded core and compatibility metadata
explained above, this accounts for growth beyond the 15% review threshold. The
same no-opt wasm build settings are used for comparisons. Record the final artifact
again after generated frontend integration. Legacy host DTO/validation helpers
remain temporarily in the sim crate for existing callers and WP7 migration.

## WP6 complete: generated frontend and browser parity

The frontend catalog and palette now come from the wasm `block_catalog` export.
The catalog retains all 132 pinned standard declarations and adds seven explicit
ACE editor interfaces, covering legacy-only behaviors, weighted Add, Gain and the
legacy Switch layout. Ports derive from authoritative declarations. All 59
executable creation presets are compiled and stepped with declared signal types;
prior editor initial values are retained as explicit bindings in an app-owned
preset profile. Kernel defaults and existing project bindings are unchanged.
The glossary marks import-only declarations; these are excluded from the palette.
Prose and equations remain in an editorial overlay, with class-specific overrides.

The optional Rust `ts` feature generates scenario/event types, simulation rows,
summary totals, diagnostic codes and native column names. Hand-written interfaces
and column lists are removed from the frontend. Its only scenario extension is
the retired project-v1 `heat_pump_fraction` migration field. Native nullability
also exposed and corrected the heat-pump editor's distinction between absent and
null optional COP fields. `generate-types` and `generate-catalog.mjs` have
non-writing check modes; deliberate drift was rejected by both, and the restored
output passes. CI and frontend builds check generated artifacts for staleness.

Real conventional and G36 project exports were downloaded through the UI at
baseline commit `73f562fc99528181c24cea8b24d81c8a18cdb41f`, reproduced with an
isolated git archive and frozen original wasm. Both import and run through the
new browser worker. The large G36 export is gzip-compressed without changing its
bytes. No project-v1 schema or migration file was changed.

The original Chromium G36 annual run completed in 254.237 seconds. The migrated
UI's complete year passed in approximately 2.3 minutes through the actual worker,
Arrow ingestion, plotting and persisted run-history path. All four summary totals
match the original exactly, and the effective step remains 15 seconds. The frozen
reference records original wasm/project hashes in
`web/tests/fixtures/pre-workspace-browser-g36-summary.json`. Browser smoke tests run
on pull requests; the full-year gate is available by workflow dispatch and is
mandatory in the Pages build before its deployment job. No deployment was run.

Final WP6 checks: 372 all-feature release workspace tests, 220 frontend tests,
14 CXF checks, strict core/wasm clippy, dependency guards and guard tests, generation
checks, wasm build/smoke checks, all 143 OBC cases against pyfunnel through both
entry points, both saved-project browser tests and the explicit full-year Chromium
gate pass. The three Rust ignores remain the previously documented explicit gates.
The final native G36 replay also matches the original snapshot byte for byte.

The current wasm is 3,664,698 bytes (767,479 gzip level 9), SHA-256
`0e0beb32500196103c65471c20c21c235abe31e2da24226d35e73eb515c0bd6d`.
The additional roughly 90 KB since the CXF/alignment boundary adds catalog preset
construction/validation and explicit ACE editor interfaces. The larger increase
from WP1 remains explained by the bundled core, typed compatibility model and
reachable CXF/catalog serialization described above. The same no-opt wasm build
settings were used. The frontend JavaScript is 842.76 KB (194.83 KB gzip), including
the generated catalog; supported behavior and initial values no longer have a
separate hand-maintained interface list in TypeScript.

## WP7 verification and CLI implemented; publishing remains

Added `cdl-verify` with a documented, version 2 suite API, simplified/CXF graph
loading, nested instance selection, RFC 4180 CSV input, Real input offsets, native
absolute time/value funnel comparison, per-sample errors and worst value/time
excesses. The implementation-under-test trait supports fresh experiments with
another controller; a CXF fixture compares its observations with the native adapter.
Dymola conversion remains in the checked Python scripts because their model-specific
array selection and event/input reconstruction exceed raw-text parsing.

`cdl-cli` provides `cdl validate`, `convert`, `run`, `verify` and `blocks`. Validation
has structured JSON diagnostics; run uses one compiled state with resolved handles;
verification can write actual CSVs and native comparison reports. Integration tests
exercise JSON/CXF conversion, replay, diagnostics, catalog coverage, failed pin
checks and failing comparison exit codes.

Migrated the checked-in suite to version 2. Graph paths now resolve beside the suite;
reference source hashes moved from Python into the manifest, and every CSV trend has
a pin. Graph contents, trend bytes and tolerance values were preserved. Removed the
temporary simulator `obc_verify` binary. `scripts/verify-obc.py` now drives `cdl verify`
and independently compares all 143 outputs with pyfunnel, requiring per-sample error
agreement within 1e-10 and identical pass/fail. Both frozen original engines remain
in the core regression tests. An additional 100 seeded passing/failing experiments
(2000 samples) agree on pass/fail; maximum error difference is approximately 1e-9,
below the cross-check's 2e-8 geometry tolerance. Both checks run in OBC CI.

Evidence: the workspace run passed 379 tests, zero failures, three pre-existing
ignored benchmarks. The subsequent targeted run also passes the added CXF/trait
test. New crates pass strict Clippy, cargo doc, a check using the actual Rust 1.85
compiler, and `cdl-verify` builds for wasm32. Dependency guards pass for all cores
and the native simulator. WP6's full-year browser/native parity remains verified;
WP7 changes do not alter executor or physical stepping code.

## WP7 release candidate verified; registry authentication outstanding

Final packages are `cdl-model`, `cdl-cxf` (Rust library `cxf`), `cdl-blocks`,
`cdl-compat`, `cdl-exec`, `cdl-verify`, and `cdl-cli`, all version 0.1.0. API checks
returned 404 for these names on 2026-11-14; `cxf` is already occupied by a credential
format crate. This changes the package namespace, not CXF behavior. Keep the cores
here through 0.1.x and reconsider a repository split at 0.2.

Original source uses MIT. Every publication archive includes a LICENSE with the MIT
grant and complete pinned modelica-json/Buildings terms and provenance. The original
upstream texts are also retained under `licenses/` and THIRD_PARTY_NOTICES.md. No
upstream material is relicensed under MIT. See `docs/crate-releases.md`.

Repository-dependent tests/examples moved to unpublished `cdl-integration-tests`,
retaining both frozen original engines, the real host adapter and the same fixture
assertions. Published package tests/examples no longer read outside their archives.
Both app crates and both test/oracle packages are explicitly unpublished.

The final locked seven-package `cargo publish --dry-run` succeeds. Fresh archive
extraction runs 45 tests with zero failures, installs the CLI offline from those
archive sources, and validates/converts the official CoolingCoil fixture. Six library
API comparisons pass 229 semver checks each against their exact archive rustdoc JSON.
Direct rustdoc JSON avoids assuming that first-release dependencies already exist
in the registry. Historical API comparison against reachable `cdl-v*` tags reports
that the first release has no prior baseline; it makes no artificial compatibility
claim. New CI verifies packaging, historical semver, warning-denied docs and Rust
1.85 with all features. The release tag workflow runs the complete native/browser
and package gates, then publishes in dependency order. No workflow was triggered.

Final release-candidate evidence:

- Workspace: **382 passed, zero failed, three existing manual benchmarks ignored**.
- Default-off integration harness passes equation rejection and supported-case
  comparisons; all-feature harness retains the full original-engine corpus.
- Strict lint checks pass for all cores, CLI and wasm wrapper; dependency guard and
  its three regression tests pass; cargo doc passes with warnings denied.
- All 143 outputs agree with independent pyfunnel at unchanged tolerances and per-
  sample 1e-10 agreement. The seeded 100-experiment/2000-sample check also passes.
- Both buildings pass 35040 recorded annual air/water/energy balance intervals.
- Fresh native annual snapshot bytes match the frozen originals: conventional SHA
  `c1da451a4718190f06701317e9c1529070f5f5b0713d11d91560088a9202c445`,
  G36 SHA `ebd4aa7cd33cc7e0b64562e93d59a08d8867c0416f919bb35aa8573cbfb8abb4`.
- Fresh wasm build/table verification and complete smoke suite pass; generated files
  are current; fourteen CXF interoperability tests, 220 graph tests, typecheck and
  Vite build pass.
- **All four final browser tests pass**, including both real original exports,
  generated palette/glossary/defaults and exact full-year G36 summary parity.

The final wasm is 3664988 bytes (767489 gzip-9), SHA-256
`1d42ec83e3d60f03633f71eca450ec81eea4bc46e41dd4b4e5d110797afbdedc`.
The 290-byte increase over the WP6 checkpoint follows package namespace/metadata
changes; the earlier bundled-core/CXF/editor explanation still covers the overall
size increase. Build settings remain unchanged.

No production deployment, commit, release tag or PR has occurred.
## WP7 first publication complete

All seven packages are published at **0.1.0**: cdl-model, cdl-cxf, cdl-blocks,
cdl-compat, cdl-exec, cdl-verify and cdl-cli. Registry uploads completed on
2026-11-15 UTC after the user configured Cargo access and verified the account.
The first five uploads succeeded, then crates.io's new-crate limit required two
scheduled cooldowns. The worker honored both server retry times; cdl-verify and
cdl-cli were subsequently accepted. No rejected upload was treated as successful.

`scripts/verify-published.py` downloaded all seven actual registry archives,
verified their registry SHA-256 checksums and every original source/README/license
file against the tested worktree. A clean `cargo install cdl-cli --version =0.1.0
--locked` outside the repository passed all five CLI commands, including direct
upstream CoolingCoil validation/conversion, catalog coverage, CSV replay and a pinned
version 2 verification case. The complete public receipt is
`docs/crate-release-0.1.0.json`. A separate clean Rust project using only registry
dependencies also passed all 143 pinned suite cases with the published cdl-verify.

The release workflow now resumes partial releases safely: it verifies existing
versions and skips matching uploads, rejects mismatched source, and honors the
registry's cooldown instructions. Seven publication regression tests plus the three
existing dependency-guard tests pass; actionlint and whitespace checks pass. Future
CI tag publication uses the documented repository token secret; this first release
used the user's configured local Cargo access. No credentials are stored in receipts.

The required architecture implementation, parity verification, packaging and first
registry publication are complete. The workspace remains uncommitted for review.
**Production remains undeployed.**


WP8's demand check is complete: `docs/python-bindings-demand.md` inventories actual
neighboring Python consumers and the CLI surfaces they can call. No in-process Rust
consumer is requested, so optional pyo3 bindings are deferred as the plan allows.
No neighboring repositories were changed. The requirement-by-requirement audit is
in `docs/architecture-acceptance.md`; parser alternatives are reviewed separately
in `docs/expression-parser-decision.md`.

## Follow-ups outside this implementation

- Optional WP8 bindings remain deferred after the documented demand check. Reopen
  them for a confirmed in-process consumer.
- Future tag-based publication requires the documented CARGO_REGISTRY_TOKEN GitHub
  secret; the first publication and registry installation acceptance are complete.
- Source review/committing and production deployment are separate follow-up actions.
