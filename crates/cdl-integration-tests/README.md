# Repository regression harness (unpublished)

This package owns tests/examples that need the Explorer corpus, frozen original
engines or the real host adapter. Published crates do not depend on it. Moving the
harness here preserves the same assertions and fixtures while keeping every
publication archive self-contained.

```sh
cargo test -p cdl-integration-tests --all-features --release
cargo test -p cdl-integration-tests --no-default-features --release
cargo run -p cdl-integration-tests --example cooling_coil --release
cargo run -p cdl-integration-tests --example extension_report --release
```

The default features enable equations and verification. Turning both off runs the
executor suite's existing equation-rejection checks; the other supported cases still
compare with both original engines. The verification feature enables `cdl-verify`,
which deliberately enables equations for the full suite. The legacy executor corpus
includes the actual `ace-building-sim` adapter source, without linking the simulator.
Cross-language CXF tests invoke the `convert` example. Pinned upstream CXF fixtures
and old TypeScript export fixtures live under `tests/cxf/`; their original provenance
and hash manifests remain intact.
