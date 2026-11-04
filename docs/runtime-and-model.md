# Runtime and sample model

## JSON and execution

The runtime accepts the simplified `python-cdl` block JSON shape: `name`, `block_type`, typed `inputs` and `outputs`, `parameters`, optional `equations`, and composite `blocks` with `connections`. Connections may use dotted `from` / `to` or explicit `from_block` / `from_output` / `to_block` / `to_input`. Imported equations use a restricted arithmetic evaluator with variables and `min` / `max`; arbitrary Python is never evaluated. The editor palette adds known blocks and parameters but does not offer equation text editing.

Known elementary blocks include Constant, Add, Subtract, Multiply, Min, MultiplyByParameter, Limiter, LimPID, Filter, LimRateLimiter, Hysteresis, Switch, Gain, Greater, Less, And, Or, Not, and Delay. A composite runs children in dependency order. The browser editor opens nested composites in a focused modal; users can add blocks, connections, and boundary ports there. Validation reports unsupported block types, missing endpoints, missing inputs, type mismatches, multiple drivers, equation parse errors, and cycles.

The named Python example JSON files include blocks with no equations. Their Rust implementations use the corresponding example code where available and standard control behavior otherwise. LimPID uses a clamped integral, Filter is first order, LimRateLimiter bounds change per second, and Hysteresis retains its previous Boolean output inside the deadband. Legacy standalone `VAVBoxController` JSON still has a built-in implementation for import compatibility. The default building uses a nested `VAVBoxController` composite made of elementary CDL blocks; its setpoints, PI gains, damper, reheat, airflow, and demand paths are editable. These implementations are functional interpretations of the example JSON, not a numerical port of all Python runtime code.

## Building and data

The generator in `scripts/generate_examples.py` produces a deterministic 2025 series: outdoor temperature combines annual, daily, and slower sinusoidal variations; sunlight varies by hour and season; weekday 08:00–18:00 occupancy raises internal gains. The times are UTC with no daylight-saving adjustment. The dataset is synthetic and does not represent a real weather station or building.

The example graph has ten nested VAV controllers, five per AHU, two AHU controller composites, and a plant dispatch composite containing chiller, boiler, and heat-pump control blocks. At each 15-minute step, zone temperatures and outdoor weather feed the graph; its AHU supply temperatures, fan speeds, VAV outputs, and plant dispatch commands feed the physical model. Plant and fan requests from the prior step enter the graph to avoid an algebraic loop. Graph edits to AHU reset, fan gain, zone logic, and plant dispatch affect the next simulation run.

Every zone uses a one-node heat balance with 0.15 kW/K envelope conductance and 5 kWh/K thermal capacity by default. The default AHU graph resets supply air to 18°C below 12°C outdoors, 16°C from 12–25°C, and 14°C above 25°C. Reheat capacity is 8 kW per zone. Plant commands cap the delivered heating and cooling; the default dispatch splits demand using the configurable heat-pump fraction. Electricity uses heat-pump COP 3.2 heating / 3.8 cooling, chiller COP 3.5, and cubic fan power. Boiler fuel uses 90% efficiency.

The building simulation expects the ten named VAV composites, two AHU composites, one plant composite, and their boundary inputs/outputs. Imported standalone control examples can be tested for a single step in the editor. Baseline values were calculated from the default graph and assumptions, then sampled hourly for the chart; the annual summary uses all 35,040 steps. Comfort counts quarter-hours outside 20–25°C occupied or 17–29°C unoccupied, summed across zones.

## Browser interface

The Web Worker initializes the Wasm module and accepts `validate`, `execute`, `simulate`, and `cancel` messages. The Rust interface exposes `validate_json(graph_json)`, `execute_json(graph_json, inputs_json, dt_seconds)`, and a `Simulation` object with `step_chunk(count)`. Simulation chunks return rows, progress, completion, and running totals. The worker yields between chunks to process cancellation.
