# CDL command line tools

The `cdl` binary validates, converts, runs and verifies native CDL documents without
Python, a browser or a building simulator. Rust 1.85 is checked with the locked
dependencies. Publication archives are tested in isolation, including installation
and conversion of the upstream CoolingCoil fixture. Registry publication is pending.

```sh
cargo run -p cdl-cli --bin cdl -- validate crates/cdl-integration-tests/tests/cxf/fixtures/CoolingCoil.jsonld --format json
cargo run -p cdl-cli --bin cdl -- convert web/public/examples/p_controller_limiter.json /tmp/controller.jsonld
cargo run -p cdl-cli --bin cdl -- blocks
cargo run --release -p cdl-cli --bin cdl -- verify verification/suite.json --output-dir /tmp/cdl-traces
```

`convert` infers the output format from `.json` or `.jsonld`; `--to json|cxf` overrides
that choice. `validate` checks the model's structure, so a valid imported class can
still lack an executable implementation. `blocks` lists that availability; its JSON
format includes declarations and coverage annotations from the registry.

`run graph.jsonld --inputs input.csv --dt 1 --trace y --trace root.child.y` writes
CSV to stdout. Omitting `--trace` selects every root output. It resolves handles
once and runs one shared controller state. The first row initializes with dt=0;
subsequent intervals must match `--dt`. See the suite format for CSV input rules.

`verify` uses version 2 suites with SHA-256-pinned references. `--output-dir` writes
actual CSVs and `native-summary.json` including the native per-sample funnel errors.
`--format json` provides the same reports on stdout. `validate --format json` emits
structured model diagnostics. Exit codes are 0 for success, 1 for invalid models or
failed comparisons, and 2 for malformed commands, I/O or execution failures.

After registry publication, install with `cargo install cdl-cli --locked`. Original
project code uses MIT; LICENSE includes complete upstream terms.
