`CoolingCoil-legacy-project.json` freezes the output of the TypeScript importer
immediately before its WP6 removal, including the WP5 enumeration-hint correction.
It was generated from the adjacent official CoolingCoil fixture and the pinned
`web/public/schemas/CXF-Core.jsonld`. The interoperability test compares every
recognized field with Rust's compatibility projection; extra native annotations
are allowed, and wire/declaration ordering and QUDT prefix spelling are normalized.
The source fixtures and core retain modelica-json's upstream license, included at
`web/public/schemas/modelica-json-LICENSE.md` and `crates/cxf/resources/`.

The two `pre-workspace-*.ace-cdl.json` fixtures were downloaded through the real
Export project action in the UI at commit
`73f562fc99528181c24cea8b24d81c8a18cdb41f`, served from an isolated `git archive`
with the frozen original wasm package. The G36 file is gzip-compressed to avoid a
7.5 MB text duplicate; decompression preserves the exact downloaded bytes.
`pre-workspace-browser-g36-summary.json` records an actual full-year Chromium run
of that original UI, its wasm/project hashes, and all four summary totals.
`tests/browser/capture-baseline.mjs` reproduces capture against that isolated UI;
its version is a historical test fixture and must not be refreshed from the current
implementation. The browser tests import the real exports, exercise the worker and
Arrow path, and compare a completed G36 run with those frozen totals.
