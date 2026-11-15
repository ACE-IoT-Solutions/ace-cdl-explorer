# cdl-compat

Reads simplified python-cdl/editor JSON into `cdl-model` and writes it back. Runtime
construction uses this adapter before passing the unchanged legacy `Block` shape to
the existing executors. The editor and saved-project format stay in simplified JSON.

```rust
let graph = r#"{"name":"c","block_type":"Constant","outputs":[{"name":"y","type":"Real"}],"parameters":[{"name":"k","value":1.0}]}"#;
let document = cdl_compat::from_json(graph, cdl_compat::ReadOptions::default()).unwrap();
assert!(cdl_model::validate(&document).is_empty());
let restored = cdl_compat::to_json(&document).unwrap();
```

Standard definitions come from the pinned CXF-Core interfaces. Short names are
resolved using connector data types; ambiguous names return `ambiguous_class`.
Standard definitions are shared, while assignments, interface metadata, display
units and original editor type hints remain on their instances. Composite definitions
remain independent unless `deduplicate_composites` is explicitly enabled.

Unknown fields are preserved on their owning instance, connector, declaration or
wire. Namespaced properties are accessible through `Meta`. The adapter retains each
wire's path/expanded spelling and boundary-port spelling. Edits to values, metadata,
units, dimensions and wiring are written from the model; there is no saved whole-graph
snapshot. A field's original spelling is reused only while its canonical value is
unchanged. The seven display-unit mappings live here, outside `cdl-model`.
Integral floating point Integer bindings and large numeric JSON literals retain
their original representation while unchanged; edits are serialized from typed values.

## ACE extensions

Extensions use the ACE CXF namespace and carry their original type on their class
and instance. Different nonstandard interfaces have explicit identifiers rather than
silently claiming the standard class. Deterministic signature suffixes distinguish
interfaces, and collision checks prevent overwriting a different definition.
The [generated interface inventory](EXTENSIONS.md) lists every observed ACE interface
in the bundled and verification graphs, including its parameter and connector names.

| Legacy family | Closest standard class or composition | Why it remains an extension |
|---|---|---|
| `Gain` | `Reals.MultiplyByParameter` | Legacy class name; migration can use the standard class directly. |
| `LimPID` | `Reals.PIDWithReset` | Legacy parameter/interface and controller-state conventions need explicit parity comparison. |
| `Filter` | `Reals` filter composition | No same-named elementary class in the pinned core; behavior equivalence remains a WP3 check. |
| `LimRateLimiter` | Rate-limit/filter composition | No same-named elementary class in the pinned core. |
| `Delay` | `Discrete.UnitDelay`/delay composition | Returns the previous execution-call input; equivalence to periodically sampled delay is not assumed. |
| `Add` with `k1`/`k2` | `Reals.Add` with input gains | The standard Add interface does not declare gain parameters. |
| `Switch` with ACE port order/types | The corresponding typed standard Switch with remapped ports | Selector/input order differs from the standard interface. |
| Other altered typed interfaces or parameter names | The qualified source class when one exists | Each deviation is classified explicitly; WP3 must audit its executable behavior. |
| Free-form equations | ACE equations extension | Preserved annotation, outside standard elementary behavior; execution is feature-gated in WP4. |

The adapter can preserve classes whose behavior is not implemented. Structural
validation does not imply executable support; the runtime/registry remains responsible
for reporting unsupported implementations. Units are metadata, with no numeric unit
conversion. Modelica expressions remain unevaluated. The pinned core contains
incomplete enumeration-member records; enum class identities are retained and bindings
that are expressions remain expressions.

## Checks

```sh
cargo test -p cdl-compat --release -- --nocapture
cargo clippy -p cdl-model -p cdl-compat --all-targets -- -D warnings
python3 scripts/check-core-dependencies.py
```

Corpus tests cover every discovered graph under examples and verification plus all
143 suite selections (192 graphs), exact JSON round trips, and all 7,077 G36 leaves.
The pinned-interface decoder and bundled upstream schema/license are owned by `cxf`.
`pinned_core_library()` remains a compatibility alias for `cxf::core_library()`.
General CXF document import/export is supplied by the `cdl-cxf` package.
Original project source uses MIT; LICENSE retains complete upstream terms.
Repository-wide corpus tests live in the unpublished `cdl-integration-tests` package.

The declared MSRV is Rust 1.85. No crates have been published.
