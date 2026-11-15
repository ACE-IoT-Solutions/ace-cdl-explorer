# Shared CDL crates

The core crates are introduced here by the modularization work packages in `plans/`.
They must remain independent of wasm bindings, Arrow, and the building simulator.
`scripts/check-core-dependencies.py` enforces that boundary in CI.

Published package names are cdl-model, cdl-cxf (directory cxf, Rust library cxf),
cdl-blocks, cdl-compat, cdl-exec, cdl-verify and cdl-cli. The native simulator,
wasm host, frozen oracle and repository integration harness remain unpublished.
See [release checks](../docs/crate-releases.md) and the
[acceptance audit](../docs/architecture-acceptance.md).
