# Browser boundary

`ace-cdl-wasm` exposes the Explorer's existing `Simulation`, `validate_json` and
`execute_json` JavaScript surface through wasm-bindgen. It parses browser JSON into
native values, calls `ace-building-sim`, and encodes typed simulation rows as JSON
or Arrow IPC. Plant physics and control execution live in other crates.

Build with `sh scripts/build-wasm.sh`. The generated module is
`web/src/wasm/ace_cdl_wasm.js`; `node scripts/smoke-wasm.mjs` checks the built module.
The two Arrow transport regressions live here rather than in the native simulator.

The module also exports `cxf_import`, `cxf_export`, `block_catalog` and
`aligned_step`. Validation returns structured diagnostics with `path`, `code` and
`message`. The frontend's CXF and alignment wrappers call these functions; main
thread initialization completes before the editor mounts. Node tests initialize
the same wasm module explicitly.

The editor projection reifies supported inherited defaults through the kernel
builder, retaining unsupported expressions for inspection. Native CXF I/O keeps
the declaration expressions. Alignment inspects registry event periods without
compiling the whole graph, including while editor wiring is incomplete.

The catalog contains all pinned declarations with availability flags, explicit ACE
editor interfaces, and validated creation presets. Initial editor bindings retain
the previous palette's values; kernel fallback semantics remain unchanged. Every
available preset is compiled and stepped in tests. `scripts/generate-catalog.mjs`
creates the palette and glossary from this API, with prose/equations in a separate
editorial overlay. Generated source is checked in CI.

Real pre-migration project exports load and run in Chromium, and the full-year G36
browser gate preserves all four original summary totals exactly. This crate is not published or deployed by local build
commands.
