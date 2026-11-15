# WP8: Python bindings (optional)

**Depends on:** WP5, WP7. **Size:** M. Start only if the team wants `python-cdl` and `ace-cdl-bog`
to share the Rust validator and CXF reader.

## Goal

A Python package built from the core crates so Python tooling stops needing its own copies of
validation, block interfaces and (missing today) CXF support.

## Why it may be worth doing

- `python-cdl` defines the simplified JSON this project grew from, and `ace-cdl-bog` consumes
  `python_cdl` models to generate Niagara BOG files. Neither can read CXF.
- The 35 `scripts/g36_*.py` generators here build graphs with a local `control_graph.py` factory and
  only learn about mistakes when the Rust tests run.

## Tasks

- [ ] Confirm demand first: list what `python-cdl` and `ace-cdl-bog` would call, and check whether
      the CLI from WP7 (invoked as a subprocess) already covers it. If it does, stop here.
- [ ] `crates/cdl-py` using `pyo3` and `maturin`, built with `uv`, Python 3.13.
- [ ] Minimal surface:
  - `validate(doc) -> list[Diagnostic]`
  - `load_cxf(path | str) -> Document`, `dump_cxf(doc) -> str`
  - `load_json` / `dump_json` for the simplified format
  - `blocks() -> list[BlockInfo]` from the registry
  - `Program.compile(doc)`, `.step(dt)`, input and output access, for tests and notebooks
- [ ] Documents cross the boundary as plain dicts (via `pythonize`) in the first version. Typed
      Python classes can follow if users ask.
- [ ] Type stubs, a `py.typed` marker, and Ruff and PyRefly clean per the team Python standards.
- [ ] Wheels for macOS arm64 and Linux x86_64 built in CI; publish to the internal index
      (`ace-pypi`) before considering PyPI.
- [ ] Use it here: have `scripts/generate_g36_sample.py` validate its output through the binding so
      generator errors surface at generation time.
- [ ] In `python-cdl`: add an optional extra that routes validation and CXF I/O through the binding.
      That change is planned and reviewed in the `python-cdl` repository, not here.

## Acceptance

- `uv add cdl-py` (from the internal index) then loading the official `CoolingCoil.jsonld` and
  validating it works in a clean environment.
- The Python `validate` returns the same diagnostics as `cdl validate --format json` for every
  fixture.
- `ace-cdl-bog` can load a CXF file through the binding and transpile it, demonstrated on one example.

## Open questions

- Whether `python-cdl`'s Pydantic models should remain the Python-side model, with this package only
  converting to and from them, or be replaced over time. That is a decision for the `python-cdl`
  maintainers.
