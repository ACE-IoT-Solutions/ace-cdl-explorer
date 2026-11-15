# CXF support and specification checks

The editor imports and exports [Control eXchange Format (CXF)](https://obc.lbl.gov/specification/cxf.html) as JSON-LD (`.jsonld`). Import resolves standard block interfaces against the bundled `CXF-Core.jsonld` from [LBNL modelica-json](https://github.com/lbl-srg/modelica-json/blob/f75058d25bafb8c503e894dd193d3fa8037aef23/CXF-Core.jsonld), pinned to commit `f75058d25bafb8c503e894dd193d3fa8037aef23`. Nine Rust fixtures from the same commit cover real, integer, logical, conversion, discrete, stateful and array interfaces plus the CoolingCoil composite. Their paths and hashes are recorded in `crates/cdl-integration-tests/tests/cxf/fixtures/manifest.json`. The upstream license is shipped with the site at `schemas/modelica-json-LICENSE.md`. Export writes block instances, connectors, parameters, nested blocks, and `S231:isConnectedTo` wire references. ACE block notes and original block type names use the `ace:` namespace. The project JSON button remains the format for scenario settings and editor layout.

## What CI verifies

The pull-request verification workflows and Pages build run these checks:

1. Import an [official modelica-json composite CXF reference](https://github.com/lbl-srg/modelica-json/blob/master/test/reference/cxf/Buildings/Controls/OBC/ASHRAE/G36/AHUs/SingleZone/VAV/SetPoints/CoolingCoil.jsonld) and check its typed ports, wiring, and SI/display unit metadata.
2. Roundtrip a composite graph and the bundled building graph through CXF, checking key connections, ports, parameters, notes, and custom types.
3. Expand the building export with a JSON-LD processor and verify that every emitted block, connector, parameter, and wire reference resolves within the file.
4. Compare editor palette blocks that claim an official CDL class with the input, output, and parameter interfaces in `CXF-Core.jsonld`.
5. Run the Rust runtime tests, including graph validation, elementary behavior, compiled/dynamic parity, and building energy flow checks.
6. Run the [OBC verification suite](../verification/README.md): replay fixed CDL input trends through the Rust runtime and compare the outputs with independently calculated reference traces using LBNL's `pyfunnel` and declared tolerances.

These checks test the supported subset. They do **not** certify full ASHRAE Standard 231 conformance. The bundled building uses several ACE shorthand/custom block types; these export as locally defined CXF classes, not official standard elementary blocks. Imported standard blocks that the Rust runtime does not implement remain inspectable but fail runtime validation. CXF parameter expressions and arrays are preserved as values/names where possible; the editor does not yet evaluate arbitrary Modelica expressions or flatten arrays. Exported ACE Celsius-valued points retain Celsius as their declared unit; the standard CDL SI/display-unit split is preserved for points imported from CXF.

## Verification scope

The [OBC verification method](https://obc.lbl.gov/specification/verification.html) compares output time series from an implementation and a CDL reference under the same input series and parameters, using value and time tolerances. CI now does this for supported elementary types and two composite graphs, including the nested G36 recovery learner. The expected values are calculated from official Modelica equations and checked in as fixed CSVs. A future expansion should use Modelica-generated traces for stateful blocks, with explicit unit conversion and appropriate event tolerances. The current fixture set does not establish complete CDL conformance.

## Rust format migration (WP5)

The `cxf` crate now imports and exports canonical documents. The frontend now calls its wasm exports; the TypeScript format implementation has
been removed. Application startup initializes wasm before mounting the editor. Pull-request CI compares
Rust CoolingCoil import through the compatibility JSON projection with every field
recognized by the frozen TypeScript importer projection. It expands Rust exports of all nine upstream
fixtures and both building graphs using npm `jsonld`, and checks local structural
references. Rust tests cover normalized fixed points, metadata edits, malformed
input and execution of saved files produced by the old exporter.

Rust output references standard elementary classes rather than redefining them,
separates user class templates from instances, and writes the QUDT schema namespace
with a trailing slash. Both historical QUDT forms remain readable. Unknown namespace
annotations and unit IRIs are preserved; `cdl-compat` supplies display symbols.
Ambiguous wire direction is diagnosed rather than guessed. Enumeration parameter hints come from the canonical declarations; the compatibility
unit display helper also preserves compound symbols such as `1/K`. Neither change alters kernel
arithmetic. See [the crate README](../crates/cxf/README.md) for API, scope, ACE
preservation fields and the reviewed core-update policy.

The frontend catalog and palette now come from the wasm registry catalog at build
time. CI checks generated output for drift and compiles/steps every available
preset. The glossary distinguishes import-only declarations from executable
blocks. Browser compatibility tests load original UI project exports and compare a
completed G36 year with the original browser's four summary totals. The project-v1
shape and migration files are unchanged.
