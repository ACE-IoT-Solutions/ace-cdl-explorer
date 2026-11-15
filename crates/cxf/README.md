# CXF JSON-LD

The `cdl-cxf` package (Rust library name `cxf`) reads and writes the compacted Control eXchange Format emitted by
modelica-json, using `cdl-model` as its document representation. Its production
dependencies are only `cdl-model`, `serde` and `serde_json`.

```rust
let library = cdl_model::Library::default();
let source = r#"{"@graph":[{"@id":"urn:example","@type":"S231:ExtensionBlock"}]}"#;
let document = cxf::from_json(source, &library).unwrap();
let json = cxf::to_json(&document).unwrap();
let restored = cxf::from_json(&json, &library).unwrap();
assert_eq!(document.root.name, restored.root.name);
```

`from_value` accepts parsed JSON. `from_value_with_root` selects a root explicitly
when multiple candidates exist; exported files carry `ace:rootBlock`. Errors are
canonical model diagnostics. Import preserves unsupported classes and expressions
for inspection; call `cdl_model::validate` and the executor compiler separately for
structural and executable validation.

The default `bundled-core` feature embeds `resources/CXF-Core.jsonld` from
modelica-json commit `f75058d25bafb8c503e894dd193d3fa8037aef23`. Import the
`CoreLibrary` extension trait to call `Library::core()`, or use
`cxf::core_library()`. With that feature disabled, `read_core_library(json)` loads
caller-supplied elementary declarations. Incomplete upstream enumeration records
retain their identities without invented members. The upstream license is in
`resources/modelica-json-LICENSE.md`; complete terms are also included in LICENSE.
Original project source uses MIT.

## Supported representation

Import resolves class definitions, typed instances, local connector and parameter
records, nested composites, constants, bindings and both `isConnectedTo` and older
`connectedTo` wiring. Instance-only declarations are retained without modifying
standard interfaces. Ambiguous wire direction returns a diagnostic. Old ACE
exports with interface-only composite headers remain readable. A standard instance
whose interface differs from the pinned definition receives an explicit ACE class
variant carrying its original `ace:blockType` and `ace:baseClass`.

Export references standard elementary classes and emits user and extension class
definitions separately from their instances. Structural references resolve locally;
standard class/datatype references and annotation IRIs may be external. Export is
normalized rather than a byte-for-byte source reconstruction.

Inline namespace prefix contexts are supported. Remote contexts, scoped contexts,
term aliases and other general JSON-LD processing are rejected. As an explicit
compatibility leniency, class identifiers beginning `http://example.org#` are mapped
to `http://data.ashrae.org/S231#`. Both QUDT schema namespace forms, ending `/` and
`#`, are read; output uses `http://qudt.org/schema/qudt/`. Exported files therefore
differ from the old TypeScript writer. Unit IRIs retain their identity; display
symbol conversion belongs to `cdl-compat`.

Typed literals, strings, expressions, enumerations, array values and symbolic
extents remain distinct. No Modelica expression evaluation, RDF/Turtle or SHACL
validation is provided. Array rank above 1024 is rejected explicitly. Unknown
namespaced annotations, including ACE notes, Brick classes, point labels, block
types and units, round-trip through `Meta`. The `ace:compat:*` namespace is reserved
for the internal legacy adapter and rejected on input.

ACE preservation fields cover distinctions absent from this compact subset:
`instanceName`, `required`, `connectorOverride`, `declarationDefault`,
`clearsDefault`, `interfaceOnly`, `baseClass` and `enumerationMembers`.
`Expression` and `EnumerationMember` literal types retain value kinds;
`EnumerationInput`/`EnumerationOutput` retain enum connector types.
Annotated wires use `Connection` records with `connections`, `from` and `to`.
These records accompany ordinary S231 wiring and do not change its meaning.

## Verification and core updates

The fixtures include nine pinned upstream files, both building graphs, and an old
TypeScript export. Rust tests check normalized fixed points, metadata edits,
malformed input and executable backward compatibility. `npm --prefix web run
test:cxf` compares CoolingCoil with a frozen TypeScript importer projection and expands Rust output
with npm `jsonld`, checking structural references. These checks run on pull requests.

A core update is a reviewed repository change: update the pinned resource, revision,
upstream license and fixture manifest together; run registry interface/default and
behavior conformance tests plus CXF round trips and JSON-LD expansion. New core
classes acquire executable support only after those tests cover their behavior.
Keep any served downloadable core resource identical to the pinned crate resource.

```sh
cargo test -p cdl-cxf --release
cargo check -p cdl-cxf --no-default-features --lib
cargo clippy -p cdl-cxf --all-targets --no-deps -- -D warnings
sh scripts/build-wasm.sh
npm --prefix web run test:cxf
```

The MSRV is Rust 1.85, checked with all features and locked dependencies.
Repository-wide fixture, compatibility and execution checks live in the unpublished
`cdl-integration-tests` package; they remain part of workspace CI.
