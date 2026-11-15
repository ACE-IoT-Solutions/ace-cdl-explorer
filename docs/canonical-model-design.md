# Canonical model design (WP2)

Status: WP2 model and adapter implemented; integration and parity checks are recorded
in `docs/workspace-migration.md`. WP3–WP7 remain outstanding.

## Model boundary

`cdl-model` owns class definitions, instances, typed interfaces, values, metadata and
structural diagnostics. It depends on `serde` and `indexmap`; JSON parsers, block
implementations, simulation, wasm and Arrow remain outside it. `serde_json` is a test
only dependency. `cdl-compat` owns the simplified JSON reader/writer and its lossless
representation details. It must translate model edits back to JSON rather than returning
a saved copy of the input graph.

The model uses the class/instance distinction and connector/property vocabulary in
[CXF section 8.2](https://obc.lbl.gov/specification/cxf.html#classes-and-properties).
Instance paths follow [section 8.5](https://obc.lbl.gov/specification/cxf.html#representing-instances-in-cxf).
Arrays and expressions are preserved, following
[section 8.6](https://obc.lbl.gov/specification/cxf.html#handling-arrays-and-expressions).
The pinned local `web/public/schemas/CXF-Core.jsonld` and
`web/tests/fixtures/CoolingCoil.jsonld` are the executable interoperability fixtures.
The model does not claim complete ASHRAE 231 conformance.

## Proposed Rust types

- `ClassId`: an owned, fully expanded IRI, with transparent serde representation.
- `Library`: ordered `ClassId -> BlockClass` definitions, including enumeration classes.
- `Document`: a library and a root `Instance`. A root instance allows its name,
  assignments and annotations to survive independently of its class definition.
- `BlockClass`: ID, `BlockKind`, input/output declarations, parameter/constant
  declarations, optional composite body, metadata and an algebraic-loop-breaking flag.
- `BlockKind`: elementary, composite or extension. Imported abstract/package records
  that are not executable blocks are preserved as library metadata rather than
  masquerading as executable elementary classes.
- `Connector`: local name, direction, `DataType`, units, dimensions and metadata.
- `ParameterDecl`: local name, data type, optional default value or expression,
  units, bounds, dimensions and metadata. Constants are separate declarations.
- `Instance`: local name, referenced class, assignments, metadata and optional
  connector/parameter declaration additions from CXF instance records. These additions preserve instance
  ports absent from the referenced class without silently modifying every instance.
- `CompositeBody`: ordered child instances and connections.
- `Connection`: two connector endpoints and metadata. Endpoints identify the owner
  (`self` or a child instance) and local connector; direction is resolved from context.
- `DataType`: Real, Integer, Boolean, String or an enumeration class reference.
- `Value`: real, integer, Boolean, string, enumeration member, recursive array or
  unevaluated expression. Scalar string values and expressions remain distinct.
- `Dimension`: a positive integer size or preserved expression.
- `Units`: optional QUDT unit, display-unit and quantity-kind IRIs. The model does not
  replace IRIs with display symbols or convert numeric values between units.
- `Meta`: label, description and ordered namespaced annotations. An annotation value
  is a serde-enabled recursive null/Boolean/integer/real/string/list/map type so it
  can preserve JSON-shaped values without depending on `serde_json`.

Connector direction is relative to its owner. A composite's own input is a source
inside its body; a child's input is a sink. A composite's output is a sink inside
its body; a child's output is a source. This rule handles boundary forwarding and
avoids treating every input connector as a sink regardless of context.

## Validation and ordering

Diagnostics have a stable enum code, instance path and human-readable message.
Codes cover unknown classes, recursive class instantiation, duplicate names,
invalid class/body combinations, unknown connectors, wrong endpoint direction,
incompatible connector types/dimensions, multiple drivers, undriven inputs and
algebraic cycles. Malformed classes must yield diagnostics without panicking.
Per-block parameter semantics remain outside the structural validator.

Validation visits definitions once for structure and instances for path-specific
assignment checks. Flattened traversal maintains an active class stack to report
recursive instantiation while allowing the same class in multiple branches.
Stable declaration order breaks ties in topological ordering. A class's explicit
loop-breaking flag removes its input dependencies from the algebraic order; the
model has no `UnitDelay` or `Pre` string special cases. The executor remains
responsible for sampled/event update semantics.

Undriven inputs need a compatibility policy: current JSON graphs can intentionally
leave optional elementary inputs disconnected. The adapter/registry must identify
those optional inputs explicitly; structural checks must not turn previously valid
building graphs into hard failures merely because the canonical interface is richer.

## Compatibility preservation

The default reader creates one composite class per instance. Structural deduplication
is opt-in. Per-instance metadata must remain on the instance when definitions are
shared. The adapter records the original short/full block spelling, connection
spelling and all editor metadata at their actual model location. Mixed edge
spellings in one graph are preserved per connection. Unknown fields are retained as
namespaced compatibility annotations on the corresponding instance, declaration or
connection rather than a document-wide raw JSON snapshot.

Short-name resolution compares connector data types against the pinned core.
Ambiguous candidates produce diagnostics. Missing standard classes and ACE variants
with nonstandard interfaces/behavior use ACE extension IDs. Fully qualified standard
names do not imply that altered connector names or parameter semantics are standard;
those differences require explicit extension classification too.

Equations become an extension annotation. Importing and round-tripping the annotation
does not require enabling execution. The default executor later rejects it unless its
`equations` feature is enabled. Modelica parameter expressions are retained without
attempting evaluation in the model.

Array dimensions are modeled now. Conditional-component expressions and less common
CXF properties are preserved in annotations initially. Import/export must preserve
them; execution of unresolved arrays or conditional components must fail clearly
rather than silently dropping components or claiming support.

## CoolingCoil worked mapping

For the official fixture, the root instance is `CoolingCoil`, referencing the composite
class whose IRI ends in `Buildings.Controls.OBC.ASHRAE.G36.AHUs.SingleZone.VAV.SetPoints.CoolingCoil`.
That class declares the four inputs `TAirSup`, `TSupCooSet`, `u1SupFan`, `uZonSta`,
the output `yCooCoi`, and the four parameters `controllerTypeCooCoi`, `kCooCoi`,
`TdCooCoi`, `TiCooCoi`. Its six child instances reference classes independently:

| Instance | Referenced class suffix |
|---|---|
| `and2` | `Buildings.Controls.OBC.CDL.Logical.And` |
| `conCoi` | `Buildings.Controls.OBC.CDL.Reals.PIDWithReset` |
| `conInt` | `Buildings.Controls.OBC.CDL.Integers.Sources.Constant` |
| `const` | `Buildings.Controls.OBC.CDL.Reals.Sources.Constant` |
| `intEqu` | `Buildings.Controls.OBC.CDL.Integers.Equal` |
| `switch` | `Buildings.Controls.OBC.CDL.Reals.Switch` |

For example, `conCoi.k` is `Value::Expr("kCooCoi")`, whereas
`conCoi.reverseActing` is `Value::Boolean(false)`. A wire linking `and2.u2` to
root `u1SupFan` becomes endpoints `self.u1SupFan` and `and2.u2`; the boundary-direction
rule identifies the source without relying on which node carried the wire property.
The connector's SI and display-unit IRIs remain separate. The documented
`http://example.org#` class-prefix fix-up belongs to the CXF/compatibility readers,
not to `ClassId` itself.

The compilable Rust example is `crates/cdl-integration-tests/examples/cooling_coil.rs`.
`cargo run -p cdl-integration-tests --example cooling_coil --release` resolves all six classes,
constructs all eleven fixture connections and checks structural validity. It is a
worked construction example; general CXF import remains WP5.

## Required implementation evidence

1. Structural negative tests exercise each diagnostic and boundary direction.
2. Shared class traversal terminates and preserves independent instance paths.
3. Every bundled graph round-trips with ports, values, metadata and connection spellings.
4. All 7,077 G36 leaves resolve uniquely with explicit extension classifications.
5. Existing runtime validation diagnostics remain covered when wiring in the adapter.
6. Native and wasm core builds pass, with the dependency guard and the full existing gate.
