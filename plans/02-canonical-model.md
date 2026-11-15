# WP2: Canonical model

**Depends on:** WP1. **Size:** L. **Behaviour change:** none visible to the app.

## Goal

Create `cdl-model`, an in-memory model that follows ASHRAE 231 / CXF, and `cdl-compat`, which reads
and writes today's simplified `python-cdl` JSON. Interop is the reason for this package: a graph
held in `cdl-model` must map onto CXF without guessing.

## Why the current model is not enough

`Block` (lib.rs 74–95) is an instance tree. It differs from the standard in ways that lose or
blur information:

| Today | Standard (CXF / S231) |
|---|---|
| `block_type` is a free string; half the G36 leaves use short names such as `Add` | Every instance references a class IRI, e.g. `S231:Buildings.Controls.OBC.CDL.Reals.Add` |
| Short names are ambiguous across Reals / Integers / Logical | The class fixes the data types |
| A composite's body is inlined at every use (ten copies of the VAV controller) | `CompositeBlock` class defined once, instantiated many times |
| Elementary, composite and custom blocks are told apart by `category` and by having children | `ElementaryBlock`, `CompositeBlock`, `ExtensionBlock` |
| Connector `type` and `unit` are strings | `RealInput`, `BooleanOutput`, …; `qudt:hasUnit`, `S231:hasDisplayUnit`, quantity kind |
| Parameter is `name` + JSON value | Data type, value or expression, unit, `min`/`max`, array dimensions, enumeration types |
| Connections are directed `from`/`to` | `isConnectedTo` between connector instances; direction is derived |
| `equations` on any leaf | Not in the standard |

## Design

Proposed shape (names and details to be settled in the design PR):

```rust
pub struct Library { pub classes: IndexMap<ClassId, BlockClass> }   // CXF-Core plus user classes
pub struct Document { pub library: Library, pub root: ClassId }

pub struct BlockClass {
    pub id: ClassId,                 // IRI
    pub kind: BlockKind,             // Elementary | Composite | Extension
    pub inputs: Vec<Connector>, pub outputs: Vec<Connector>,
    pub parameters: Vec<ParameterDecl>, pub constants: Vec<ParameterDecl>,
    pub body: Option<CompositeBody>, // only for Composite
    pub meta: Meta,                  // label, description, extension annotations
}
pub struct CompositeBody { pub instances: Vec<Instance>, pub connections: Vec<Connection> }
pub struct Instance { pub name: String, pub class: ClassId, pub assignments: Vec<Assignment>, pub meta: Meta }
pub enum DataType { Real, Integer, Boolean, String, Enumeration(ClassId) }
pub enum Value { Real(f64), Integer(i64), Boolean(bool), String(String), Enum(String),
                 Array(Vec<Value>), Expr(String) /* preserved, never evaluated here */ }
```

- `Meta` holds an open, namespaced annotation map so `ace:notes`, `ace:brickClass`,
  `ace:pointLabel` and editor layout survive a round trip without `cdl-model` knowing about them.
- Units are stored as QUDT IRIs plus an optional display unit, matching what `cxf.ts` already
  preserves in `cxf_unit` / `cxf_display_unit`.
- Validation returns `Vec<Diagnostic { path, code, message }>` with a stable `code` enum, not strings.
- Dependencies: `serde`, `indexmap`. `serde_json` only in tests.

## Tasks

### `cdl-model`
- [ ] Design PR: types above, doc comments citing the CXF spec section for each, and a worked
      example (the `CoolingCoil.jsonld` fixture expressed in the model). Review before coding further.
- [ ] Structural validation ported from `validate_block`'s wiring half (lib.rs 317–364): unknown
      class, missing endpoint, data-type mismatch, multiple drivers, undriven input, cycles.
      Per-block parameter rules stay behind for WP3.
- [ ] Evaluation order ported from `order` (367–393). The rule that `UnitDelay` and `Pre` break
      cycles becomes a class property (`breaks_algebraic_loop`) supplied by the caller, so the model
      has no hard-coded block names.
- [ ] Instance-path helpers (`a.b.c` paths, iteration over a flattened instance tree) because every
      consumer needs them.

### `cdl-compat`
- [ ] Reader: simplified JSON → `Document`. Each inline composite becomes a `CompositeBlock` class.
      Structurally identical composites are deduplicated only when a flag asks for it; the default
      keeps one class per instance so nothing the editor relies on changes.
- [ ] Name resolution: short `block_type` → class IRI. Resolve the twelve ambiguous names from the
      connector data types on the instance. Emit a diagnostic when resolution is not unique.
- [ ] The five names with no standard class (`LimPID`, `Filter`, `LimRateLimiter`, `Gain`, `Delay`)
      and the ACE variants whose interface differs from the standard (for example `Add` with
      `k1`/`k2` gains, `Switch` port order) become `ExtensionBlock` classes in an `ace:` namespace.
      Record each one in a table in the crate docs with the closest standard class, as input to WP3.
- [ ] Leaves with `equations` become `ExtensionBlock` classes carrying the equations as an annotation.
- [ ] Writer: `Document` → simplified JSON, so the editor, the 101 project migrations and saved
      projects keep working unchanged.
- [ ] Both connection spellings (`from`/`to` and `from_block`/`from_output`/…) are accepted on read;
      the writer emits what the reader saw.

### Wiring it in
- [ ] `runtime` depends on both crates. `validate_json` and `Simulation::new` parse through
      `cdl-compat`, validate through `cdl-model`, then convert back to the old `Block` for the
      executors (which are replaced in WP4). This proves the adapter on real graphs early.
- [ ] CI builds both crates for native and `wasm32-unknown-unknown`.

## Acceptance

- Every file in `web/public/examples` and every graph in `verification/` round-trips
  JSON → `Document` → JSON with semantic equality (same blocks, ports, parameters, connections).
- All 7,077 leaves of `four_pipe_g36.json` resolve to exactly one class, standard or `ace:`.
- The diagnostics from the new validator cover every message the old one produced for the
  existing negative tests.
- `cargo tree -p cdl-model` shows no `serde_json`, `wasm-bindgen` or `arrow-*`.

## Open questions

- **Per-instance edits.** The editor lets users edit inside one VAV controller without touching the
  other nine. With shared classes that needs a "make unique" step. Until WP6 decides, the adapter
  keeps one class per instance.
- **Expressions.** CXF parameter values can be Modelica expressions. This package preserves them as
  `Value::Expr`. Evaluating them is out of scope and should be raised separately.
- **Arrays and conditional components.** They are in CXF-Core (`isArray`, `isConditionalComponent`).
  Decide in the design PR whether to model them now or carry them as preserved annotations.
