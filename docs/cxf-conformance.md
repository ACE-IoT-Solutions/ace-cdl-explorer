# CXF support and specification checks

The editor imports and exports [Control eXchange Format (CXF)](https://obc.lbl.gov/specification/cxf.html) as JSON-LD (`.jsonld`). Import resolves standard block interfaces against the bundled `CXF-Core.jsonld` from [LBNL modelica-json](https://github.com/lbl-srg/modelica-json/blob/f75058d25bafb8c503e894dd193d3fa8037aef23/CXF-Core.jsonld), pinned to commit `f75058d25bafb8c503e894dd193d3fa8037aef23`. The test fixture comes from the same commit. The upstream license is shipped with the site at `schemas/modelica-json-LICENSE.md`. Export writes block instances, connectors, parameters, nested blocks, and `S231:isConnectedTo` wire references. ACE block notes and original block type names use the `ace:` namespace. The project JSON button remains the format for scenario settings and editor layout.

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

The [OBC verification method](https://obc.lbl.gov/specification/verification.html) compares output time series from an implementation and a CDL reference under the same input series and parameters, using value and time tolerances. CI now does this for five supported elementary types and one composite sequence. The expected values are calculated from official Modelica equations and checked in as fixed CSVs. A future expansion should use Modelica-generated traces for stateful blocks, with explicit unit conversion and appropriate event tolerances. The current fixture set does not establish complete CDL conformance.
