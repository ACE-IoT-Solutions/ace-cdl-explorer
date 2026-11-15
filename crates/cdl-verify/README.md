# CDL verification

This crate loads pinned version 2 verification suites and replays input CSVs through
`cdl-exec`. It has no simulator, Arrow, wasm binding or Python dependency. Rust 1.85
is checked with the locked dependencies. Publication archives pass isolated tests
and API-equivalence checks. Original project code uses MIT; LICENSE includes the
complete upstream terms.

```rust,no_run
use cdl_verify::{CdlImplementation, Suite};
let suite = Suite::load("verification/suite.json")?;
for case in &suite.cases {
    let result = suite.run_case(case, &mut CdlImplementation::default())?;
    assert!(result.comparison.passed, "{}", result.id);
}
# Ok::<(), cdl_verify::Error>(())
```

`Implementation` separates initialize/reset and stepping so another implementation,
including a socket client, can supply the same scalar observations. Every case starts
fresh; its first step has zero duration. Later durations are timestamp differences.
Root Boolean inputs require zero or one; integers must be integral and in range.
Offsets apply only to Real input declarations. Output and trace handles resolve once.

`compare` checks a piecewise-linear reference with absolute horizontal and vertical
tolerances. Its value envelope is the reference's minimum/maximum over each time
window, expanded by the value tolerance. It reports per-sample vertical excess,
the largest excess and its sample index, plus minimum extra horizontal displacement
needed for failing values. A missing horizontal result means a test value is outside
the entire reference value band. Pass/fail retains the existing 1e-10 excess threshold.
Reference endpoints are held within the horizontal tolerance. Samples outside that
expanded domain, duplicate timestamps, empty traces and nonfinite values fail.
Relative tolerances and duplicate-time discontinuities are not supported.

The repository independently cross-checks every suite output against LBNL `pyfunnel`,
plus 100 seeded passing/failing experiments (2000 samples). The suite requires 1e-10
agreement per sample; the seeded geometry check permits 2e-8 for floating-point
intersection differences and requires identical pass/fail. Keep these Python checks
for at least one full release cycle. The native implementation uses window extrema
and interpolation; it does not link or copy the C funnel implementation.

See [the version 2 suite format](../../verification/README-suite-v2.md).

A standalone comparison needs no suite or external file:

```rust
use cdl_verify::{compare, Sample};
let reference = [Sample { time: 0.0, value: 1.0 }, Sample { time: 1.0, value: 2.0 }];
let test = [Sample { time: 0.0, value: 1.01 }, Sample { time: 1.0, value: 2.01 }];
assert!(compare(&reference, &test, 0.0, 0.02)?.passed);
# Ok::<(), cdl_verify::Error>(())
```
