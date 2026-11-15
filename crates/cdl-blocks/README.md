# CDL block registry

`cdl-blocks` owns elementary parameter checks and numeric-slot behavior. Its only
production dependency is `cdl-model`; JSON, wasm, Arrow and simulation stay in hosts.
The registry implements **52 of 132** pinned CXF-Core classes and all **43 legacy
behavior families**. Unsupported interfaces remain visible in the catalog with
`ace:implemented: false` and fail compilation explicitly.

Hosts supply the authoritative `Library` to `Registry::new`. Currently
`cxf::core_library()` loads it; `cdl_compat::pinned_core_library()` remains a convenience
alias. This injected-library API replaces the proposed global registry so block
definitions cannot silently embed a second copy of interface declarations.
`Registry::catalog()` returns a serializable library with labels, descriptions,
`ace:group`, typed ports/parameters and implementation availability.
`cdl_compat::catalog_json(&registry)` provides JSON encoding at the format boundary.

```rust
use cdl_blocks::{ConfiguredInstance, Registry, SlotAlloc, Step, StepCtx};
use cdl_model::{Assignment, Instance, Meta, Value};

let library = cxf::core_library();
let id = cdl_model::ClassId::from(
    "http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.Sources.Constant");
let mut instance = Instance::new("setpoint", id.clone());
instance.assignments.push(Assignment {
    name: "k".into(), value: Value::Real(22.5), meta: Meta::default(),
});
let config = ConfiguredInstance::new(&instance, &library.classes[&id], "setpoint");
let registry = Registry::new(library.clone());
let mut io = SlotAlloc::default();
io.bind_output("y", 0);
let mut kernel = registry.build(&config, &io)?;
let mut slots = [0.0];
kernel.initialize(&mut slots);
kernel.step(&mut slots, &StepCtx::new(1.0)?)?;
assert_eq!(slots[0], 22.5);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Step` separates initialization, stored-output publication, ordered evaluation and
final input capture. Hosts perform `Pre` fixed-point event iteration, restoring a
cloned checkpoint before each attempt. `Kernel` owns its state and exposes Boolean
event-state access without path-keyed maps. Durations must be finite and nonnegative;
zero is used for initialization. WP4 supplies composite execution and event alignment.

Standard native instances use declared defaults and require bindings where the core
declares none. The compatibility reader records actual legacy builder fallbacks as
typed assignments, including dependent hold/reset defaults. Unchanged implicit values
are omitted when writing legacy JSON; edits become ordinary visible bindings. Invalid
legacy configurations never acquire defaults that would conceal an error.

Legacy deviations have explicit ACE class identifiers; see the
[interface inventory](../cdl-compat/EXTENSIONS.md). `Gain` maps to
`Reals.MultiplyByParameter`; weighted `Add` can use two gains and a standard Add;
legacy `Switch` needs selector-port remapping. `LimPID` has distinct PI state/reset
conventions, `Filter` uses an execution-step dependent first-order update,
`LimRateLimiter` limits change per execution interval, and `Delay` holds the previous
execution-call input. Those behaviors remain explicit extensions; equivalence to a
standard composition has not been established. No examples are migrated silently.

Direct differential tests compare all 43 families over input/state changes, every
one of the 7,077 G36 leaves, and the 20 elementary verification CSV cases against
both old engines. Conformance checks bind each registered standard interface from
the pinned core and verify the primitive types of parameter accesses. No replacement
composite executor is selected by production yet.

```sh
cargo test -p cdl-blocks -p cdl-compat --release
cargo test -p cdl-integration-tests --test blocks_conformance --release -- --nocapture
cargo clippy -p cdl-blocks -p cdl-model -p cdl-compat --all-targets --no-deps -- -D warnings
python3 scripts/check-core-dependencies.py
```

MSRV is Rust 1.85. Original project source uses MIT; LICENSE retains complete upstream terms.

`legacy_catalog_classes` explicitly supplies ACE editor interfaces for behaviors
missing from the pinned core and for weighted Add, Gain and the legacy Switch
layout. Ports derive from core declarations, and hosts bind editable initial
values explicitly. This helper does not inject classes into a registry implicitly;
its catalog presets are validated against the same builders as execution.

Version 0.1.1 exposes versioned `BlockSnapshot`/`BlockMemory` through
`Kernel::snapshot` and `restore`; enable the optional `serde` feature for codecs.
These exclude compiler slots and coefficients, and include comparator memory that
is intentionally outside the old event-rollback set. `Kernel::track_output` supports
explicit PI/PID initialization to a desired output. These additions are included in 0.1.1.
