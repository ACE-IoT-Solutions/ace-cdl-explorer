# Architecture acceptance audit

This records the 0.1.0 architecture/release checkpoint. Subsequent 0.1.1
controller-state and plant-only APIs are documented in
[controller-state-and-plant-api.md](controller-state-and-plant-api.md).

This maps the requested plans to concrete artifacts and checks. Evidence is for the
verified release workspace. Registry publication
progress is recorded below; no merge or production deployment has occurred. Historical step-by-step measurements and baseline revisions are
in `workspace-migration.md`. Proposed design shapes were refined as documented in
`canonical-model-design.md`, rather than treated as a requirement to reproduce each
illustrative signature verbatim.

| Requirement | Implementation and evidence |
| --- | --- |
| WP1 workspace, shared metadata/dependencies, module split, tests and cache/target migration | Root Cargo workspace; app/frozen-oracle modules under crates; runtime directory removed in WP6. Native app lib.rs is 13 lines, wasm lib.rs 74. Only sim/mod.rs exceeds 2000 Rust lines. All 286 original test annotations were accounted for during WP1; later public-API ports retain the assertions and frozen original engines. |
| WP1 guard rails and unchanged runtime exports/results | Dependency-guard tests cover direct, transitive and build dependencies. Core/native strict lint and formatting gates; historical app warnings remain advisory. Wasm smoke retains Simulation/validate_json/execute_json and checks execution/Arrow transport. Both annual native snapshots are byte-identical to frozen pre-migration outputs. |
| WP2 S231 model, metadata, units, dimensions, values, paths, validation and stable order | cdl-model types/validate/path modules and self-contained structure tests; worked CoolingCoil example in the unpublished integration harness. Caller-supplied break-loop property; no block-name rules in the model. Default normal dependency tree is serde/indexmap; optional ts feature only supports generated frontend contracts. |
| WP2 lossless compatibility adapter, aliases, extensions, equations and independent edits | cdl-compat read/write/default tests and integration compat_roundtrip. Every corpus graph and all 143 suite selections round-trip; all 7077 G36 leaves resolve. ACE interface inventory documents standard mappings, weighted Add, legacy Switch, nonstandard families and preserved equations. Deduplication remains opt-in; literal/field/wire spellings and unknown metadata round-trip. |
| WP2 runtime integration and native/wasm builds | Native simulation consumes Document; wasm/legacy entry points parse through cdl-compat and use model/registry validation. Native and wasm builds pass; migration/graph tests keep project v1 and existing migration modules unchanged. |
| WP3 registry, rules, Step implementations, catalog and coverage | cdl-blocks kernel/config/rules/default/catalog modules; integration blocks_conformance/blocks_suite/blocks_graph_leaves. All 43 legacy families, every elementary suite case and both original engines are covered. The authoritative injected Library avoids a second interface table. Coverage is 52/132 standard classes; 7 explicit editor variants are additive. |
| WP4 flat typed compiler, state, event rollback, traces and alignment | cdl-exec compile/state/instruction/alignment modules and API tests; integration exec_suite/exec_legacy. Inputs resolve once, wires alias slots, Boolean Pre retains its bounded iteration/rollback, delays retain staged capture. Trace aliases and incomplete-editor alignment share registry timing. |
| WP4 optional equations and one production engine | Feature off by default, explicit rejection tested; Explorer opts in. Parser alternatives reviewed in expression-parser-decision.md. Production app wrappers delegate to Program/State; both original engines live only in unpublished/test harnesses. |
| WP4 regressions, native/wasm performance and native example | Original assertion corpus plus all suite outputs compare against the frozen engines. Both annual snapshot hashes match exactly. Recorded native ratio 0.917 and wasm ratio 0.956 pass the 5% limit; later typed handle optimization improves the wasm annual ratio to 0.464. Measurements and reproduction scripts are recorded in workspace-migration.md. The crate README's PI loop is a passing doctest. |
| WP5 pinned core, CXF import/export, contexts, units and annotations | cdl-cxf package in crates/cxf, default bundled-core and external core loader, complete upstream license. Both QUDT namespace forms read, slash form written; explicit old example.org alias. Imports class/instance variants and both wire properties, preserves expressions and namespaced metadata, diagnoses ambiguous direction, and emits explicit root identity. |
| WP5 fixture and processor interoperability | Nine pinned upstream fixtures plus old TypeScript export in the integration harness, fixed-point/negative/metadata tests. Fourteen JS tests pass through real wasm/native conversion, including frozen TS CoolingCoil projection and npm jsonld expansion of both buildings. |
| WP6 native fixed-topology simulator and wasm host split | ace-building-sim constructor takes Document/weather/Scenario and returns SimError; points.rs is the sole control contract, bound once to typed handles. Typed row/chunk/totals, fixed 10/2/1 topology. Arrow, JsValue and wasm-bindgen remain in ace-cdl-wasm. Native baseline and annual-balance binaries moved with the simulator. |
| WP6 generated frontend, saved projects and browser parity | Wasm owns CXF, structured diagnostics, catalog and alignment. Generated TS scenario/rows/diagnostics and catalog have non-writing stale checks, tested with deliberate drift. Editorial help remains text-only. 220 graph/migration tests pass. Real pre-migration conventional/G36 exports load and run through the worker. Four final browser tests pass, including exact full-year G36 summary totals. |
| WP6 bundle accounting and docs | Current wasm: 3664988 bytes, 767489 gzip-9, SHA-256 1d42ec83e3d60f03633f71eca450ec81eea4bc46e41dd4b4e5d110797afbdedc. Growth beyond 15% is explained by the embedded core, typed format models/CXF writer and generated editor metadata. README/runtime/model/CXF docs describe the new layout and gates. |
| WP7 portable verification API, suite version/pins/formats and implementation trait | cdl-verify loads version 2 manifests, checks all pinned sources/CSV references, selects simplified/CXF graphs, replays typed inputs and offsets, reports traces and worst value/time excess. Another-implementation test receives the same CXF experiment. Model-specific Dymola conversion stays in checked scripts with rationale documented in README-suite-v2.md. |
| WP7 native funnel and independent oracle cycle | All 143 cases pass unchanged tolerances; native and pyfunnel agree per sample within 1e-10. 100 seeded passing/failing experiments (2000 samples) agree on pass/fail, maximum excess difference about 1e-9. CI retains the independent Python checks for at least one release cycle. |
| WP7 five CLI commands and old runner retirement | cdl validate/convert/run/verify/blocks; structured diagnostics, CSV outputs, machine-readable catalog/reports and nonzero invalid/failure exits. Tests cover conversion, replay, pin failure, comparison failure and coverage. Temporary obc_verify removed; Python drives cdl verify. |
| WP7 names, licenses, docs, MSRV, packaging and release workflow | Seven final package names checked; cxf collision resolved as cdl-cxf. MIT for original work with complete upstream terms in each LICENSE. Rust 1.85 all-feature check and warning-denied docs pass. All seven locked publish dry runs pass. Extracted archives pass 45 tests and CLI install/conversion; all six library APIs pass 229 semver checks each against their archive JSON. Historical tag check handles the first release explicitly. Release tag workflow gates publishing on native/wasm/browser/interoperability/package checks and publishes in dependency order. Layout retained here through 0.1.x. |
| WP7 first registry publication | Complete. All seven 0.1.0 packages are published. Public registry checksums and original files match the verified source. A clean registry cargo install passes all five CLI commands and upstream CXF validation/conversion. See crate-release-0.1.0.json. No release tag pushed. |
| WP8 conditional demand check | Complete: python-bindings-demand.md inventories the actual neighboring consumers and CLI coverage, identifies the in-process gap, and defers optional bindings until a caller requests that capability. No changes were made to neighboring repositories. |

All workflow files also pass actionlint and YAML parsing; formatting and diff
whitespace checks pass.

The final workspace run passed 382 tests, zero failed, with three existing manual
benchmarks ignored. Both annual buildings pass 35040 recorded balance intervals.
Final original native snapshot SHA-256 values are:

- conventional: c1da451a4718190f06701317e9c1529070f5f5b0713d11d91560088a9202c445
- G36: ebd4aa7cd33cc7e0b64562e93d59a08d8867c0416f919bb35aa8573cbfb8abb4

This audit proves implementation, release-candidate verification and actual first
registry publication/installation. The resumable publication helper also passes
seven regression tests (ten Python guard/publication tests total). It does
not claim reviews, PRs, commits, unverified registry uploads or production deployment that
have not occurred. Historical instructions for staged design PRs/module commits
were not executed; the design and changes are available in the worktree for review.
