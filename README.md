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

## What the project supports

- The editor imports and exports the simplified block JSON used in [`python-cdl`](https://github.com/ACE-IoT-Solutions/python-cdl), including its shipped examples. ACE projects wrap a graph and scenario in a [versioned JSON file](web/public/schemas/project-v1.json) and autosave in browser storage.
- The Rust runtime implements the named elementary blocks used by the examples, safe arithmetic equations, typed wiring, composite execution, and stateful controls. The default building exposes nested VAV, AHU, and plant control graphs; select a composite and open it to edit its internal blocks. See [runtime and model notes](docs/runtime-and-model.md) for supported behavior and limits.
- Hover or focus a block to see its function and input/output ports; tap its information button on touch screens. Select a block to edit its notes, which appear in the tooltip and are saved with the project. The building example includes notes throughout its nested control graphs. Use the canvas toolbar to zoom from 50% to 200%.
- Drag empty canvas space to pan the graph. Edge arrows appear when more blocks are outside the view and can also move the canvas; scrollbars stay hidden.
- The bundled weather dataset has 35,040 quarter-hour samples for 2025. The baseline trends are sampled hourly from the same engine; a custom run computes all 35,040 steps.
- Simulation can trace any graph input, output, internal block output, or connection at every 15-minute step. Search for a signal to run its trace, then drag across the chart or use the date and zoom controls to focus the time window. The same window applies to the temperature and energy plots.
- Tabs use URL hashes (`#primer`, `#editor`, `#simulation`) so refresh, direct links, and browser history keep the current page on GitHub Pages.
- Styles and two components are vendored from the neighboring `ace-visual-system` repository so public builds do not require access to a private package.

To regenerate the synthetic dataset and baseline after changing the model:

```sh
python3 scripts/generate_examples.py
cargo run --release --manifest-path runtime/Cargo.toml --bin baseline
```

The physical model is intentionally simplified for control exploration and is not a design or energy compliance tool.
