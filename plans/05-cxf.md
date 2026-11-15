# WP5: CXF crate

**Depends on:** WP2. Can run in parallel with WP3 and WP4. **Size:** M.

## Goal

Create `cxf`: CXF JSON-LD import and export for `cdl-model` in Rust, so Rust, wasm and (later)
Python callers share one implementation. Today this exists only as `web/src/lib/cxf.ts` (244 lines).

Because the model is now S231-shaped, most of what `cxf.ts` does to bridge two shapes disappears.
The crate is mainly a careful JSON-LD reader and writer.

## Scope

In: reading and writing the compacted JSON-LD form that `modelica-json` emits; the pinned
`CXF-Core.jsonld` as a bundled `Library`; unit handling; `ace:` annotations.
Out: a general JSON-LD processor (expansion, remote contexts), RDF/Turtle, evaluating Modelica
expressions, SHACL validation.

## Tasks

- [ ] `Library::core()`: parse the pinned `CXF-Core.jsonld` (commit `f75058d`) into `cdl-model`
      classes. Embed it with `include_str!` behind a default `bundled-core` feature so users can
      supply a newer file. Keep the upstream license file with the crate.
- [ ] Import, ported from `importCxf` (cxf.ts 48–152):
  - root selection, class and instance nodes, `containsBlock`, `hasInput` / `hasOutput` /
    `hasParameter` / `hasInstance`;
  - instance records that carry ports missing from the class;
  - connection direction derived from `isConnectedTo` (and the older `connectedTo`);
  - the `http://example.org#` → `http://data.ashrae.org/S231#` class-prefix fix-up, kept as an
    explicit, documented leniency.
- [ ] Export, ported from `exportCxf` (cxf.ts 154–243). Standard classes are referenced, not
      redefined. Extension and user composite classes are emitted as class definitions.
- [ ] Units: keep QUDT IRIs as they are. Drop the seven-entry symbol table from `cxf.ts`
      (`DEG_C` → `°C` …) into `cdl-compat`, which is the only place display symbols are needed.
- [ ] Handle both `qudt` context IRIs seen in the wild: CXF-Core uses
      `http://qudt.org/schema/qudt/`, while `cxf.ts` writes `http://qudt.org/schema/qudt#`. Read
      both; write the CXF-Core form. Note the change in the PR because exported files will differ.
- [ ] `ace:` annotations (`notes`, `brickClass`, `pointLabel`, `blockType`, `unit`) read into and
      written from `Meta`. Unknown namespaced properties are preserved on round trip.
- [ ] Tests ported from `web/tests/cxf.test.ts`, using the same fixtures:
  - import the official `CoolingCoil.jsonld` and check ports, wiring, SI and display units;
  - round-trip a composite and the bundled building;
  - every emitted `@id` reference resolves within the file.
- [ ] Add more upstream fixtures from `modelica-json/test/reference/cxf` at the pinned commit,
      at least one per block family, to test import beyond a single file.
- [ ] Cross-check against a real JSON-LD processor in CI: keep the existing `jsonld` (npm) expansion
      test, now fed with files written by the Rust exporter.

## Acceptance

- Rust import of `CoolingCoil.jsonld` yields the same graph as the TS importer (compare through
  `cdl-compat` JSON).
- Export of `four_pipe_g36` expands cleanly with the `jsonld` npm library and every reference resolves.
- Import → export → import is a fixed point for all fixtures.
- Depends only on `cdl-model`, `serde`, `serde_json`.

## Open questions

- **CXF-Core version policy.** The file is pinned to one `modelica-json` commit. Decide who bumps it
  and how block coverage is re-checked when it changes (the WP3 conformance test will flag drift).
- **Where direction comes from for unusual wiring.** `cxf.ts` falls back to "left is the source" when
  both ends look like drivers. Decide whether the Rust importer should report that as a diagnostic.
