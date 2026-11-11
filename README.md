# ACE CDL Explorer

**[Open the published CDL Explorer](https://ace-iot-solutions.github.io/ace-cdl-explorer/)**

[![Rust runtime tests](https://github.com/ACE-IoT-Solutions/ace-cdl-explorer/actions/workflows/runtime-checks.yml/badge.svg?branch=main)](https://github.com/ACE-IoT-Solutions/ace-cdl-explorer/actions/workflows/runtime-checks.yml)
[![CXF structure checks](https://github.com/ACE-IoT-Solutions/ace-cdl-explorer/actions/workflows/cxf-checks.yml/badge.svg?branch=main)](https://github.com/ACE-IoT-Solutions/ace-cdl-explorer/actions/workflows/cxf-checks.yml)
[![OBC time-series verification](https://github.com/ACE-IoT-Solutions/ace-cdl-explorer/actions/workflows/obc-verification.yml/badge.svg?branch=main)](https://github.com/ACE-IoT-Solutions/ace-cdl-explorer/actions/workflows/obc-verification.yml)
[![GitHub Pages](https://github.com/ACE-IoT-Solutions/ace-cdl-explorer/actions/workflows/pages.yml/badge.svg?branch=main)](https://github.com/ACE-IoT-Solutions/ace-cdl-explorer/actions/workflows/pages.yml)

A static Vue portal for learning and editing simplified CDL controls, with a Rust/WebAssembly runtime for closed-loop browser simulation. The default example is a synthetic four-pipe building with two AHUs, ten VAV zones, and a shared chiller, boiler, and reversible heat pump plant.

The example picker also includes a G36-style control graph for the same building. It adds ten raw Brick occupancy sensors, editable OBC optimal-start calculations and zone-group modes, occupied/standby/unoccupied setpoint and ventilation resets, a sampled pressure trim-and-respond graph, outdoor-airflow feedback, a simple free-cooling command, and sampled chilled-water supply reset from AHU requests. The simulation page compares completed runs from this browser; run both conventional and G36-style controls to compare their measured totals. See [model scope and limits](docs/runtime-and-model.md) and the [G36 implementation scope](docs/g36-implementation.md).

The simulation scenario can inject timed heat-pump, boiler, and chiller outages. Raw Brick `Status` inputs feed editable G36 plant CDL gates, which redistribute eligible load when a source becomes unavailable. Configurable physical startup delays produce running-status feedback for CDL water-reset holds; the physical model limits source delivery until startup completes.

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
cd web && npm ci && npm run test:cxf && npm run build
```

Run the [OBC time-series verification](verification/README.md) with `python -m pip install -r verification/requirements.txt && python scripts/verify-obc.py` from the repository root.

`web/dist` is the complete GitHub Pages artifact. The Pages workflow builds and deploys it on pushes to `main`. Enable GitHub Pages with **GitHub Actions** as the source in repository settings.

The Pages workflow adds Google Tag Manager only when the repository Actions secret `GTM_CONTAINER_ID` is set to a valid `GTM-...` container ID. The secret is absent from forks, so their builds omit the tag. Local Vite builds also omit it unless `scripts/inject-gtm.mjs` is run explicitly against the build artifact.

## What the project supports

- The editor imports and exports the simplified block JSON used in [`python-cdl`](https://github.com/ACE-IoT-Solutions/python-cdl), including its shipped examples. ACE projects wrap a graph and scenario in a [versioned JSON file](web/public/schemas/project-v1.json) and autosave in browser storage.
- The primer shows live excerpts of the bundled VAV graph using the editor's block, port, and connection styling. Change zone temperature, cooling setpoint, or demand gain to see the values move through the blocks.
- The Rust runtime implements the named elementary blocks used by the examples, safe arithmetic equations, typed wiring, composite execution, and stateful controls. The default building exposes nested VAV, AHU, and plant control graphs; select a composite and open it to edit its internal blocks. See [runtime and model notes](docs/runtime-and-model.md) for supported behavior and limits.
- Hover or focus a block to see its function and input/output ports; tap its information button on touch screens. Select a block to edit its notes, which appear in the tooltip and are saved with the project. The building example includes notes throughout its nested control graphs. Use the canvas toolbar to zoom from 50% to 200%.
- Drag empty canvas space to pan the graph. Edge arrows appear when more blocks are outside the view and can also move the canvas; scrollbars stay hidden. New graphs place sources and constants at the left, then use dependency depth for calculation columns; output point cards attach beside their producing blocks. Use **Arrange** to apply that layout after moving blocks.
- Top-level example connections cross the boundary between CDL controls and the building simulation. The editor shows these as IN/OUT terminals with point names on hover; open a composite to follow its internal block-to-block wires.
- The bundled weather dataset has 35,040 quarter-hour temperature and relative-humidity samples for 2025. Simulation timesteps can be 15 seconds, 30 seconds, 1 minute, 5 minutes, or 15 minutes. Weather is held during each 15-minute interval; controls, thermal physics, and energy totals update at every selected timestep. Full-year chart rows are recorded every 15 minutes.
- Recorded simulation results use Apache Arrow IPC between Rust/Wasm, the Web Worker, and Vue. The browser stores typed Arrow columns and reads only the rows needed for plots or CSV export; graph trace columns are captured at the 15-minute chart interval.
- The comparison saves the latest 12 completed run summaries in this browser, including control label, completion time, elapsed run time, effective timestep, electricity, boiler fuel, present-zone comfort exceedance, and all-zone comfort-band exceedance. The current-run summary shows those same four totals; cancelled runs are excluded. It plots only metrics from runs completed in this browser.
- The building simulation compiles editable CDL controls to indexed numeric slots once per run. An ignored Rust benchmark compares this path with the compatibility executor: `cargo test --release --manifest-path runtime/Cargo.toml benchmark_compiled_vs_dynamic -- --ignored --nocapture`.
- Simulation can trace multiple graph inputs, outputs, internal block outputs, connections, and physical process variables at each recorded 15-minute chart point. Add signals, then run the model; plots group signals by unit and include labeled y-axes. Drag across a signal plot to zoom; Shift+drag or turn on Pan charts to pan it. Drag a standard trend to pan. Date, zoom, earlier/later, and Full year controls keep all charts on the same time range and stay visible while scrolling through results.
- Example graph inputs and outputs carry Brick-aligned `brick_class` metadata. Modeled and normalized values use generic `brick:Point` rather than a physical sensor class. Connector IDs stay stable so saved projects still run. The primer and editor show Brick point cards separately from CDL calculation blocks, with orthogonal wires. The glossary at `#glossary` covers every supported runtime block type.
- Both four-pipe examples include a separate Brick/RealEstateCore building model: ten `rec:Room` spaces each belong to a `brick:HVAC_Zone`, and two AHUs feed the corresponding VAVs and zones. The editor shows this mapping above the control graph and exports it as Brick JSON-LD. Each synthetic HVAC zone represents one room; room dimensions and floor locations are unspecified.
- Fan speed limits delivered airflow, and actual supply-air temperature and normalized duct pressure feed back to the controls. Editable AHU CDL blocks command heating and cooling coil valves from measured temperatures. The plant graph commands hot/chilled loop pumps and source production from measured supply-water temperatures; the Rust model stores energy in mixed water volumes, limits delivered heat by flow and temperature difference, and reports return temperatures. VAV airflow requests drive the fan separately from pressure-affected flow estimates. Run metrics and energy use are calculated from these connected flows; [model notes](docs/runtime-and-model.md) list the remaining simplifications.
- Pages use URL hashes (`#primer`, `#editor`, `#simulation`, `#glossary`) so refresh, direct links, and browser history keep the current page on GitHub Pages.
- Styles and two components are vendored from the neighboring `ace-visual-system` repository so public builds do not require access to a private package.

To regenerate the synthetic dataset after changing the model:

```sh
python3 scripts/generate_examples.py
python3 scripts/generate_g36_sample.py
```

The physical model is intentionally simplified for control exploration and is not a design or energy compliance tool.
