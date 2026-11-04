# ACE CDL Explorer

A static Vue portal for learning and editing simplified CDL controls, with a Rust/WebAssembly runtime for closed-loop browser simulation. The default example is a synthetic four-pipe building with two AHUs, ten VAV zones, and a shared chiller, boiler, and reversible heat pump plant.

## Run locally

Requirements: Rust with `wasm32-unknown-unknown`, `wasm-pack`, and Node 22 or later.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked
sh scripts/build-wasm.sh
cd web
npm ci
npm run dev
```

Open the Vite URL. The simulation and editor run locally in your browser; no server API or account is needed.

## Verify and build

```sh
cargo test --manifest-path runtime/Cargo.toml
sh scripts/build-wasm.sh
cd web && npm ci && npm run build
```

`web/dist` is the complete GitHub Pages artifact. The Pages workflow builds and deploys it on pushes to `main`. Enable GitHub Pages with **GitHub Actions** as the source in repository settings.

The Pages workflow adds Google Tag Manager only when the repository Actions secret `GTM_CONTAINER_ID` is set to a valid `GTM-...` container ID. The secret is absent from forks, so their builds omit the tag. Local Vite builds also omit it unless `scripts/inject-gtm.mjs` is run explicitly against the build artifact.

## What the project supports

- The editor imports and exports the simplified block JSON used in [`python-cdl`](https://github.com/ACE-IoT-Solutions/python-cdl), including its shipped examples. ACE projects wrap a graph and scenario in a [versioned JSON file](web/public/schemas/project-v1.json) and autosave in browser storage.
- The primer shows live excerpts of the bundled VAV graph using the editor's block, port, and connection styling. Change zone temperature, cooling setpoint, or demand gain to see the values move through the blocks.
- The Rust runtime implements the named elementary blocks used by the examples, safe arithmetic equations, typed wiring, composite execution, and stateful controls. The default building exposes nested VAV, AHU, and plant control graphs; select a composite and open it to edit its internal blocks. See [runtime and model notes](docs/runtime-and-model.md) for supported behavior and limits.
- Hover or focus a block to see its function and input/output ports; tap its information button on touch screens. Select a block to edit its notes, which appear in the tooltip and are saved with the project. The building example includes notes throughout its nested control graphs. Use the canvas toolbar to zoom from 50% to 200%.
- Drag empty canvas space to pan the graph. Edge arrows appear when more blocks are outside the view and can also move the canvas; scrollbars stay hidden.
- Top-level example connections cross the boundary between CDL controls and the building simulation. The editor shows these as IN/OUT terminals with point names on hover; open a composite to follow its internal block-to-block wires.
- The bundled weather dataset has 35,040 quarter-hour samples for 2025. Simulation timesteps can be 15 seconds, 30 seconds, 1 minute, 5 minutes, or 15 minutes. Weather is held during each 15-minute interval; controls, thermal physics, and energy totals update at every selected timestep. Full-year chart rows are recorded every 15 minutes, and the baseline trends are sampled hourly.
- Recorded simulation results use Apache Arrow IPC between Rust/Wasm, the Web Worker, and Vue. The browser stores typed Arrow columns and reads only the rows needed for plots or CSV export; graph trace columns are captured at the 15-minute chart interval.
- The building simulation compiles editable CDL controls to indexed numeric slots once per run. An ignored Rust benchmark compares this path with the compatibility executor: `cargo test --release --manifest-path runtime/Cargo.toml benchmark_compiled_vs_dynamic -- --ignored --nocapture`.
- Simulation can trace multiple graph inputs, outputs, internal block outputs, connections, and physical process variables at each recorded 15-minute chart point. Add signals, then run the model; plots group signals by unit and include labeled y-axes. Drag across a signal plot to zoom; Shift+drag or turn on Pan charts to pan it. Drag a standard trend to pan. Date, zoom, earlier/later, and Full year controls keep all charts on the same time range.
- Equipment-facing example ports carry readable Brick-aligned labels and `brick_class` metadata where a real point role applies. Connector IDs stay stable so saved projects still run. Calculated and normalized signals are identified separately. The glossary at `#glossary` covers every supported runtime block type and the primer explains how Brick points relate to CDL logic.
- Fan speed limits delivered airflow, plant output limits AHU coil and VAV reheat delivery, and actual supply-air temperature feeds back to the VAVs. The sample baseline and energy use are calculated from these connected flows; [model notes](docs/runtime-and-model.md) list the remaining simplifications.
- Pages use URL hashes (`#primer`, `#editor`, `#simulation`, `#glossary`) so refresh, direct links, and browser history keep the current page on GitHub Pages.
- Styles and two components are vendored from the neighboring `ace-visual-system` repository so public builds do not require access to a private package.

To regenerate the synthetic dataset and baseline after changing the model:

```sh
python3 scripts/generate_examples.py
cargo run --release --manifest-path runtime/Cargo.toml --bin baseline
```

The physical model is intentionally simplified for control exploration and is not a design or energy compliance tool.
