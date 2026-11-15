# WP8 demand check: defer optional bindings

Decision: stop after the demand check. No `cdl-py`, wheels or changes to neighboring
repositories are required for this architecture migration. The CLI covers the
identified file-oriented integration points. There is no confirmed request here
for shared in-process Rust controller state, and the plan makes that demand a
prerequisite for starting bindings.

The local working trees inspected on 2026-11-14 were `../python-cdl` (HEAD
65fac3096f0369e3ba09bbe706290ac9a5b0521e) and `../ace-cdl-bog` (no Git metadata).
No files in either project were changed. This is a source inventory, not an assertion
that their maintainers have approved an integration or that their Python models
have identical semantics to the Rust model.

| Consumer and current entry point | Shared operation | Existing CLI surface |
| --- | --- | --- |
| python-cdl `validators/block_validator.py`, `validators/graph_validator.py` | Rust structural diagnostics for a serialized graph | `cdl validate graph.json --format json` |
| python-cdl `parser/json_parser.py`, `CDLParser.parse_block` | Read CXF before parsing the existing simplified schema | `cdl convert graph.jsonld graph.json` |
| Python tooling exporting simplified JSON | Write CXF without maintaining another writer | `cdl convert graph.json graph.jsonld` |
| ace-cdl-bog `cli.py` reads JSON then calls `CDLParser.parse_block` | Convert/validate an input artifact before the existing transpiler | the same `convert` and `validate` commands |
| Editors/generators needing block interfaces | Enumerate supported classes and interface declarations | `cdl blocks --format json` |
| Explorer `scripts/generate_g36_sample.py` writes a graph artifact | Validate generated output before downstream use | `cdl validate web/public/examples/four_pipe_g36.json --format json` |
| Offline Python trend verification/notebooks | Replay a complete input series and inspect outputs | `cdl run graph.json --inputs inputs.csv --dt 1 --trace y` or `cdl verify suite.json --format json` |

Subprocess calls exchange ordinary files/JSON/CSV. For example, a Python caller can
run `subprocess.run(["cdl", "validate", path, "--format", "json"], capture_output=True,
text=True)`, parse stdout diagnostics, and use exit codes 0/1/2 to distinguish valid,
invalid and command/execution failures. Conversion remains a separate explicit step;
a caller need not replace its Pydantic model or BOG transpiler.

python-cdl also has a stateful `runtime/executor.py` with `BlockExecutor.execute`
and an event context. The CLI does **not** replace interactive, step-at-a-time
in-process use. No request to migrate that runtime, measured subprocess bottleneck,
or requirement for cross-call Rust state was found in this task. If one is confirmed,
reopen WP8 and implement its full pyo3/maturin surface, type stubs, checks and wheels.
Do not claim that Python validation, native models or a BOG round trip have been
unified merely because conversion commands are available. Any adoption changes in
python-cdl or ace-cdl-bog remain work reviewed in those repositories, as WP8 specifies.
