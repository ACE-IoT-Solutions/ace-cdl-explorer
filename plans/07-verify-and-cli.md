# WP7: Verification harness and CLI

**Depends on:** WP4, WP5. **Size:** M.

## Goal

Package the OBC time-series verification as `cdl-verify`, give outside users a `cdl-cli` binary, and
publish the core crates for the first time.

## Current state

- `runtime/src/bin/obc_verify.rs` (187 lines) reads `verification/suite.json`, replays each input
  trend through the runtime and writes output traces.
- `scripts/verify-obc.py` (143 lines) pins SHA-256 hashes of reference files, runs the binary, and
  compares traces with LBNL `pyfunnel` using each case's `time_tolerance_s` and `value_tolerance`.
- The suite format (`id`, `source`, `graph` | `graph_file`, `graph_path`, `trend`, `output`,
  `input_offsets`, tolerances) is specific to this repository and undocumented outside the code.

## Tasks

### `cdl-verify`
- [ ] Library API: load a suite, run one case against `cdl-exec`, return the output trace and a
      pass/fail with the worst deviation in value and time.
- [ ] Document the suite format and version it (`"version": 2`). Accept graphs as CXF JSON-LD in
      addition to simplified JSON, since CXF is what other implementers will have.
- [ ] Comparison in Rust. Implement the funnel check natively so the harness works without Python.
      Keep `pyfunnel` in this repository's CI as an independent cross-check until the two have
      agreed on every case for a full release cycle.
- [ ] Reference-file pinning (the hash table in `verify-obc.py`) moves into the suite file.
- [ ] Trend input formats: the existing CSV layout, plus the Dymola `.txt` reference results already
      checked in under `verification/`, if the parsing in the `generate_*_reference.py` scripts is
      simple enough to port. Otherwise leave conversion to those scripts and say so in the docs.
- [ ] A trait for "implementation under test" so the harness can drive something other than
      `cdl-exec` (for example a controller reached over a socket). Only the `cdl-exec` implementation
      is required here.

### `cdl-cli`
- [ ] `cdl validate <file>`: structured diagnostics, non-zero exit on error, `--format json`.
- [ ] `cdl convert <in> <out>`: between simplified JSON and CXF JSON-LD, by extension or `--to`.
- [ ] `cdl run <graph> --inputs <csv> --dt <s> [--trace <path>...]`: write outputs as CSV.
- [ ] `cdl verify <suite.json>`: run a suite, print a table, exit non-zero on failure.
- [ ] `cdl blocks [--format json]`: list supported classes and coverage against CXF-Core.
- [ ] Replace `obc_verify` and the trace-running half of `verify-obc.py` in CI with `cdl verify`.

### Publishing
- [ ] Choose final crate names now that the architecture has been exercised (decision deferred from
      the review). Check availability on crates.io at that point.
- [ ] Decide license and add `LICENSE` files; confirm the bundled `CXF-Core.jsonld` and Modelica
      Buildings reference files are redistributable under their upstream licenses in a published crate.
- [ ] README with a runnable example, `#![deny(missing_docs)]`, `cargo doc` clean, and MSRV stated
      for each core crate.
- [ ] `cargo semver-checks` and a release workflow (tag → publish in dependency order).
- [ ] Decide repository layout: keep the core crates here, or move them to their own repository
      with the Explorer depending on published versions. Recommended: keep them here until 0.2, so
      that app and core can still change together.

## Acceptance

- `cdl verify verification/suite.json` reproduces today's pass/fail result for every case.
- Native funnel and `pyfunnel` agree on every case.
- A user with only `cargo install` can validate a CXF file from `modelica-json` and convert it.
- `cargo publish --dry-run` succeeds for each core crate in dependency order.
