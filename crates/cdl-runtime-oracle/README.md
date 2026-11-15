# Temporary differential oracle

This unpublished test package retains both pre-migration executors and their
validation/alignment helpers from baseline commit
`73f562fc99528181c24cea8b24d81c8a18cdb41f`. Arithmetic and state transitions are frozen.
Only module paths, shared legacy DTO imports and test-observation visibility differ.
The legacy JSON DTOs are owned by `cdl-compat` so hosts and oracles use the same input
containers without depending on the simulator or wasm.

Reusable crates and the runtime reference this package only in dev-dependencies.
Simulator regression tests can select the interpreted oracle explicitly; production
has a required canonical executor and no fallback. Keep this oracle for one release
as required by WP4, then remove it after the replacement architecture has passed all
parity gates. Nothing in this package is intended for publication or production use.
