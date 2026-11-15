# WP3: Block registry

**Depends on:** WP2. **Size:** L. **Behaviour change:** none intended; parity tests decide.

## Goal

Create `cdl-blocks`: one definition per elementary block that supplies its class, its parameter
rules and its step behaviour. Today that knowledge is spread over five places.

| Knowledge | Today |
|---|---|
| Which blocks exist | `known_type` (lib.rs 97–146) |
| Parameter rules | `validate_block` (lib.rs 162–306) |
| Behaviour, interpreted | `Executor::execute` leaf match (lib.rs 499–1300) |
| Behaviour, compiled | `Op` enum and `compile_leaf` (compiled.rs 30–290, 501–938) |
| Palette text and ports | `web/src/lib/blockCatalog.ts`, `blockHelp.ts` |

## Design

```rust
pub trait BlockImpl: Send + Sync {
    fn class(&self) -> &ClassId;                         // S231 IRI or ace: extension IRI
    fn check(&self, inst: &Instance, out: &mut Vec<Diagnostic>);   // parameter rules
    fn build(&self, inst: &Instance, io: &mut SlotAlloc) -> Result<Box<dyn Step>, Diagnostic>;
    fn breaks_algebraic_loop(&self) -> bool { false }    // UnitDelay, Pre
    fn event_periods(&self, inst: &Instance) -> Vec<f64> { vec![] } // for step alignment
}
pub trait Step { fn step(&mut self, slots: &mut [f64], ctx: &StepCtx); fn is_stateful(&self) -> bool; }
pub fn registry() -> &'static Registry;                  // class IRI → &dyn BlockImpl
```

- Interface data (ports, parameter types, defaults, descriptions) comes from the `cdl-model`
  `Library` loaded from the pinned `CXF-Core.jsonld`, not from a second hand-written list. A block
  definition adds only what the standard file cannot say: checks and behaviour.
- `Step` over `&mut [f64]` matches what `compiled.rs` does now. Whether dispatch is `dyn Step` or a
  generated enum is a performance decision for WP4; the trait is the contract either way.
- State lives inside the `Step` value, which removes the path-keyed `HashMap`s in `Executor`.

## Tasks

- [ ] Design PR with `Add`, `PIDWithReset`, `UnitDelay` and `TrueFalseHold` implemented end to end,
      since they cover stateless, continuous-state, sampled and timed behaviour.
- [ ] Port the remaining blocks family by family, one PR each, in this order: Reals arithmetic and
      comparison; Logical; Integers; Conversions; Discrete (samplers, delays, moving mean); timers
      and holds; PID.
- [ ] For each block, take the behaviour from the compiled `Op` (the production path) and use the
      interpreted branch as the oracle. Any difference between the two found during the port is
      filed as a bug and fixed in its own PR.
- [ ] Move each block's parameter rules out of `validate_block` into `check`.
- [ ] Implement the `ace:` extension blocks from the WP2 table. For each, note in its docs whether
      it can be expressed with standard blocks (`Gain` is `MultiplyByParameter`; the others need
      checking against CXF-Core) so example graphs can migrate off it later.
- [ ] Interface conformance test: for every registered standard class, the ports and parameters the
      implementation reads must exist in `CXF-Core.jsonld` with the same data types. This replaces
      CI check 4 in `docs/cxf-conformance.md`, which today only covers the TS palette.
- [ ] Catalog export: a function that returns the registry as JSON (class, short label, group,
      ports, parameters, description, whether implemented). WP6 uses it to generate the palette.
- [ ] Coverage report: implemented classes out of the 132 in CXF-Core, printed by a test and kept
      in the crate README so users can see what is supported.

## Acceptance

- Every one of the 43 current block names maps to exactly one registry entry.
- Per-block unit tests ported from the inline tests, plus every elementary case in
  `verification/suite.json`, pass against registry `Step` implementations directly.
- The conformance test passes for all standard classes; deviations are listed explicitly as
  `ace:` extensions, not silently tolerated.
- `cdl-blocks` depends only on `cdl-model`.

## Risks

- **The two executors may disagree in corners the parity test does not reach.** Budget time for
  this; it is the main reason the package is sized L.
- **Short-name variants behave differently from the standard class** (gains on `Add`, selector port
  on `Switch`). Changing them to standard behaviour would change simulation results, so they stay
  as extensions here. Migrating the example graphs to standard classes is separate work.
