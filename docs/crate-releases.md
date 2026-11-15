# CDL crate releases

The core stays in this repository through 0.1.x so the Explorer and its libraries
can evolve together. Reconsider splitting repositories at 0.2. The app consumes
workspace paths; all published dependencies carry matching registry versions.

The first release is 0.1.0. The package names are `cdl-model`, `cdl-cxf`,
`cdl-blocks`, `cdl-compat`, `cdl-exec`, `cdl-verify`, and `cdl-cli`. The CXF package
retains the Rust library name `cxf`. On 2026-11-14 the crates.io API returned 404 for
these seven names. `cxf` returned 200 for an unrelated Credential Exchange Format
crate, which is why this project uses `cdl-cxf`. Availability is not a reservation;
recheck before the first upload.

Original project code uses MIT. Each published package includes a LICENSE with
that grant, licensing scope, provenance, and complete upstream modelica-json and
Modelica Buildings terms. These upstream licenses permit source/binary redistribution
with their notices and conditions retained; the extra enhancements paragraph is
preserved. See THIRD_PARTY_NOTICES.md and the original files in `licenses/`.
The pinned schema and upstream fixtures retain their original licensing.
The published verification library contains no raw Buildings reference corpus;
repository regression files remain in the unpublished integration harness.
Dependency licenses are not replaced by the project license.

Rust 1.85 is the MSRV for all eight current packages. CI checks all features using 1.85
and the lockfile. Package publication/verification uses current stable Cargo with
workspace publishing support. The browser wrapper, development tools, original-runtime oracle and integration
harness are marked `publish = false`. They are never selected for release.

## Local verification

```sh
cargo install cargo-semver-checks --locked
python3 scripts/check-semver.py
python3 scripts/check-packages.py --allow-dirty --semver
```

The package check runs `cargo publish --dry-run` on all eight packages. Cargo stages
and verifies their dependency graph without uploading. The script then extracts
only those archives into a fresh temporary directory, verifies normalized manifests
and licenses, tests every packaged crate with all features, and installs the CLI.
A temporary workspace patch points exclusively to the extracted release artifacts;
it substitutes for unavailable registry dependencies before the first publication.
The installed native binary validates and converts the pinned upstream CoolingCoil
fixture without using the repository's runtime or Python at execution time.
`--allow-dirty` is for local review; CI uses a clean checkout.

`--semver` compares the workspace APIs with the exact archives via cargo-semver-checks.
This establishes that published API artifacts agree with their source. The separate
historical check compares with the latest reachable `cdl-v*` release tag (excluding
the current commit), using the actual package version bump to assess changes. The
first release has no prior API baseline: it reports that explicitly and relies on
archive equivalence; it makes no historical compatibility claim. Preserve release
tags and fetch full history for future checks.

Published package tests are self-contained. The unpublished `cdl-integration-tests`
package additionally retains the full original-engine and Explorer-corpus tests.
Run both its full-feature and no-default-feature configurations. The second exercises
the existing equation feature rejection checks. Every CI and release still runs
independent pyfunnel comparisons; do not remove those for at least one full release
cycle of agreement.

## Tag workflow

`crate-packages.yml` runs archive tests, semver, documentation and MSRV on pull
requests. `crates-release.yml` verifies a `cdl-v<version>` tag, including native
regressions, annual balances, native/pyfunnel agreement, package/install checks,
wasm/JSON-LD/frontend checks and the full-year browser parity test. The publish job
runs only after those gates pass and checks that the tag matches every crate version.
A manual workflow run performs verification without publishing.

The publish job uses the repository's `CARGO_REGISTRY_TOKEN` secret and publishes
in this order: model, CXF, blocks, compatibility, executor, verification, building plant, CLI.
Configure a crates.io account/token with rights to those names and verify the
account email before invoking it. Crates.io rejects uploads from an account without
a verified email, even when Cargo authentication succeeds.
Create/push a release tag only when the reviewed release is ready. Publication is
permanent; package versions cannot be overwritten. `scripts/publish-crates.py` verifies checksums and exact source files before skipping
an already-published version, stops if its contents differ, and honors the registry
retry time for new-crate rate limits. It can resume a partially published release
without trying to overwrite existing versions. `--check` inspects the release without
uploading. The workflow finishes with `scripts/verify-published.py`, which downloads
all eight actual registry archives and tests a clean `cargo install` and all five CLI
commands against the pinned upstream CXF fixture.

This workflow publishes crates only. The GitHub Pages production workflow remains
separate; no production deployment is performed by local verification or packaging.

## Published 0.1.0

All seven packages are available on crates.io. The registry checksum/source receipts
and clean-install results are in [crate-release-0.1.0.json](crate-release-0.1.0.json).

```sh
cargo install cdl-cli --version 0.1.0 --locked
```

The CLI installed from the registry passes all five commands, including validation
and conversion of the pinned upstream CoolingCoil fixture. Published cdl-verify also
passes all 143 cases from a separate registry-only Rust consumer. Original source
files in 0.1.0 remain identical to the verified candidate; the package READMEs retain
pre-publication checkpoint notes, while this document records the final status.
No production website deployment or Git source push accompanied publication.

## Release 0.1.1

This release adds optional serialized block/executor checkpoints, explicit PI/PID
output tracking for bumpless handover, and the first registry release of
`ace-building-sim` with a public controller-independent plant step API. All eight
packages use 0.1.1. See [the API guide](controller-state-and-plant-api.md).
The plant's repository development binaries live in the unpublished
`ace-building-sim-tools` package. Packaged tests use synthetic weather; the full
repository retains the original oracle/fixture regression tests.

The release verifier also builds a separate registry-only native consumer that
steps the plant, checks invalid-command atomicity, and restores serialized state
into a freshly compiled program. Its receipt is saved with the CLI/archive checks.
