# cdl-model

Typed CDL classes, instances, connectors, parameters, constants, units, arrays and
preserved expressions. The vocabulary follows CXF sections 8.2, 8.5 and 8.6. The
crate has no JSON parser, wasm bindings, Arrow dependency or simulator.

`Library` owns reusable class definitions. `Document` identifies a configured root
instance. Instance connectors and parameter declarations can preserve instance-only
interface additions and metadata without redefining a standard class. Units retain
QUDT IRIs and a separate display unit. Namespaced metadata uses a recursive serde
value type; it does not depend on `serde_json`.

```rust
use cdl_model::*;

let mut class = BlockClass::new("urn:example:constant", BlockKind::Extension);
class.outputs.push(Connector::new("y", Direction::Output, DataType::Real));
let mut library = Library::default();
library.classes.insert(class.id.clone(), class);
let document = Document {
    library,
    root: Instance::new("constant", "urn:example:constant"),
};
assert!(validate(&document).is_empty());
assert_eq!(document.instances().unwrap()[0].path.to_string(), "constant");
```

`validate` returns path/code/message diagnostics. `evaluation_order` performs stable
ordering and uses `breaks_algebraic_loop` supplied by a class provider. It contains
no special cases for block names. Expressions are preserved without evaluation;
block-specific parameter rules and execution belong to the registry and executor.

The complete pinned CoolingCoil worked model can be built and checked natively:

```sh
cargo run -p cdl-integration-tests --example cooling_coil --release
cargo test -p cdl-model --release
```

The example resolves six children, all eleven connections, parameter expressions,
typed inputs/outputs and units from the pinned fixture and core. It demonstrates
model construction and is not a general-purpose CXF reader.

The declared MSRV is Rust 1.85. Original project source uses MIT; LICENSE retains complete upstream terms.
