# G36 implementation scope

Target the applicable air-side and hot/chilled-water sequences in ASHRAE Guideline 36-2024. The four-pipe heat pump shared with a boiler and chiller needs an explicit equipment-specific sequence alongside those provisions. The current example is **G36-style**, not a verified Guideline 36 implementation.

## Current working slice

- The G36 sample uses scheduled, occupied-standby, unoccupied, and six-hour preconditioning setpoints. Ten synthetic raw occupancy sensors are debounced by editable CDL `TrueDelay` and `Latch` blocks. The six-hour preconditioning decision is also computed by CDL from a calendar lookahead input.
- Each generated VAV has separate requested airflow and a pressure-affected estimate. Fan control uses the graph request, avoiding a low-pressure deadlock. The physical model returns a normalized duct-pressure measurement to the graph and no longer invents a legacy airflow request.
- Each G36-style AHU selects 20% or 80% outdoor air based on outdoor, return, and supply-target temperatures. The commanded share changes mixed-air temperature, coil demand, energy, and zone temperature. This is a discrete free-cooling demonstration, not the G36 modulating economizer.
- The conventional AHUs command their fixed 20% outdoor-air share through editable CDL constants. Rust reads both samples' graph outputs for the mixing box; it does not select a default damper command.
- The physical model now returns ten measured primary-airflow and discharge-temperature signals. The G36-style VAV graph uses measured flow and supply temperature to cap reheat against an editable 35 °C discharge target, and uses measured flow and discharge temperature for five-minute fault alarms. The plant still has a separate 40 °C equipment safety cap. The 15-minute sample and one-step sensor delay can overshoot the 35 °C target transiently; this is not yet a verified G36 discharge control sequence.
- A heating call now opens each G36-style VAV damper beyond its ventilation minimum as the editable PI demand rises; the resulting airflow request drives fan delivery. This improves person-present comfort over the fixed-minimum heating path, but still does not implement the G36 active heating flow setpoint and valve control loops.
- The physical plant caps the heat pump's combined heating/cooling command and separately caps boiler and chiller output. Both control programs use the same limits. Rust tests cover the pressure recovery, outdoor-air path, and plant shortfall.

## Remaining system work

| Layer | Required next work | Verification gate |
| --- | --- | --- |
| VAV terminal | Replace the teaching discharge cap and two fault flags with the G36 discharge setpoint, valve/damper loops, full alarms and suppression; add window/CO₂ options, ventilation minimums, and integer zone/AHU/plant requests. | Reference traces for occupied, standby, heating, cooling, and fault transitions. |
| Multizone AHU | Add measured outdoor flow, mixed-air and discharge points, damper and fan feedback, supply-air and duct-pressure reset, trim-and-respond, modulating economizer, ventilation, freeze protection, and mode transitions. | Reference traces across outdoor conditions and zone requests; preserve air and heat balance. |
| Hot/chilled water | Model hot/chilled supply and return temperature, water flow, pump and valve behavior, pipe/mixing volume, equipment staging and limits. Add plant-enable and temperature/pressure-reset controls. | Load step and unavailable-equipment tests; delivered coil load must match water-side heat transfer. |
| Hybrid plant | Specify when the four-pipe heat pump heats, cools, or recovers heat, plus boiler/chiller fallback and capacity/efficiency curves. Identify which rules are equipment-specific extensions. | Capacity and energy balance tests for simultaneous heating/cooling and transitions. |
| CDL runtime | Add typed integer and Boolean blocks, timers, latches, sampled reset logic, arrays, and alarm suppression needed by the chosen reference sequences. | Time-series checks against OpenBuildingControl examples in CI. |

The synthetic year currently provides only outdoor temperature, solar/internal gains, schedule, and generated presence. The physical model derives primary airflow and discharge temperature from equipment delivery. CO₂, humidity, outdoor airflow, water temperatures/flows, and actuator/status points require physical models and documented design values before their controls can be evaluated. A one-node sensible zone cannot verify ventilation or latent performance.

Keep annual comparisons on the same weather, model, capacities, and timestep. Report person-present comfort alongside energy. With current defaults at 15-minute steps, the conventional sample records 59,564 kWh electricity, 99,695 kWh boiler fuel, and 0 person-present comfort exceedance zone-hours; the G36-style sample records 42,225 kWh, 76,722 kWh, and **122 zone-hours**. The G36-style sample still fails occupied comfort, so those energy differences do not establish G36 performance.

References: [ASHRAE Guideline 36-2024 contents](https://store.accuristech.com/ashrae/products/preview/2926782), [OpenBuildingControl G36 VAV terminal package](https://simulationresearch.lbl.gov/modelica/releases/v14.0.0/help/Buildings_Controls_OBC_ASHRAE_G36_TerminalUnits.html), [multizone AHU economizer package](https://simulationresearch.lbl.gov/modelica/releases/v14.0.0/help/Buildings_Controls_OBC_ASHRAE_G36_AHUs_MultiZone_VAV_Economizers.html), and [chiller plant package](https://simulationresearch.lbl.gov/modelica/releases/v14.0.0/help/Buildings_Controls_OBC_ASHRAE_G36_Plants_Chillers.html).
