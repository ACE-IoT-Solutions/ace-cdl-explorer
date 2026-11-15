# CDL executor

Compile a canonical `Document` once, instantiate independent states, and resolve
typed input/output/trace handles before stepping. Production dependencies are only
`cdl-model` and `cdl-blocks`. The optional `equations` feature adds `meval` and is off
by default. No JavaScript, wasm bindings, Arrow or building simulator is required.

This native PI example uses the format adapter to load standard declarations:

```rust
use cdl_blocks::Registry;
use cdl_exec::{Program, Scalar};
use cdl_model::{Assignment, Document, Instance, Meta, Value};

let library = cxf::core_library();
let mut controller = Instance::new("controller",
    "http://data.ashrae.org/S231#Buildings.Controls.OBC.CDL.Reals.PIDWithReset");
for (name, value) in [("k", 0.1), ("Ti", 60.0)] {
    controller.assignments.push(Assignment {
        name: name.into(), value: Value::Real(value), meta: Meta::default(),
    });
}
let document = Document {library: library.clone(), root: controller};
let program = Program::compile(&document, &Registry::new(library))?;
let setpoint = program.input("u_s")?;
let measurement = program.input("u_m")?;
let trigger = program.input("trigger")?;
let output = program.output("y")?;
let mut state = program.instantiate();
state.set(&setpoint, Scalar::Real(22.0))?;
state.set(&trigger, Scalar::Boolean(false))?;
let mut temperature = 20.0;
for _ in 0..60 {
    state.set(&measurement, Scalar::Real(temperature))?;
    state.step(1.0)?;
    let Scalar::Real(command) = state.get(&output)? else {unreachable!()};
    assert!((0.0..=1.0).contains(&command));
    temperature += command * 0.02;
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

The compiler aliases composite connections to slots, expands children in the model's
stable evaluation order and builds registered kernels. Unsupported classes fail
compilation; there is no fallback engine. Initialization-only constants are removed
from the stepping loop. Delays publish stored values before evaluation and capture
inputs afterward. `Pre` event iteration uses the original 32-attempt limit and restores
checkpointed state on failure. The original Greater/Less checkpoint convention is
preserved deliberately.

`State::set` checks handle ownership, declared signal type and finite numeric inputs.
Handles from clones of the same `Program` work with its independent states; handles
from another compilation are rejected. `trace("root.child.port")` resolves internal
signals. Legacy `composite|child.port`, `|edge.N` and `|output.name` aliases are also
available for the Explorer. Scalar integers use the original executor's internal
f64 representation, so integers beyond f64's exact range retain that existing limit.

`aligned_step` retains the building simulator's whole-second, 15-second minimum and
includes sample periods, trigger phase, delays and holds. A requested step below that
minimum or an event that cannot be aligned fails explicitly.

With `equations`, free-form ACE equation blocks evaluate ordered assignments using
the same `meval` parser and min/max functions as the original interpreter. Known
builtin families retain precedence over documentary equations. Without the feature,
any nonempty equations annotation fails compilation with an explicit diagnostic.
General Modelica parameter expressions remain outside executable support. Keeping
`meval` isolates the dependency and preserves existing numerical behavior; replacing
the parser is deferred until an alternative has equivalent expression coverage.

All 143 verification traces compare every output against both frozen old engines
when equations are enabled. The default-feature test verifies rejection of equation
graphs and compares the remaining cases. API tests cover state independence, handle
ownership, types, event rollback, timing and feature gating. The 82 ported legacy
executor tests use the real host adapter with no simulator/wasm dependency; their
call shape retains the original graph corpus and expectations. The unpublished `cdl-integration-tests` package owns the repository harness,
including the real host adapter and frozen original engines. These files are not
needed by consumers or included in the published core packages.

Full-year G36 comparisons pass natively (candidate/baseline ratio 0.917) and in wasm
(0.956), with exact native snapshot and wasm summary parity. Reproduction scripts
and evidence are recorded in `docs/workspace-migration.md`.

```sh
cargo test -p cdl-exec --release
cargo test -p cdl-exec --all-features --release
cargo test -p cdl-integration-tests --all-features --release --test exec_performance -- --ignored --nocapture
cargo tree -p cdl-exec --edges normal,build
```

MSRV is Rust 1.85. Original project source uses MIT; LICENSE retains complete upstream terms.

`cdl_exec::aligned_step(document, registry, requested)` inspects the same registered
event periods as `Program::aligned_step` without compiling or running kernels. It
supports timestep display while editor wiring is incomplete; executable validation
remains separate. Both APIs use one alignment implementation.

Version 0.1.1 adds an optional `serde` feature with versioned
`StateSnapshot` export/atomic restore across fresh compilations. It identifies
signals and block memory by stable paths, validates aliases/constants/configuration,
and preserves f64 values through JSON using exact decimal strings. The snapshot
includes input initialization and all mutable block memory. `State::track_output(path, target)` independently
initializes PI/PID memory for bumpless handover after current inputs are assigned.
See `docs/controller-state-and-plant-api.md` in the repository for the protocol.
