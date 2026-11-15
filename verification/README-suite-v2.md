# Verification suite format, version 2

All paths resolve relative to the manifest's directory, independently of the working
directory. Files outside that directory can use `../`. Graphs can be simplified CDL
JSON or compacted CXF JSON-LD supported by `cxf`; no remote contexts are fetched.

```json
{
  "version": 2,
  "reference_commit": "upstream revision",
  "reference_files": {"input.csv": "64 hexadecimal SHA-256 digits"},
  "cases": [{
    "id": "controller_case",
    "source": "upstream model and experiment provenance",
    "graph_file": "controller.jsonld",
    "graph_path": [],
    "trend": "input.csv",
    "output": "y",
    "input_offsets": {},
    "time_tolerance_s": 0.1,
    "value_tolerance": 0.001
  }]
}
```

`graph` can contain an inline document instead of `graph_file`; specify exactly one.
`graph_path` selects nested child instances by name before compiling the experiment.
`output` defaults to `y` and accepts executor trace paths. Case IDs must be unique
and contain only ASCII letters, digits, underscore or hyphen. Tolerances must be
finite and nonnegative. Unknown manifest/case fields are rejected.

Every trend must have a `reference_files` pin. This map can also pin original source
models and raw upstream reference files. All hashes are checked before execution;
the loader never refreshes them. Review source/reference changes before updating
pins. Version 1 graph paths were relative to the repository root; version 2 graph
paths use the suite directory. The migration changes paths and pins, not trend data,
controller graphs or tolerances.

CSV uses a header with `time_s` first, scalar root input names, and `y` for the
reference observation. Header names must be unique. CSV quoting is supported.
Boolean inputs accept `true`/`false` or `0`/`1`. Numeric fields must be finite and
rows must strictly increase in time. Input offsets are finite constants added to
Real inputs. The first row uses dt=0; later rows use consecutive time differences.
For `cdl run`, the reference `y` column is optional and all input intervals must
match `--dt`. Input names other than required root connectors are ignored.

Dymola `.txt` conversion remains in the existing `scripts/generate_*_reference.py`
scripts. These conversions select model-specific arrays, construct input experiments
and handle upstream interpolation/event boundaries; a generic raw-text parser alone
would not reproduce the current experiments. Run their `--check` modes through
`scripts/verify-obc.py`. The portable Rust library consumes their pinned CSV outputs.
