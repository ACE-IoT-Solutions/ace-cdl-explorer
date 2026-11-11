<template>
  <div class="app-shell">
    <header class="site-header"><div class="brand" @click="navigate('primer')"><span class="brand-mark">ACE<span>◆</span></span><span class="brand-divider"/><span class="brand-name">CDL <b>EXPLORER</b></span></div><nav class="top-nav" aria-label="Main navigation"><button :class="{active:tab==='primer'}" @click="navigate('primer')">Primer</button><button :class="{active:tab==='editor'}" @click="navigate('editor')">Visual editor</button><button :class="{active:tab==='simulation'}" @click="navigate('simulation')">Simulation</button><button :class="{active:tab==='glossary'}" @click="navigate('glossary')">Glossary</button></nav><span class="header-tag">BROWSER-BASED CONTROLS LAB</span></header>

    <main v-if="tab==='primer'"><PrimerPage/><section class="cta-band"><div><span class="eyebrow">READY TO EXPERIMENT?</span><h2>Turn the sequence into a working model.</h2></div><button class="solid-button" @click="navigate('editor')">Open the visual editor ↗</button></section></main>

    <main v-else-if="tab==='glossary'"><GlossaryPage/></main>

    <main v-else-if="tab==='editor'" class="workspace-page"><div class="workspace-heading"><div><span class="eyebrow">WORKSPACE / CONTROL GRAPH</span><h1>{{ project?.name ?? 'Loading controls…' }}</h1><p>Build a sequence from blocks, tune parameters, and test its outputs.</p></div><div class="workspace-actions"><select v-model="exampleChoice" aria-label="Load example"><option value="">Load an example…</option><option v-for="example in examples" :key="example.file" :value="example.file">{{ example.label }}</option></select><button class="outline-button" @click="loadExample">Load</button><button class="outline-button" @click="triggerImport">Import JSON / CXF</button><button class="outline-button" :disabled="!project" @click="exportProject">Export project</button><button class="outline-button" :disabled="!project" @click="exportCxfFile">Export CXF</button><button v-if="project?.graph.brick_model" class="outline-button" @click="exportBrickFile">Export Brick model</button><input ref="fileInput" class="sr-only" type="file" accept="application/json,application/ld+json,.json,.jsonld" @change="importFile"/></div></div>
      <div v-if="storageWarning" class="error-banner">{{ storageWarning }} <button v-if="recoverableProject" class="outline-button" type="button" @click="downloadSavedProject">Download saved project</button><button v-if="recoverableProject" class="outline-button" type="button" @click="loadCurrentExample">Load current example</button></div><div v-if="runtimeError" class="error-banner">{{ runtimeError }} <button class="outline-button" type="button" @click="reloadLatestRuntime">Reload latest runtime</button></div><div v-if="!project && errors.length" class="error-banner">{{ errors.join("; ") }}</div>
      <div v-if="project" class="workspace-body"><div class="validation-strip" :class="{invalid:errors.length}"><span>{{ errors.length ? `● ${errors.length} graph issue${errors.length===1?'':'s'}` : '● Graph ready' }}</span><span>{{ project.graph.blocks?.length ?? 0 }} top-level blocks · Local autosave</span></div><div v-if="errors.length" class="validation-list"><p v-for="(error,i) in errors.slice(0,8)" :key="i">{{ error }}</p></div><GraphEditor :key="project.graph.name" :project="project" @change="updateProject"/><BuildingSpaces v-if="project.graph.brick_model" :model="project.graph.brick_model"/><section v-if="!isBuildingGraph" class="try-panel"><div><span class="eyebrow">QUICK TEST</span><h2>Run one control step</h2><p>Supply values for the imported sequence and inspect its output.</p></div><div class="try-inputs"><label v-for="input in project.graph.inputs ?? []" :key="input.name" class="field"><span>{{ input.name }} <small>{{ input.unit }}</small></span><select v-if="input.type==='Boolean'" v-model="testInputs[input.name]"><option :value="true">True</option><option :value="false">False</option></select><input v-else type="number" step="any" v-model.number="testInputs[input.name]"/></label></div><button class="solid-button" :disabled="errors.length>0" @click="runStep">Run step</button><pre v-if="testResult">{{ testResult }}</pre></section></div>
    </main>

    <main v-else class="simulation-page"><section class="simulation-hero"><div><span class="eyebrow">THE FOUR-PIPE BUILDING</span><h1>One year. Ten zones. Your control logic.</h1><p>A documented synthetic mixed-climate year drives two AHUs, ten VAVs, and shared hot and chilled water loops. Load the conventional or G36-style sample in the editor, then run it against the same building model.</p><div class="hero-actions"><AceButton :disabled="!project || !isBuildingGraph || errors.length>0 || running" @click="runYear">{{ running ? 'Running…' : 'Run 2025 simulation' }}</AceButton><button v-if="running" class="outline-button" @click="cancelRun">Cancel</button><button class="outline-button" @click="navigate('editor')">Edit controls ↗</button></div><p v-if="!isBuildingGraph" class="simulation-notice">Load a Four-pipe building example in the editor to run the building simulation.</p></div><div class="hero-stat"><span>2025 SIMULATION</span><strong>{{ number(totalSteps) }}</strong><small>{{ stepLabel }} steps · 35,040 weather samples</small></div></section>
      <div class="sim-layout"><aside class="sim-side"><div class="panel-title"><span class="eyebrow">SCENARIO</span><h2>Building assumptions</h2></div><p class="muted small">These simplified thermal assumptions apply to all ten zones.</p><PollutionScenario v-if="isG36Sample" :scenario="scenario" :running="running" @change="saveScenario"/><label v-if="isG36Sample" class="field"><span>Outdoor humidity source</span><select :value="scenario.outdoor_relative_humidity===null ? 'weather' : 'fixed'" :disabled="running" @change="setHumiditySource"><option value="weather">Synthetic 15-minute weather</option><option value="fixed">Fixed override</option></select></label><label v-if="isG36Sample && scenario.outdoor_relative_humidity!==null" class="field"><span>Fixed relative humidity · fraction</span><input v-model.number="scenario.outdoor_relative_humidity" type="number" min="0" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><label v-if="isG36Sample" class="field"><span>Return-air humidity source</span><select :value="scenario.return_air_relative_humidity===null ? 'modeled' : 'fixed'" :disabled="running" @change="setReturnHumiditySource"><option value="modeled">Modeled from zone moisture</option><option value="fixed">Fixed override</option></select></label><label v-if="isG36Sample && scenario.return_air_relative_humidity!==null" class="field"><span>Fixed return relative humidity · fraction</span><input v-model.number="scenario.return_air_relative_humidity" type="number" min="0" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><p v-if="isG36Sample" class="muted small">The synthetic outdoor humidity and physical zone moisture balance supply raw enthalpy inputs; editable CDL decides whether to economize. A saved fixed return-air value remains an explicit override.</p><label class="field"><span>Maximum simulation timestep</span><select v-model.number="scenario.step_seconds" :disabled="running" @change="saveScenario"><option :value="900">15 minutes</option><option :value="300">5 minutes</option><option :value="60">1 minute</option><option :value="30">30 seconds</option><option :value="15">15 seconds</option></select></label><p class="muted small">Weather updates every 15 minutes. The controls and building physics run at the selected maximum step or a smaller step required by sampled or timed CDL blocks. Charts record one value per weather interval.</p><label class="field"><span>AHU damper full stroke · seconds</span><input v-model.number="scenario.ahu_damper_stroke_seconds" type="number" min="1" max="3600" step="15" :disabled="running" @change="saveScenario"/></label><p class="muted small">Outdoor and return dampers move toward their CDL commands at this physical rate. Fault positions override the actuator.</p><p v-if="effectiveStepSeconds<scenario.step_seconds" class="muted small">This graph contains timed CDL controls. The runtime uses {{ effectiveStepSeconds }}-second internal steps to resolve their timing.</p><label class="field"><span>Zone envelope · kW/K</span><input v-model.number="scenario.zone_ua_kw_per_k" type="number" min="0.01" max="2" step="0.01" @change="saveScenario"/></label><label class="field"><span>Thermal capacity · kWh/K</span><input v-model.number="scenario.zone_capacity_kwh_per_k" type="number" min="0.1" max="50" step="0.1" @change="saveScenario"/></label><details class="field"><summary>AHU physical fan capacity</summary><p class="muted small">Full-speed supply fan airflow in the simplified air network. Fan speed commands and duct-pressure resets remain editable CDL controls.</p><div class="terminal-flow-grid"><label v-for="(_,index) in scenario.ahu_fan_design_airflow" :key="index"><span>AHU {{ index+1 }} · normalized airflow</span><input v-model.number="scenario.ahu_fan_design_airflow[index]" type="number" min="0.1" max="20" step="0.1" :disabled="running" @change="saveScenario"/></label></div></details><details class="field"><summary>AHU physical fan startup</summary><p class="muted small">The motor takes this long to prove on after a positive CDL speed command. During startup, measured fan speed and delivered air remain zero; the editable CDL fan alarm responds to that proof.</p><div class="terminal-flow-grid"><label v-for="(_,index) in scenario.ahu_fan_startup_seconds" :key="`fan-start-${index}`"><span>AHU {{ index+1 }} · seconds</span><input v-model.number="scenario.ahu_fan_startup_seconds[index]" type="number" min="0" max="3600" step="15" :disabled="running" @change="saveScenario"/></label></div></details><details class="field"><summary>VAV physical design airflow</summary><p class="muted small">Each zone’s fully open VAV capacity at normalized duct pressure 1. These values describe physical equipment; setpoints, damper commands, and alarm thresholds remain editable in CDL blocks.</p><div class="terminal-flow-grid"><label v-for="(_,index) in scenario.terminal_design_flow" :key="index"><span>Zone {{ String(index+1).padStart(2,'0') }} · normalized airflow</span><input v-model.number="scenario.terminal_design_flow[index]" type="number" min="0.01" max="5" step="0.01" :disabled="running" @change="saveScenario"/></label></div></details><details class="field"><summary>Water pump physical startup</summary><p class="muted small">Motor proof and hydraulic flow begin after this physical delay from a positive CDL speed command. The graph decides the pump start diagnostic and source proof from the measured status.</p><div class="terminal-flow-grid"><label v-for="(_,index) in scenario.water_pump_startup_seconds" :key="`pump-start-${index}`"><span>{{ index===0 ? 'Hot' : 'Chilled' }} pump · seconds</span><input v-model.number="scenario.water_pump_startup_seconds[index]" type="number" min="0" max="3600" step="15" :disabled="running" @change="saveScenario"/></label></div></details><details class="field"><summary>Water-loop physical pump curves</summary><p class="muted small">Tune the hot and chilled loop pump design flow and shutoff head in the simplified hydraulic network. Measured flow and pressure return to the editable CDL pump and source-proof blocks on the next step.</p><div class="terminal-flow-grid"><label v-for="(_,index) in scenario.water_pump_design_flow_kg_s" :key="index"><span>{{ index===0 ? "Hot" : "Chilled" }} loop design flow · kg/s</span><input v-model.number="scenario.water_pump_design_flow_kg_s[index]" type="number" min="0.1" max="20" step="0.1" :disabled="running" @change="saveScenario"/></label><label v-for="(_,index) in scenario.water_pump_shutoff_pressure_pa" :key="`head-${index}`"><span>{{ index===0 ? "Hot" : "Chilled" }} pump shutoff head · Pa</span><input v-model.number="scenario.water_pump_shutoff_pressure_pa[index]" type="number" min="10000" max="500000" step="1000" :disabled="running" @change="saveScenario"/></label></div><p class="muted small">The coefficients below describe physical bypass and valve conductance. They change measured flow and pressure at the same CDL pump command. Bypass flow does not deliver coil heat. Total opening is capped at one.</p><div class="terminal-flow-grid"><label v-for="(_,index) in scenario.water_bypass_conductance" :key="`bypass-${index}`"><span>{{ index===0 ? "Hot" : "Chilled" }} loop bypass conductance</span><input v-model.number="scenario.water_bypass_conductance[index]" type="number" min="0" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><label v-for="(_,index) in scenario.water_coil_conductance" :key="`coil-conductance-${index}`"><span>{{ index===0 ? "Hot" : "Chilled" }} coil valve conductance</span><input v-model.number="scenario.water_coil_conductance[index]" type="number" min="0" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><label><span>Hot loop reheat valve conductance</span><input v-model.number="scenario.hot_reheat_conductance" type="number" min="0" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label></div></details><details class="field"><summary>Zone CO₂ physical assumptions</summary><p class="muted small">The simulator mixes outdoor and return air, adds CO₂ from raw presence, and sends the measured concentration to CDL. Open-window infiltration also changes zone sensible heat; the closed-window envelope value already includes ordinary leakage heat loss. Ventilation commands are calculated in the editable graph. One normalized airflow unit is treated as 1 m³/s.</p><label><span>Zone air volume · m³</span><input v-model.number="scenario.zone_air_volume_m3" type="number" min="1" max="10000" step="10" @change="resetZoneAirProperty('zone_air_volume_m3_by_zone')"/></label><label><span>Infiltration · m³/s per zone</span><input v-model.number="scenario.zone_infiltration_m3_s" type="number" min="0" max="5" step="0.005" @change="resetZoneAirProperty('zone_infiltration_m3_s_by_zone')"/></label><label><span>Additional open-window infiltration · m³/s per zone</span><input v-model.number="scenario.open_window_additional_infiltration_m3_s" type="number" min="0" max="5" step="0.01" @change="resetZoneAirProperty('open_window_additional_infiltration_m3_s_by_zone')"/></label><details><summary>Individual zone air properties</summary><p class="muted small">These physical volumes and air-exchange rates shape measured CO₂; open-window flow also changes sensible heat. The shared settings above reset individual values.</p><div class="terminal-flow-grid"><label v-for="(_,index) in 10" :key="`volume-${index}`"><span>Zone {{ String(index+1).padStart(2,'0') }} · air volume · m³</span><input :value="scenario.zone_air_volume_m3_by_zone?.[index] ?? scenario.zone_air_volume_m3" type="number" min="1" max="10000" step="10" @change="setZoneAirProperty('zone_air_volume_m3_by_zone',index,$event)"/></label><label v-for="(_,index) in 10" :key="`infiltration-${index}`"><span>Zone {{ String(index+1).padStart(2,'0') }} · infiltration · m³/s</span><input :value="scenario.zone_infiltration_m3_s_by_zone?.[index] ?? scenario.zone_infiltration_m3_s" type="number" min="0" max="5" step="0.005" @change="setZoneAirProperty('zone_infiltration_m3_s_by_zone',index,$event)"/></label><label v-for="(_,index) in 10" :key="`window-infiltration-${index}`"><span>Zone {{ String(index+1).padStart(2,'0') }} · added open-window flow · m³/s</span><input :value="scenario.open_window_additional_infiltration_m3_s_by_zone?.[index] ?? scenario.open_window_additional_infiltration_m3_s" type="number" min="0" max="5" step="0.01" @change="setZoneAirProperty('open_window_additional_infiltration_m3_s_by_zone',index,$event)"/></label></div></details><label><span>Default people per present zone</span><input v-model.number="scenario.design_occupants_per_zone" type="number" min="0" max="1000" step="1" @change="resetZoneDesignOccupants"/></label><details><summary>People per individual zone</summary><p class="muted small">These raw design counts feed the Brick occupancy-count points and the physical CO₂ source. Changing the default above resets individual counts.</p><div class="terminal-flow-grid"><label v-for="(_,index) in 10" :key="`occupants-${index}`"><span>Zone {{ String(index+1).padStart(2,'0') }} · people when present</span><input :value="scenario.zone_design_occupants?.[index] ?? scenario.design_occupants_per_zone" type="number" min="0" max="1000" step="1" @change="setZoneDesignOccupants(index,$event)"/></label></div></details><label><span>Occupant moisture · kg/h per person</span><input v-model.number="scenario.occupant_moisture_kg_h_per_person" type="number" min="0" max="0.5" step="0.01" :disabled="running" @change="saveScenario"/></label><label><span>CO₂ generation · L/s per person</span><input v-model.number="scenario.co2_generation_lps_per_person" type="number" min="0" max="0.1" step="0.001" @change="saveScenario"/></label><label><span>Occupant sensible heat · kW/person</span><input v-model.number="scenario.occupant_sensible_heat_kw_per_person" type="number" min="0" max="0.3" step="0.005" @change="saveScenario"/></label><p class="muted small">Physical occupant-count changes add or remove this heat from the bundled weather’s synthetic internal gains. Default synthetic occupancy keeps its original heat balance.</p><label><span>Outdoor CO₂ · ppm</span><input v-model.number="scenario.outdoor_co2_ppm" type="number" min="250" max="2000" step="10" @change="saveScenario"/></label></details><p class="muted small">Tune heat pump dispatch in the plant composite’s <code>{{ isG36Sample ? "heat_pump_utilization_limit" : "heat_pump_dispatch_fraction" }}</code> CDL block in the visual editor. The performance points below describe physical equipment capacity and efficiency; they do not choose operating modes.</p><label class="field"><span>Heat pump capacity · kW</span><input v-model.number="scenario.heat_pump_capacity_kw" type="number" min="1" step="5" @change="saveScenario"/></label><label class="field"><span>Fallback simultaneous-service compressor savings · fraction</span><input v-model.number="scenario.heat_recovery_compressor_savings_fraction" type="number" min="0" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><p class="muted small">Used when the joint map omits simultaneous_cop: 0 uses separate heating and cooling compressor work; 1 avoids the smaller of those two loads. Heat-pump mode and thermal dispatch still come from editable CDL.</p><HeatPumpJointMapEditor :value="scenario.heat_pump_joint_performance" :disabled="running" @save="saveJointPerformanceMap"/><details class="field"><summary>Heat pump performance vs outdoor temperature</summary><p class="muted small">Synthetic equipment data. Capacity is a fraction of nameplate; COP values set compressor electricity. Values interpolate between points and hold outside the listed range. Keep outdoor temperatures increasing.</p><div v-for="(point,index) in scenario.heat_pump_performance" :key="index" class="performance-point"><label><span>Outdoor · °C</span><input v-model.number="point.outdoor_c" type="number" step="1" :disabled="running" @change="saveScenario"/></label><label><span>Capacity fraction</span><input v-model.number="point.capacity_fraction" type="number" min="0.01" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><label><span>Heating COP</span><input v-model.number="point.heating_cop" type="number" min="0.1" step="0.1" :disabled="running" @change="saveScenario"/></label><label><span>Cooling COP</span><input v-model.number="point.cooling_cop" type="number" min="0.1" step="0.1" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running || scenario.heat_pump_performance.length<=1" @click="removeHeatPumpPoint(index)">Remove point</button></div><button class="outline-button" type="button" :disabled="running" @click="addHeatPumpPoint">Add point</button></details><details class="field"><summary>Heat pump COP vs delivered part load</summary><p class="muted small">Synthetic physical COP multipliers applied after CDL dispatch and water-loop delivery. A flat 1.0 curve preserves the existing model. Enter increasing load fractions from 0 to 1; manufacturer data can replace these points.</p><div v-for="(point,index) in scenario.heat_pump_part_load_performance" :key="index" class="performance-point"><label><span>Delivered load fraction</span><input v-model.number="point.load_fraction" type="number" min="0" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><label><span>COP multiplier</span><input v-model.number="point.cop_multiplier" type="number" min="0.1" max="2" step="0.05" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running || scenario.heat_pump_part_load_performance.length<=2" @click="removePartLoadPoint(index)">Remove point</button></div><button class="outline-button" type="button" :disabled="running" @click="addPartLoadPoint">Add point</button></details><details class="field"><summary>Heat pump performance vs hot-water temperature</summary><p class="muted small">Physical heating capacity and COP multipliers from measured hot-water supply temperature at the start of each step. Flat 1.0 preserves prior runs; enter increasing °C points.</p><div v-for="(point,index) in scenario.heat_pump_hot_water_cop_performance" :key="index" class="performance-point"><label><span>Hot water · °C</span><input v-model.number="point.water_c" type="number" min="-20" max="100" step="1" :disabled="running" @change="saveScenario"/></label><label><span>COP multiplier</span><input v-model.number="point.cop_multiplier" type="number" min="0.1" max="2" step="0.05" :disabled="running" @change="saveScenario"/></label><label><span>Capacity multiplier</span><input type="number" min="0.1" max="2" step="0.05" :value="point.capacity_multiplier??1" :disabled="running" @change="setWaterCapacityMultiplier('hot',index,$event)"/></label><button class="outline-button" type="button" :disabled="running || scenario.heat_pump_hot_water_cop_performance.length<=2" @click="removeWaterCopPoint('hot',index)">Remove point</button></div><button class="outline-button" type="button" :disabled="running" @click="addWaterCopPoint('hot')">Add point</button></details><details class="field"><summary>Heat pump performance vs chilled-water temperature</summary><p class="muted small">Physical cooling capacity and COP multipliers from measured chilled-water supply temperature at the start of each step. Flat 1.0 preserves prior runs; enter increasing °C points.</p><div v-for="(point,index) in scenario.heat_pump_chilled_water_cop_performance" :key="index" class="performance-point"><label><span>Chilled water · °C</span><input v-model.number="point.water_c" type="number" min="-20" max="100" step="1" :disabled="running" @change="saveScenario"/></label><label><span>COP multiplier</span><input v-model.number="point.cop_multiplier" type="number" min="0.1" max="2" step="0.05" :disabled="running" @change="saveScenario"/></label><label><span>Capacity multiplier</span><input type="number" min="0.1" max="2" step="0.05" :value="point.capacity_multiplier??1" :disabled="running" @change="setWaterCapacityMultiplier('chilled',index,$event)"/></label><button class="outline-button" type="button" :disabled="running || scenario.heat_pump_chilled_water_cop_performance.length<=2" @click="removeWaterCopPoint('chilled',index)">Remove point</button></div><button class="outline-button" type="button" :disabled="running" @click="addWaterCopPoint('chilled')">Add point</button></details><label class="field"><span>Boiler capacity · kW</span><input v-model.number="scenario.boiler_capacity_kw" type="number" min="1" step="5" @change="saveScenario"/></label><label class="field"><span>Chiller capacity · kW</span><input v-model.number="scenario.chiller_capacity_kw" type="number" min="1" step="5" @change="saveScenario"/></label><details class="field"><summary>Equipment outage tests</summary><p class="muted small">Inject raw availability status for the heat pump, boiler, or chiller. The G36 plant CDL graph decides how to dispatch the sources that remain available.</p><p class="muted small">Physical startup delays hold source delivery and running proof after a CDL command. The plant graph decides when proven stages may release water-reset holds.</p><label class="field"><span>Heat pump start delay · seconds</span><input v-model.number="scenario.equipment_startup_seconds.heat_pump" type="number" min="0" step="15" :disabled="running" @change="saveScenario"/></label><label class="field"><span>Boiler start delay · seconds</span><input v-model.number="scenario.equipment_startup_seconds.boiler" type="number" min="0" step="15" :disabled="running" @change="saveScenario"/></label><label class="field"><span>Chiller start delay · seconds</span><input v-model.number="scenario.equipment_startup_seconds.chiller" type="number" min="0" step="15" :disabled="running" @change="saveScenario"/></label><div v-for="(outage,index) in scenario.equipment_outages" :key="index" class="outage-entry"><label><span>Equipment</span><select v-model="outage.equipment" :disabled="running" @change="saveScenario"><option value="heat_pump">Heat pump</option><option value="boiler">Boiler</option><option value="chiller">Chiller</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="outage.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="outage.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeEquipmentOutage(index)">Remove outage</button></div><button class="outline-button" type="button" :disabled="running" @click="addEquipmentOutage">Add outage</button></details><details class="field"><summary>Source failure to start tests</summary><p class="muted small">The equipment stays available but cannot prove running during the test interval. The G36 plant CDL blocks diagnose each source after an editable timeout. A heat-pump fault moves its load to backup sources; each latched source fault inhibits further commands until reset.</p><div v-for="(failure,index) in scenario.equipment_start_failures" :key="index" class="outage-entry"><label><span>Equipment</span><select v-model="failure.equipment" :disabled="running" @change="saveScenario"><option value="heat_pump">Heat pump</option><option value="boiler">Boiler</option><option value="chiller">Chiller</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="failure.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="failure.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeStartFailure(index)">Remove failure</button></div><button class="outline-button" type="button" :disabled="running" @click="addStartFailure">Add failure</button><label class="field"><span>Heat pump operator reset · elapsed hours</span><input v-model.number="scenario.heat_pump_fault_reset_hour" type="number" min="-1" step="0.25" :disabled="running" @change="saveScenario"/></label><label class="field"><span>Boiler operator reset · elapsed hours</span><input v-model.number="scenario.boiler_fault_reset_hour" type="number" min="-1" step="0.25" :disabled="running" @change="saveScenario"/></label><label class="field"><span>Chiller operator reset · elapsed hours</span><input v-model.number="scenario.chiller_fault_reset_hour" type="number" min="-1" step="0.25" :disabled="running" @change="saveScenario"/></label></details><details class="field" v-if="isG36Sample"><summary>Supply fan fault tests</summary><p class="muted small">Inject a fan motor failure, stuck-on motor, or drive speed cap. The physical model changes airflow and fan energy; CDL compares command with measured status and speed.</p><div v-for="(fault,index) in scenario.fan_faults" :key="index" class="outage-entry"><label><span>Fault</span><select v-model="fault.equipment" :disabled="running" @change="saveScenario"><option value="ahu_1_stuck_off">AHU 1 fan fails off</option><option value="ahu_1_stuck_on">AHU 1 fan stuck on</option><option value="ahu_2_stuck_off">AHU 2 fan fails off</option><option value="ahu_2_stuck_on">AHU 2 fan stuck on</option><option value="ahu_1_speed_capped">AHU 1 fan speed capped at 30%</option><option value="ahu_2_speed_capped">AHU 2 fan speed capped at 30%</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="fault.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="fault.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeFanFault(index)">Remove fault</button></div><button class="outline-button" type="button" :disabled="running" @click="addFanFault">Add fan fault</button></details><details class="field" v-if="isG36Sample"><summary>Fan-speed sensor error tests</summary><p class="muted small">Add a bias to the speed reading supplied to CDL. The physical fan, airflow, and electricity remain unchanged; the editable tracking alarm interprets the sensor value. The last overlapping event wins.</p><div v-for="(event,index) in scenario.fan_speed_sensor_events" :key="index" class="outage-entry"><label><span>AHU</span><select v-model.number="event.ahu" :disabled="running" @change="saveScenario"><option :value="1">AHU 1</option><option :value="2">AHU 2</option></select></label><label><span>Sensor bias · fraction</span><input v-model.number="event.bias" type="number" min="-1" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeFanSpeedSensorEvent(index)">Remove event</button></div><button class="outline-button" type="button" :disabled="running" @click="addFanSpeedSensorEvent">Add sensor event</button></details><details class="field" v-if="isG36Sample"><summary>Water pump fault tests</summary><p class="muted small">Stop a physical hot- or chilled-water pump during a scheduled interval. The CDL speed command remains visible; measured water flow and pressure fall, and editable source-proof logic reacts on the next step.</p><div v-for="(fault,index) in scenario.water_pump_faults" :key="index" class="outage-entry"><label><span>Loop</span><select v-model="fault.loop_name" :disabled="running" @change="saveScenario"><option value="hot">Hot water</option><option value="chilled">Chilled water</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="fault.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="fault.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeWaterPumpFault(index)">Remove fault</button></div><button class="outline-button" type="button" :disabled="running" @click="addWaterPumpFault">Add pump fault</button></details><details class="field" v-if="isG36Sample"><summary>Water-flow sensor error tests</summary><p class="muted small">Bias the flow reading sent to CDL without changing physical water delivery. Source proof is calculated by the editable plant blocks. The last overlapping event wins.</p><div v-for="(event,index) in scenario.water_flow_sensor_events" :key="index" class="outage-entry"><label><span>Loop</span><select v-model="event.loop_name" :disabled="running" @change="saveScenario"><option value="hot">Hot water</option><option value="chilled">Chilled water</option></select></label><label><span>Sensor bias · kg/s</span><input v-model.number="event.bias_kg_s" type="number" min="-20" max="20" step="0.1" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.0042" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeWaterFlowSensorEvent(index)">Remove event</button></div><button class="outline-button" type="button" :disabled="running" @click="addWaterFlowSensorEvent">Add sensor event</button></details><details class="field" v-if="isG36Sample"><summary>Outdoor temperature disturbances</summary><p class="muted small">Add a short physical temperature change between 15-minute weather samples. The same temperature feeds the outside-air sensor, envelope, mixed-air path, and equipment performance.</p><div v-for="(event,index) in scenario.outdoor_temperature_events" :key="index" class="outage-entry"><label><span>Change · °C</span><input v-model.number="event.delta_c" type="number" min="-25" max="25" step="0.5" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.0166667" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.0041667" step="0.0166667" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeOutdoorTemperatureEvent(index)">Remove disturbance</button></div><button class="outline-button" type="button" :disabled="running" @click="addOutdoorTemperatureEvent">Add disturbance</button></details><details class="field" v-if="isG36Sample"><summary>AHU damper fault tests</summary><p class="muted small">Hold an outdoor or return damper at a physical position. Measured airflow and mixing respond to the fault; editable CDL checks position tracking.</p><div v-for="(fault,index) in scenario.ahu_damper_faults" :key="index" class="outage-entry"><label><span>AHU</span><select v-model.number="fault.ahu" :disabled="running" @change="saveScenario"><option :value="1">AHU 1</option><option :value="2">AHU 2</option></select></label><label><span>Damper</span><select v-model="fault.damper" :disabled="running" @change="saveScenario"><option value="outdoor">Outdoor air</option><option value="return">Return air</option></select></label><label><span>Stuck position · 0–1</span><input v-model.number="fault.stuck_position" type="number" min="0" max="1" step="0.1" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="fault.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="fault.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeAhuDamperFault(index)">Remove fault</button></div><button class="outline-button" type="button" :disabled="running" @click="addAhuDamperFault">Add AHU damper fault</button></details><details class="field" v-if="isG36Sample"><summary>AHU damper position sensor bias tests</summary><p class="muted small">Offset a measured position without moving the physical damper. Editable CDL decides whether the tracking alarm should fire; air mixing continues to use the physical position.</p><div v-for="(event,index) in scenario.ahu_damper_sensor_events" :key="index" class="outage-entry"><label><span>AHU</span><select v-model.number="event.ahu" :disabled="running" @change="saveScenario"><option :value="1">AHU 1</option><option :value="2">AHU 2</option></select></label><label><span>Damper sensor</span><select v-model="event.damper" :disabled="running" @change="saveScenario"><option value="outdoor">Outdoor air</option><option value="return">Return air</option></select></label><label><span>Bias · fraction</span><input v-model.number="event.bias" type="number" min="-1" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeAhuDamperSensorEvent(index)">Remove bias</button></div><button class="outline-button" type="button" :disabled="running" @click="addAhuDamperSensorEvent">Add sensor bias</button></details><details class="field" v-if="isG36Sample"><summary>AHU mixed-air temperature sensor bias tests</summary><p class="muted small">Offset the mixed-air reading supplied to the editable CDL economizer and freeze logic. Physical mixing starts from the damper openings and source temperatures; CDL may then change commands in response to the false reading.</p><div v-for="(event,index) in scenario.ahu_mixed_air_sensor_events" :key="index" class="outage-entry"><label><span>AHU</span><select v-model.number="event.ahu" :disabled="running" @change="saveScenario"><option :value="1">AHU 1</option><option :value="2">AHU 2</option></select></label><label><span>Bias · °C</span><input v-model.number="event.bias_c" type="number" min="-25" max="25" step="0.5" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeAhuMixedAirSensorEvent(index)">Remove bias</button></div><button class="outline-button" type="button" :disabled="running" @click="addAhuMixedAirSensorEvent">Add sensor bias</button></details><details class="field" v-if="isG36Sample"><summary>VAV actuator fault tests</summary><p class="muted small">Inject a physical damper stuck open or a reheat valve seat leak. The fault changes actual airflow or heat transfer; editable CDL blocks decide the alarm level from measured points.</p><div v-for="(fault,index) in scenario.terminal_faults" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="fault.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>Fault</span><select v-model="fault.kind" :disabled="running" @change="saveScenario"><option value="damper_stuck_open">Damper stuck open</option><option value="reheat_valve_leak">Reheat valve leaks while closed</option></select></label><label v-if="fault.kind==='reheat_valve_leak'"><span>Leak opening · fraction</span><input :value="fault.leak_fraction ?? 0.3" type="number" min="0" max="1" step="0.05" :disabled="running" @change="setTerminalLeakFraction(index,$event)"/></label><label><span>Start · elapsed hours</span><input v-model.number="fault.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="fault.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeTerminalFault(index)">Remove fault</button></div><button class="outline-button" type="button" :disabled="running" @click="addTerminalFault">Add VAV fault</button></details><details class="field" v-if="isG36Sample"><summary>VAV reheat position sensor bias tests</summary><p class="muted small">Offset a reheat valve position reading without moving its stem or changing leakage. Editable CDL uses the reading in the valve leak diagnostic.</p><div v-for="(event,index) in scenario.terminal_valve_sensor_events" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="event.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>Bias · fraction</span><input v-model.number="event.bias" type="number" min="-1" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeTerminalValveSensorEvent(index)">Remove bias</button></div><button class="outline-button" type="button" :disabled="running" @click="addTerminalValveSensorEvent">Add sensor bias</button></details><details class="field"><summary>Freeze-stat test inputs</summary><p class="muted small">Inject a raw AHU freeze-stat trip and an operator reset pulse. The editable CDL graph decides the shutdown and recovery.</p><label class="field"><span>Test AHU</span><select v-model.number="scenario.freeze_test_ahu" :disabled="running" @change="saveScenario"><option :value="0">Off</option><option :value="1">AHU 1</option><option :value="2">AHU 2</option></select></label><label class="field"><span>Trip starts · elapsed hours</span><input v-model.number="scenario.freeze_test_start_hour" type="number" min="-1" step="0.25" @change="saveScenario"/></label><label class="field"><span>Trip duration · hours</span><input v-model.number="scenario.freeze_test_duration_hours" type="number" min="0" step="0.25" @change="saveScenario"/></label><label class="field"><span>Operator reset · elapsed hours</span><input v-model.number="scenario.freeze_test_reset_hour" type="number" min="-1" step="0.25" @change="saveScenario"/></label></details><details class="field" v-if="isG36Sample"><summary>Window contact tests</summary><p class="muted small">Inject raw window contacts. CDL excludes an open zone from ordinary group decisions and turns off terminal conditioning while retaining freeze protection.</p><div v-for="(opening,index) in scenario.window_openings" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="opening.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="opening.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="opening.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeWindowOpening(index)">Remove opening</button></div><button class="outline-button" type="button" :disabled="running" @click="addWindowOpening">Add opening</button></details><details class="field"><summary>Physical occupant-count events</summary><p class="muted small">Change how many people are actually in a zone. This sets its modeled CO₂ and sensible-heat sources; the count sensor follows actual people unless a separate sensor event overrides it. The binary sensor follows actual presence unless its own sensor event overrides it. Last overlapping event wins.</p><div v-for="(event,index) in scenario.occupant_count_events" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="event.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>People present</span><input v-model.number="event.count" type="number" min="0" max="1000" step="1" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.0042" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeOccupantCountEvent(index)">Remove event</button></div><button class="outline-button" type="button" :disabled="running" @click="addOccupantCountEvent">Add people event</button></details><details class="field"><summary>Occupancy-count sensor events</summary><p class="muted small">Override only the raw count reported to CDL. Actual people, CO₂ generation, sensible heat, and the binary presence sensor remain separate. Last overlapping event wins.</p><div v-for="(event,index) in scenario.occupancy_count_sensor_events" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="event.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>Reported people</span><input v-model.number="event.count" type="number" min="0" max="1000" step="1" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.0042" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeOccupancyCountSensorEvent(index)">Remove event</button></div><button class="outline-button" type="button" :disabled="running" @click="addOccupancyCountSensorEvent">Add sensor event</button></details><details class="field"><summary>Occupancy sensor events</summary><p class="muted small">Override the synthetic raw presence sensor for a zone during an interval. The event changes only the sensor reading; physical occupants and their CO₂ source stay separate. Editable CDL determines confirmation, operating mode, ventilation, and alarms. The last overlapping event wins.</p><div v-for="(event,index) in scenario.occupancy_sensor_events" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="event.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>Sensor reading</span><select v-model="event.present" :disabled="running" @change="saveScenario"><option :value="true">Present</option><option :value="false">Absent</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.0042" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeOccupancySensorEvent(index)">Remove event</button></div><button class="outline-button" type="button" :disabled="running" @click="addOccupancySensorEvent">Add sensor event</button></details><details class="field" v-if="isG36Sample"><summary>Local occupancy override tests</summary><p class="muted small">Inject raw local override button contacts. Editable CDL holds each press for two hours by default and makes the affected zone and AHU group occupied.</p><div v-for="(overrideEvent,index) in scenario.occupancy_overrides" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="overrideEvent.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="overrideEvent.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="overrideEvent.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeOccupancyOverride(index)">Remove override</button></div><button class="outline-button" type="button" :disabled="running" @click="addOccupancyOverride">Add override</button></details><details class="field" v-if="isG36Sample"><summary>VAV alarm acknowledgement tests</summary><p class="muted small">Schedule a raw operator button press for a zone. Editable CDL acknowledges each active VAV alarm in that zone independently and clears its status when that alarm clears. Alarm levels stay visible.</p><div v-for="(event,index) in scenario.alarm_acknowledgements" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="event.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>Press starts · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Press duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.0042" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeAlarmAcknowledgement(index)">Remove press</button></div><button class="outline-button" type="button" :disabled="running" @click="addAlarmAcknowledgement">Add press</button></details><details class="field" v-if="isG36Sample"><summary>AHU alarm acknowledgement tests</summary><p class="muted small">Schedule a raw operator press for an AHU. Editable CDL acknowledges each currently active AHU alarm separately and clears its status when that alarm clears. Alarm levels remain visible.</p><div v-for="(event,index) in scenario.ahu_alarm_acknowledgements" :key="index" class="outage-entry"><label><span>AHU</span><select v-model.number="event.ahu" :disabled="running" @change="saveScenario"><option :value="1">AHU 1</option><option :value="2">AHU 2</option></select></label><label><span>Press starts · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Press duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.0042" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeAhuAlarmAcknowledgement(index)">Remove press</button></div><button class="outline-button" type="button" :disabled="running" @click="addAhuAlarmAcknowledgement">Add press</button></details><details class="field" v-if="isG36Sample"><summary>Plant alarm acknowledgement tests</summary><p class="muted small">Schedule a raw plant operator press. Editable CDL acknowledges active source-start, pump-start, and water-flow faults independently. Each fault remains visible until its own reset or physical condition clears.</p><div v-for="(event,index) in scenario.plant_alarm_acknowledgements" :key="index" class="outage-entry"><label><span>Press starts · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Press duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.0042" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removePlantAlarmAcknowledgement(index)">Remove press</button></div><button class="outline-button" type="button" :disabled="running" @click="addPlantAlarmAcknowledgement">Add press</button></details><div class="assumption-note">The conventional graph commands a 20% outdoor-air share. The G36-style graph uses editable CDL minimum-air and economizer controls with measured airflow feedback. Fan capacity determines delivered airflow and normalized duct pressure. Graph-commanded water pumps and source output charge mixed water volumes; flow and measured supply temperature limit coil and reheat delivery.</div></aside>
      <div class="sim-content"><div class="sim-status"><div><span class="status-dot">● {{ running ? 'Running in your browser' : results.length ? 'Custom run complete' : 'Ready for a new run' }}</span><strong>{{ running ? `${Math.round(progress/totalSteps*100)}% complete` : '2025 · January → December' }}</strong></div><div v-if="running" class="progress-track"><i :style="{width:`${progress/totalSteps*100}%`} "/></div></div><div v-if="simError" class="error-banner">{{ simError }}</div>
      <div class="chart-card signal-card">
        <div class="card-heading"><div><span class="eyebrow">CONTROL SIGNAL EXPLORER</span><h2>Compare process variables</h2></div><span class="muted small">{{ signalOptions.length }} selectable signals</span></div>
        <div class="signal-picker"><label class="field"><span>Find a variable or connection</span><input v-model="signalSearch" type="search" placeholder="Search block, port, Brick class, or unit…" @input="candidateSignal=''"/></label><label class="field"><span>Signal</span><select v-model="candidateSignal" :disabled="running || !isBuildingGraph"><option value="">Choose a signal…</option><option v-for="signal in filteredSignals" :key="signal.id" :value="signal.id">{{ signal.kind==='physical' ? 'Physical · ' : signal.kind==='connection' ? 'Connection · ' : signal.kind==='input' ? 'Input · ' : 'Output · ' }}{{ signal.label }} ({{ signal.unit }}){{ signal.brickClass ? ` · brick:${signal.brickClass}` : '' }}</option></select></label><button class="outline-button signal-add" type="button" :disabled="!candidateSignal || running" @click="addSignal">Add signal</button></div>
        <div v-if="selectedSignalDetails.length" class="signal-selections"><div v-for="signal in selectedSignalDetails" :key="signal.id" class="signal-chip"><span><b>{{ signal.unit }}</b>{{ signal.label }}<small v-if="signal.brickClass"> · brick:{{ signal.brickClass }}</small></span><button type="button" :aria-label="`Remove ${signal.label}`" :disabled="running" @click="removeSignal(signal.id)">×</button></div></div>
        <p v-if="!selectedSignalDetails.length" class="muted small">Add several graph or physical signals. Each unit type gets its own plot with a labeled y-axis.</p>
        <div v-else-if="needsSignalRun" class="signal-run"><span>{{ results.length ? 'New graph signals need a fresh run.' : 'Run the current model to record your selected signals.' }}</span><button class="solid-button" type="button" :disabled="running || !isBuildingGraph || errors.length>0" @click="runYear">{{ running ? 'Running…' : 'Run selected signals' }}</button></div>
      </div>
      <div class="time-controls timeline-toolbar"><label class="field"><span>From · UTC</span><input type="datetime-local" min="2025-01-01T00:00" max="2025-12-31T23:45" step="900" :value="timeValue(timeStart)" @change="setTimeRange(timeIndex(($event.target as HTMLInputElement).value),timeEnd)"/></label><label class="field"><span>To · UTC</span><input type="datetime-local" min="2025-01-01T00:00" max="2025-12-31T23:45" step="900" :value="timeValue(timeEnd)" @change="setTimeRange(timeStart,timeIndex(($event.target as HTMLInputElement).value))"/></label><div class="time-zoom"><button class="outline-button" type="button" aria-label="Pan time earlier" :disabled="timeStart===0" @click="shiftTime(-1)">←</button><button class="outline-button" type="button" aria-label="Zoom time out" @click="zoomTime(2)">−</button><button class="outline-button" type="button" aria-label="Reset time range" @click="resetTimeRange">Full year</button><button class="outline-button" type="button" aria-label="Zoom time in" @click="zoomTime(0.5)">+</button><button class="outline-button" type="button" aria-label="Pan time later" :disabled="timeEnd===35039" @click="shiftTime(1)">→</button><button class="outline-button" :class="{active:panMode}" type="button" :aria-pressed="panMode" :disabled="timeStart===0&&timeEnd===35039" @click="panMode=!panMode">Pan charts</button></div></div>
      <SignalChart v-for="group in plotGroups" :key="group.unit" :rows="results" :version="rowVersion" :pan-mode="panMode" :signals="group.signals" :unit="group.unit" :start="timeStart" :end="timeEnd" @range="setTimeRange" @pan="panTimeTo"/>
      <RunComparison :runs="runHistory" :current-run-id="currentRunId" @rename="renameRun"/>

      <div v-if="displaySummary" class="current-run-summary">
        <span class="eyebrow">CURRENT RUN SUMMARY · {{ running ? 'IN PROGRESS' : 'COMPLETE' }} · {{ currentRun?.label ?? project?.name }}<template v-if="currentRun"> · {{ new Date(currentRun.completedAt).toLocaleString() }}</template></span>
        <p class="muted small">These cumulative values come from the current Rust simulation. Match both the run label and the metric heading to a bar above. Present-zone comfort counts only physical occupants during scheduled occupancy; all-zone comfort also includes absent zones and off-hours.</p>
        <div class="metric-grid">
          <article v-for="metric in summaryMetrics" :key="metric.key"><span>{{ metric.label.toUpperCase() }} · {{ metric.unit }}</span><strong>{{ formatSummaryValue(displaySummary[metric.key], metric.key) }}</strong><small>{{ metric.detail }}</small><button v-if="currentRun" class="summary-match" type="button" @click="scrollToComparison(metric.key)">View this run’s {{ metric.label.toLowerCase() }} bar ↑</button></article>
        </div>
        <div class="latest-zone-temperature"><span>LATEST AVG ZONE TEMP</span><strong>{{ latestRow?.avg_zone_c.toFixed(1) ?? '—' }}°C</strong><small>Snapshot at the latest recorded time step; not a cumulative total</small></div>
      </div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">TEMPERATURE RESPONSE</span><h2>Zone and outdoor temperature</h2></div><span class="muted small">{{ results.length ? 'CURRENT RUN' : 'RUN TO VIEW' }}</span></div><TrendChart v-if="results.length" :rows="results" :version="rowVersion" :start="timeStart" :end="timeEnd" field-a="avg_zone_c" field-b="outdoor_c" unit="°C" label-a="Average zone" label-b="Outdoor air" @pan="panTimeTo"/></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">AIR DELIVERY</span><h2>VAV request and fan-delivered airflow</h2></div><span class="muted small">NORMALIZED FLOW UNITS</span></div><TrendChart v-if="results.length" :rows="results" :version="rowVersion" :start="timeStart" :end="timeEnd" field-a="requested_airflow" field-b="delivered_airflow" unit="normalized airflow" :zero-based="true" label-a="VAV request" label-b="Fan delivered" @pan="panTimeTo"/></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">COIL RESPONSE</span><h2>AHU 1 supply air</h2></div><span class="muted small">TARGET / ACTUAL</span></div><TrendChart v-if="results.length" :rows="results" :version="rowVersion" :start="timeStart" :end="timeEnd" field-a="ahu_1_supply_setpoint_c" field-b="ahu_1_supply_actual_c" unit="°C" label-a="Target · °C" label-b="Actual · °C" @pan="panTimeTo"/></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">ENERGY PROFILE</span><h2>Electricity and boiler fuel</h2></div><button v-if="results.length" class="text-button" @click="exportResults">Download CSV ↓</button></div><TrendChart v-if="results.length" :rows="results" :version="rowVersion" :start="timeStart" :end="timeEnd" field-a="electric_kw" field-b="gas_kw" unit="kW" :zero-based="true" label-a="Electric · kW" label-b="Gas · kW" @pan="panTimeTo"/></div><div class="zone-card"><div class="card-heading"><div><span class="eyebrow">ZONE SNAPSHOT</span><h2>Latest temperatures</h2></div><span class="muted small">AHU 1 · Z1–5 / AHU 2 · Z6–10</span></div><div class="zone-grid"><div v-for="(temp,i) in latestRow?.zone_c ?? []" :key="i"><span>ZONE {{ String(i+1).padStart(2,'0') }}</span><strong>{{ temp.toFixed(1) }}°C</strong></div></div></div></div></div>
    </main>
    <footer class="site-footer"><span>ACE IoT · CDL Explorer</span><span>Simplified educational simulation · Runs locally in your browser</span><a href="https://github.com/ACE-IoT-Solutions/python-cdl" target="_blank" rel="noreferrer">Python CDL reference ↗</a></footer>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, shallowRef, triggerRef } from 'vue'
import PrimerPage from './components/PrimerPage.vue'
import GlossaryPage from './components/GlossaryPage.vue'
import GraphEditor from './components/GraphEditor.vue'
import TrendChart from './components/TrendChart.vue'
import SignalChart from './components/SignalChart.vue'
import RunComparison from './components/RunComparison.vue'
import PollutionScenario from './components/PollutionScenario.vue'
import HeatPumpJointMapEditor from './components/HeatPumpJointMapEditor.vue'
import { formatSummaryValue, summaryMetrics } from './lib/formatSummary'
import AceButton from './vendor/ace/AceButton.vue'
import { defaultScenario, edgeEndpoints, effectiveStepForGraph, layoutFor, type Block, type Project, type Row, type Scenario, type Summary } from './lib/types'
import { exportCxf, importCxf, type CxfDocument } from './lib/cxf'
import { exportBrickModel, migrateBrickModel, validateBrickModel } from './lib/brickModel'
import { plainWorkerData, scenarioForWorker } from './lib/workerPayload'
import { readRunHistory, saveRunHistory, type SimulationRun } from './lib/runHistory'
import { expandLegacyVavLeaves, hasLegacyVavLeaf } from './lib/legacyVav'
import { storeMigrationBackup, storeProject } from './lib/projectStorage'
import BuildingSpaces from './components/BuildingSpaces.vue'
import { graphSignals, physicalSignalsForGraph, type GraphSignal } from './lib/signals'
import { migrateG36Timing, migrateG36TerminalFeedback, migrateG36AhuFeedback, migrateG36PressureTrim, migrateG36PressureTiming, migrateG36PressureRequestSampler, migrateG36Economizer, migrateG36VacancyDelay, migrateG36FreezeHoldTiming } from './lib/g36Migration'
import { migrateG36PollutionMode, needsG36PollutionModeMigration } from './lib/g36PollutionModeMigration'
import { migrateG36EnthalpyHighLimit, needsG36EnthalpyHighLimitMigration } from './lib/g36EnthalpyHighLimitMigration'
import { migrateG36DifferentialEnthalpy, needsG36DifferentialEnthalpyMigration } from './lib/g36DifferentialEnthalpyMigration'
import { migrateOutdoorAirCommand, migrateCoilControl } from './lib/airCommandMigration'
import { migrateWaterLoopControl } from './lib/waterLoopMigration'
import { migrateConventionalPumpSizing, needsConventionalPumpSizingMigration } from './lib/conventionalPumpSizingMigration'
import { migrateG36WaterReset } from './lib/g36WaterResetMigration'
import { migrateG36HotWaterReset } from './lib/g36HotWaterResetMigration'
import { migrateG36TerminalHotRequestLevels } from './lib/g36TerminalHotRequestLevelsMigration'
import { migrateG36AhuPlantRequests } from './lib/g36AhuPlantRequestsMigration'
import { migrateG36SupplyAirReset } from './lib/g36SupplyAirResetMigration'
import { migrateG36TerminalDischarge } from './lib/g36TerminalDischargeMigration'
import { migrateG36DamperFeedback } from './lib/g36DamperFeedbackMigration'
import { migrateG36DamperPiReset } from './lib/g36DamperPiResetMigration'
import { migrateG36DamperModeGate, needsG36DamperModeGateMigration } from './lib/g36DamperModeGateMigration'
import { migrateHeatPumpModeThreshold, needsHeatPumpModeThresholdMigration } from './lib/heatPumpModeThresholdMigration'
import { migrateG36FlowOverride, needsG36FlowOverrideMigration } from './lib/g36FlowOverrideMigration'
import { migrateG36DischargeMinimum, needsG36DischargeMinimumMigration } from './lib/g36DischargeMinimumMigration'
import { migrateG36NormalFlow, needsG36NormalFlowMigration } from './lib/g36NormalFlowMigration'
import { migrateG36DischargeSetpoint, needsG36DischargeSetpointMigration } from './lib/g36DischargeSetpointMigration'
import { migrateG36ValvePiReset, needsG36ValvePiResetMigration } from './lib/g36ValvePiResetMigration'
import { migrateG36FreezeStageOne } from './lib/g36FreezeStageOneMigration'
import { migrateG36EconomizerEnable } from './lib/g36EconomizerEnableMigration'
import { migrateG36OutdoorAirLimits } from './lib/g36OutdoorAirLimitsMigration'
import { migrateG36ReturnDamper } from './lib/g36ReturnDamperMigration'
import { migrateG36CommonDamperMapping } from './lib/g36CommonDamperMappingMigration'
import { migrateG36Ventilation } from './lib/g36VentilationMigration'
import { migrateG36VentilationDesign } from './lib/g36VentilationDesignMigration'
import { migrateG36VentilationEffectiveness, needsG36VentilationEffectivenessMigration } from './lib/g36VentilationEffectivenessMigration'
import { migrateG36MinimumOutdoorAirPi, needsG36MinimumOutdoorAirPiMigration } from './lib/g36MinimumOutdoorAirPiMigration'
import { migrateG36WaterSourceProof } from './lib/g36WaterSourceProofMigration'
import { migrateG36WaterPumpStatus } from './lib/g36WaterPumpStatusMigration'
import { migrateG36WaterPumpStartFault } from './lib/g36WaterPumpStartFaultMigration'
import { migrateG36WaterFlowFault } from './lib/g36WaterFlowFaultMigration'
import { migrateG36WaterPressure } from './lib/g36WaterPressureMigration'
import { migrateG36ChilledWaterMapping } from './lib/g36ChilledWaterMappingMigration'
import { migrateConventionalSourceEnable } from './lib/conventionalSourceEnableMigration'
import { migrateConventionalDamperCommands, needsConventionalDamperCommandsMigration } from './lib/conventionalDamperCommandsMigration'
import { migrateConventionalStatus, needsConventionalStatusMigration } from './lib/conventionalStatusMigration'
import { migrateConventionalMode, needsConventionalModeMigration } from './lib/conventionalModeMigration'
import { migrateWaterDispatch } from './lib/waterDispatchMigration'
import { migrateG36WaterStageHold } from './lib/g36WaterStageHoldMigration'
import { migrateG36HybridDispatch } from './lib/g36HybridDispatchMigration'
import { migrateG36WaterCapacity, needsG36WaterCapacityMigration } from './lib/g36WaterCapacityMigration'
import { migrateG36CapacityReallocation, needsG36CapacityReallocation } from './lib/g36CapacityReallocationMigration'
import { migrateSourceStartRequests, needsSourceStartMigration } from './lib/sourceStartMigration'
import { migrateG36SourceMinRun, needsG36SourceMinRunMigration } from './lib/g36SourceMinRunMigration'
import { migrateG36ForcedStop, needsG36ForcedStopMigration } from './lib/g36ForcedStopMigration'
import { migrateG36BackupStage, needsG36BackupStageMigration } from './lib/g36BackupStageMigration'
import { migrateG36BackupFailover, needsG36BackupFailoverMigration } from './lib/g36BackupFailoverMigration'
import { migrateG36EquipmentAvailability } from './lib/g36EquipmentAvailabilityMigration'
import { migrateG36StageFeedback } from './lib/g36StageFeedbackMigration'
import { migrateG36StartFault } from './lib/g36StartFaultMigration'
import { migrateG36BackupStartFault } from './lib/g36BackupStartFaultMigration'
import { migrateG36ZoneModes } from './lib/g36ZoneModesMigration'
import { migrateG36ZoneGroups } from './lib/g36ZoneGroupMigration'
import { migrateG36OptimalStart, needsG36OptimalStartMigration } from './lib/g36OptimalStartMigration'
import { migrateG36ZoneStatus, needsG36ZoneStatusMigration } from './lib/g36ZoneStatusMigration'
import { migrateG36OperationMode, needsG36OperationModeMigration } from './lib/g36OperationModeMigration'
import { migrateG36GroupStatus, needsG36GroupStatusMigration } from './lib/g36GroupStatusMigration'
import { migrateG36ObcOptimalStart, needsG36ObcOptimalStartMigration } from './lib/g36ObcOptimalStartMigration'
import { migrateG36DailyRecovery, needsG36DailyRecoveryMigration } from './lib/g36DailyRecoveryMigration'
import { migrateG36Co2, needsG36Co2Migration } from './lib/g36Co2Migration'
import { migrateG36WindowMinimum, needsG36WindowMinimumMigration } from './lib/g36WindowMinimumMigration'
import { migrateG36VentilationStandby, needsG36VentilationStandbyMigration } from './lib/g36VentilationStandbyMigration'
import { migrateG36GroupUnoccupied } from './lib/g36GroupUnoccupiedMigration'
import { migrateG36Windows } from './lib/g36WindowMigration'
import { migrateG36Overrides } from './lib/g36OverrideMigration'
import { migrateG36TimedOverrides, needsG36TimedOverrideMigration } from './lib/g36TimedOverrideMigration'
import { migrateG36TerminalAirflowAlarms, needsG36TerminalAirflowAlarmMigration, migrateG36LowFlowHysteresis, needsG36LowFlowHysteresisMigration, migrateG36LowFlowInit, needsG36LowFlowInitMigration } from './lib/g36TerminalAirflowAlarmMigration'
import { migrateG36HighDischargeAlarm, needsG36HighDischargeAlarmMigration } from './lib/g36HighDischargeAlarmMigration'
import { migrateG36AlarmAcknowledgement, needsG36AlarmAcknowledgementMigration } from './lib/g36AlarmAcknowledgementMigration'
import { migrateG36AhuAlarmAcknowledgement, needsG36AhuAlarmAcknowledgementMigration } from './lib/g36AhuAlarmAcknowledgementMigration'
import { migrateG36PlantAlarmAcknowledgement, needsG36PlantAlarmAcknowledgementMigration } from './lib/g36PlantAlarmAcknowledgementMigration'
import { migrateG36PressureRequestLevels, migrateG36PressureRequestHysteresis, migrateG36PressureDamperLatch, needsG36PressureRequestLevelsMigration, needsG36PressureRequestHysteresisMigration, needsG36PressureDamperLatchMigration } from './lib/g36PressureRequestLevelsMigration'
import { migrateG36SatRequestLevels, needsG36SatRequestLevelsMigration } from './lib/g36SatRequestLevelsMigration'
import { migrateG36SatThresholds, needsG36SatThresholdMigration } from './lib/g36SatThresholdMigration'
import { migrateG36SatDynamicSuppression, needsG36SatDynamicSuppressionMigration } from './lib/g36SatDynamicSuppressionMigration'
import { migrateG36SatCoolingLoop, needsG36SatCoolingLoopMigration } from './lib/g36SatCoolingLoopMigration'
import { migrateG36SatSampledSetpoint, needsG36SatSampledSetpointMigration } from './lib/g36SatSampledSetpointMigration'
import { migrateG36SupplyTemperatureModes, needsG36SupplyTemperatureModeMigration } from './lib/g36SupplyTemperatureModeMigration'
import { migrateG36ObcSupplyAirReset, needsG36ObcSupplyAirResetMigration } from './lib/g36ObcSupplyAirResetMigration'
import { migrateG36SupplyFanAlarms, needsG36SupplyFanAlarmMigration } from './lib/g36SupplyFanAlarmMigration'
import { migrateG36AirflowSensorAlarms, needsG36AirflowSensorAlarmMigration } from './lib/g36AirflowSensorAlarmMigration'
import { migrateG36LowDischargeAlarms, needsG36LowDischargeAlarmMigration } from './lib/g36LowDischargeAlarmMigration'
import { migrateG36TerminalLeakAlarms, migrateG36TerminalLeakTiming, migrateG36TerminalLeakFlowHysteresis, needsG36TerminalLeakAlarmMigration, needsG36TerminalLeakTimingMigration, needsG36TerminalLeakFlowHysteresisMigration } from './lib/g36TerminalLeakAlarmMigration'
import { migrateG36FanSpeedFeedback, needsG36FanSpeedFeedbackMigration } from './lib/g36FanSpeedFeedbackMigration'
import { migrateG36AhuDamperFeedback, needsG36AhuDamperFeedbackMigration } from './lib/g36AhuDamperFeedbackMigration'
import { migrateG36EconomizerHold, needsG36EconomizerHoldMigration } from './lib/g36EconomizerHoldMigration'
import { migrateG36EconomizerDisableTransition, needsG36EconomizerDisableTransitionMigration } from './lib/g36EconomizerDisableTransitionMigration'
import { migrateG36FixedDryBulb, needsG36FixedDryBulbMigration } from './lib/g36FixedDryBulbMigration'
import { migrateG36CombinedDryBulb, needsG36CombinedDryBulbMigration } from './lib/g36CombinedDryBulbMigration'
import { migrateG36OptionalLowLimit, needsG36OptionalLowLimitMigration } from './lib/g36OptionalLowLimitMigration'
import { migrateG36OptionalDemandGate, needsG36OptionalDemandGateMigration } from './lib/g36OptionalDemandGateMigration'
import { migrateG36CoolingStartup } from './lib/g36CoolingStartupMigration'
import { migrateWaterEnergyControl } from './lib/waterEnergyMigration'
import { ArrowRows } from './lib/frames'
import RuntimeWorker from './workers/runtime.worker?worker'

type Tab = 'primer' | 'editor' | 'simulation' | 'glossary'
function routeFromHash():Tab {const route=location.hash.slice(1);return route==='editor'||route==='simulation'||route==='glossary'?route:'primer'}
const tab=ref<Tab>(routeFromHash())
const project=shallowRef<Project|null>(null)
const errors=ref<string[]>([])
const storageWarning=ref('')
const recoverableProject=ref<string|null>(null)
const worker=new RuntimeWorker()
const exampleChoice=ref('')
const fileInput=ref<HTMLInputElement|null>(null)
const examples=[{file:'four_pipe_building.json',label:'Four-pipe building · conventional controls'},{file:'four_pipe_g36.json',label:'Four-pipe building · G36-style controls'},{file:'obc_zone_status.json',label:'OBC G36 zone status · reference composite'},{file:'obc_operation_mode.json',label:'OBC G36 operation mode · reference composite'},{file:'obc_group_status.json',label:'OBC G36 group status · reference composite'},{file:'obc_optimal_start_calculation.json',label:'OBC optimal start calculation · reference composite'},{file:'vav_composite_template.json',label:'VAV composite · editable blocks'},{file:'p_controller_limiter.json',label:'P controller with limiter'},{file:'room_control_system.json',label:'Dual-mode room controller'},{file:'simple_temp_controller.json',label:'Basic temperature controller'},...['east','north','south','west','corridor'].map(x=>({file:`${x}_controller.json`,label:`VAV composite · ${x}`}))]
const testInputs=reactive<Record<string,number|boolean>>({})
const testResult=ref('')
const runtimeError=ref('')
const scenario=reactive<Scenario>(structuredClone(defaultScenario))
const effectiveStepSeconds=computed(()=>project.value?effectiveStepForGraph(project.value.graph,scenario.step_seconds):scenario.step_seconds)
const totalSteps=computed(()=>35_040*900/effectiveStepSeconds.value)
const stepLabel=computed(()=>effectiveStepSeconds.value>=60?`${effectiveStepSeconds.value/60}-minute`:`${effectiveStepSeconds.value}-second`)
const runHistory=ref<SimulationRun[]>([])
const currentRunId=ref<string|null>(null)
const currentRun=computed(()=>runHistory.value.find(run=>run.id===currentRunId.value))
// A completed tile and its comparison bar must read the exact same saved
// snapshot. During a run, show the latest total sent by the Rust worker.
const displaySummary=computed(()=>currentRun.value?.summary ?? resultSummary.value)
const results=shallowRef(new ArrowRows([]))
const rowVersion=ref(0)
const resultSummary=ref<Summary|null>(null)
const running=ref(false),progress=ref(0),simError=ref('')
const panMode=ref(false)
const signalSearch=ref(''),candidateSignal=ref(''),selectedSignals=ref<string[]>([]),activeTraceIds=ref<string[]>([]),timeStart=ref(0),timeEnd=ref(35039)
let lastResultRefresh=0
let jobId=0
let activeRunId:number|null=null
let activeRunContext:{id:number;program:SimulationRun['program'];label:string;startedAt:number;requestedStepSeconds:number;effectiveStepSeconds:number}|null=null
const isBuildingGraph=computed(()=>{const blocks=project.value?.graph.blocks??[];return blocks.filter(b=>b.block_type==='VAVBoxController'&&b.blocks?.length).length===10&&blocks.filter(b=>b.block_type==='AHUController').length===2&&blocks.some(b=>b.block_type==='PlantDispatch')})
const isG36Sample=computed(()=>project.value?.graph.name==='FourPipeG36Controls'||project.value?.graph.blocks?.some(block=>block.block_type==='ZoneGroupModeController'))
const latestRow=computed(()=>{rowVersion.value;return results.value.at(-1)})
const signalOptions=computed(()=>project.value?[...physicalSignalsForGraph(project.value.graph),...graphSignals(project.value.graph)]:[])
const filteredSignals=computed(()=>{const query=signalSearch.value.trim().toLowerCase();return signalOptions.value.filter(signal=>!selectedSignals.value.includes(signal.id)&&(!query||`${signal.label} ${signal.unit} ${signal.brickClass??''}`.toLowerCase().includes(query)))})
const selectedSignalDetails=computed(()=>selectedSignals.value.flatMap(id=>{const signal=signalOptions.value.find(item=>item.id===id);return signal?[signal]:[]}))
const needsSignalRun=computed(()=>selectedSignals.value.length>0&&(!results.value.length||selectedSignalDetails.value.some(signal=>signal.kind!=='physical'&&!activeTraceIds.value.includes(signal.id))))
const plotGroups=computed(()=>{if(!results.value.length)return [];const groups=new Map<string,GraphSignal[]>();for(const signal of selectedSignalDetails.value){if(signal.kind!=='physical'&&!activeTraceIds.value.includes(signal.id))continue;const series=groups.get(signal.unit)??[];series.push(signal);groups.set(signal.unit,series)}return [...groups].map(([unit,signals])=>({unit,signals}))})
const number=(n:number)=>Math.round(n).toLocaleString()
const asset=(name:string)=>new URL(`examples/${name}`,document.baseURI).toString()
async function fetchJson<T>(name:string):Promise<T>{const response=await fetch(asset(name));if(!response.ok)throw new Error(`Could not load ${name}`);return response.json() as Promise<T>}
async function expandLegacyVavs(graph:Block):Promise<Block>{return hasLegacyVavLeaf(graph)?expandLegacyVavLeaves(graph,await fetchJson<Block>('vav_composite_template.json')):graph}
function syncInputs(graph:Block){for(const key of Object.keys(testInputs))delete testInputs[key];for(const input of graph.inputs??[])testInputs[input.name]=input.type==='Boolean'?false:input.name.includes('enthalpy_kj_kg')?50:input.name.toLowerCase().includes('max')?20:input.name.toLowerCase().includes('temp')?22:1;testResult.value=''}
function updateProject(next:Project){project.value=next;results.value=new ArrowRows([]);resultSummary.value=null;currentRunId.value=null;activeTraceIds.value=[];const saved=storeProject(localStorage,next);if(saved)recoverableProject.value=null;storageWarning.value=saved?'':'Browser storage is full. Export your project to preserve these edits.';validateGraph(next.graph)}
async function fillExampleMetadata(saved:Project):Promise<Project>{
  const example=saved.graph.name==='FourPipeBuildingControls'?'four_pipe_building.json':saved.graph.name==='FourPipeG36Controls'?'four_pipe_g36.json':null
  if(!example)return saved
  try{
    const reference=await fetchJson<Block>(example)
    const next=migrateBrickModel(structuredClone(saved),reference)
    let changed=next!==saved
    function visit(block:Block,source:Block){
      if(block.notes===undefined&&source.notes){block.notes=source.notes;changed=true}
      if(saved.graph.name==='FourPipeG36Controls'&&block.notes){
        const refreshed=block.notes.replaceAll('heating-production proof','heating-source proof').replaceAll('physical hot-water production proof','physical hot-water source-operation proof')
        if(refreshed!==block.notes){block.notes=refreshed;changed=true}
      }
      for(const direction of ['inputs','outputs'] as const){
        for(const port of block[direction]??[]){
          const reference=source[direction]?.find(item=>item.name===port.name)
          if(!reference)continue
          if(port.name==='hot_water_plant_proven'&&port.description==='Physical heating production during the previous simulation step; false when only cooling runs'&&reference.description){port.description=reference.description;changed=true}
          for(const key of ['label','unit','brick_class','description'] as const){
            if(!port[key]&&reference[key]){port[key]=reference[key];changed=true}
          }
        }
      }
      for(const child of block.blocks??[]){const match=source.blocks?.find(item=>item.name===child.name&&item.block_type===child.block_type);if(match)visit(child,match)}
    }
    visit(next.graph,reference)
    return changed?next:saved
  }catch{return saved}
}
async function upgradeSavedG36Timing(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||!saved.graph.inputs?.some(port=>port.name==='preconditioning_mode'))return saved
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36Timing(saved,reference)
}
async function upgradeG36TerminalFeedback(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||(saved.graph.inputs?.some(port=>port.name==='zone_01_measured_airflow')&&saved.graph.blocks?.find(block=>block.name==='zone_01')?.blocks?.some(block=>block.name==='heating_damper_select')))return saved
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36TerminalFeedback(saved,reference)
}
async function upgradeG36AhuFeedback(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||(saved.graph.inputs?.some(port=>port.name==='ahu_1_outdoor_airflow')&&saved.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='fan_enable')))return saved
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36AhuFeedback(saved,reference)
}
async function upgradeG36PressureTrim(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls')return saved
  const corrected=migrateG36PressureTiming(saved)
  if(corrected!==saved)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-pressure-timing-backup',JSON.stringify(saved))
  const hasTrim=corrected.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='pressure_previous')
  const hasSamplers=['ahu_1','ahu_2'].every(name=>corrected.graph.blocks?.find(block=>block.name===name)?.blocks?.some(block=>block.name==='pressure_request_sample'))
  if(hasTrim&&hasSamplers)return corrected
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  const trimmed=hasTrim?corrected:migrateG36PressureTrim(corrected,reference)
  if(!hasSamplers)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-pressure-sampler-backup',JSON.stringify(corrected))
  return migrateG36PressureRequestSampler(trimmed,reference)
}
async function upgradeG36Economizer(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='economizer_mixed_air_pi'))return saved
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36Economizer(saved,reference)
}
async function upgradeOutdoorAirCommand(saved:Project):Promise<Project>{
  if(!['FourPipeBuildingControls','FourPipeG36Controls'].includes(saved.graph.name))return saved
  if([1,2].every(ahu=>saved.graph.outputs?.some(port=>port.name===`ahu_${ahu}_outdoor_air_fraction`))&&Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}`).every(name=>saved.graph.outputs?.some(port=>port.name===`${name}_airflow_request`)))return saved
  const reference=await fetchJson<Block>('four_pipe_building.json')
  return migrateOutdoorAirCommand(saved,reference)
}
async function upgradeCoilControl(saved:Project):Promise<Project>{
  if(!['FourPipeBuildingControls','FourPipeG36Controls'].includes(saved.graph.name)||[1,2].every(ahu=>saved.graph.outputs?.some(port=>port.name===`ahu_${ahu}_heating_coil_valve_position`)&&saved.graph.outputs?.some(port=>port.name===`ahu_${ahu}_cooling_coil_valve_position`)&&saved.graph.blocks?.find(block=>block.name===`ahu_${ahu}`)?.blocks?.some(block=>block.name==='heating_coil_feedback')&&saved.graph.blocks?.find(block=>block.name===`ahu_${ahu}`)?.blocks?.some(block=>block.name==='cooling_coil_feedback')))return saved
  const reference=await fetchJson<Block>('four_pipe_building.json')
  return migrateCoilControl(saved,reference)
}
async function upgradeWaterLoopControl(saved:Project):Promise<Project>{
  if(!['FourPipeBuildingControls','FourPipeG36Controls'].includes(saved.graph.name)||['hot_water_pump_speed','chilled_water_pump_speed','hot_water_supply_setpoint_c','chilled_water_supply_setpoint_c'].every(name=>saved.graph.outputs?.some(port=>port.name===`plant_${name}`)))return saved
  const reference=await fetchJson<Block>('four_pipe_building.json')
  return migrateWaterLoopControl(saved,reference)
}
async function upgradeG36WaterReset(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='chilled_reset_previous'))return saved
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36WaterReset(saved,reference)
}
async function upgradeWaterEnergyControl(saved:Project):Promise<Project>{
  if(!['FourPipeBuildingControls','FourPipeG36Controls'].includes(saved.graph.name))return saved
  const plant=saved.graph.blocks?.find(block=>block.name==='plant')
  const hasEnergy=plant?.blocks?.some(block=>block.name==='hot_water_source_gain')
  const needsGate=saved.graph.name==='FourPipeBuildingControls'&&!plant?.blocks?.some(block=>block.name==='heat_pump_heating_enable_gate')
  const needsDampers=needsConventionalDamperCommandsMigration(saved)
  const needsStatus=needsConventionalStatusMigration(saved)
  const needsMode=needsConventionalModeMigration(saved)
  if(hasEnergy&&!needsGate&&!needsDampers&&!needsStatus&&!needsMode)return saved
  if(needsGate)storeMigrationBackup(localStorage,'ace-cdl-pre-source-enable-backup',JSON.stringify(saved))
  if(needsDampers)storeMigrationBackup(localStorage,'ace-cdl-pre-conventional-dampers-backup',JSON.stringify(saved))
  if(needsStatus)storeMigrationBackup(localStorage,'ace-cdl-pre-conventional-status-backup',JSON.stringify(saved))
  if(needsMode)storeMigrationBackup(localStorage,'ace-cdl-pre-conventional-mode-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>(saved.graph.name==='FourPipeG36Controls'?'four_pipe_g36.json':'four_pipe_building.json')
  const withEnergy=hasEnergy?saved:migrateWaterEnergyControl(saved,reference)
  const withGate=needsGate?migrateConventionalSourceEnable(withEnergy,reference):withEnergy
  const withDampers=needsDampers?migrateConventionalDamperCommands(withGate,reference):withGate
  const withStatus=needsStatus?migrateConventionalStatus(withDampers,reference):withDampers
  return needsMode?migrateConventionalMode(withStatus,reference):withStatus
}
async function upgradeWaterDispatch(saved:Project):Promise<Project>{
  if(!['FourPipeBuildingControls','FourPipeG36Controls'].includes(saved.graph.name)||!saved.graph.blocks?.some(block=>block.name==='plant'))return saved
  const hasDispatch=saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>['heat_pump_dispatch_fraction','heat_pump_allocated_fraction'].includes(block.name))
  if(hasDispatch&&!saved.graph.inputs?.some(port=>port.name==='heat_pump_fraction')&&saved.scenario.heat_pump_fraction===undefined)return saved
  storeMigrationBackup(localStorage,'ace-cdl-pre-dispatch-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_building.json')
  return migrateWaterDispatch(saved,reference)
}
async function upgradeG36WaterStageHold(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='chilled_reset_output_active'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-water-stage-hold-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36WaterStageHold(saved,reference)
}
async function upgradeG36HybridDispatch(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='heat_pump_allocated_fraction'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-hybrid-dispatch-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36HybridDispatch(saved,reference)
}
async function upgradeG36WaterCapacity(saved:Project):Promise<Project>{
  if(!needsG36WaterCapacityMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-water-capacity-backup',JSON.stringify(saved))
  return migrateG36WaterCapacity(saved,await fetchJson<Block>('four_pipe_g36.json'))
}
async function upgradeG36CapacityReallocation(saved:Project):Promise<Project>{
  if(!needsG36CapacityReallocation(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-capacity-reallocation-backup',JSON.stringify(saved))
  return migrateG36CapacityReallocation(saved,await fetchJson<Block>('four_pipe_g36.json'))
}
async function upgradeSourceStartRequests(saved:Project):Promise<Project>{
  if(!needsSourceStartMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-pre-source-start-backup',JSON.stringify(saved))
  const isG36=saved.graph.name==='FourPipeG36Controls'||saved.graph.blocks?.some(block=>block.block_type==='ZoneGroupModeController')
  return migrateSourceStartRequests(saved,await fetchJson<Block>(isG36?'four_pipe_g36.json':'four_pipe_building.json'))
}
async function upgradeG36SourceMinRun(saved:Project):Promise<Project>{
  if(!needsG36SourceMinRunMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-source-min-run-backup',JSON.stringify(saved))
  return migrateG36SourceMinRun(saved,await fetchJson<Block>('four_pipe_g36.json'))
}
async function upgradeG36ForcedStop(saved:Project):Promise<Project>{
  if(!needsG36ForcedStopMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-forced-stop-backup',JSON.stringify(saved))
  return migrateG36ForcedStop(saved,await fetchJson<Block>('four_pipe_g36.json'))
}
async function upgradeG36BackupStage(saved:Project):Promise<Project>{
  if(!needsG36BackupStageMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-backup-stage-backup',JSON.stringify(saved))
  return migrateG36BackupStage(saved,await fetchJson<Block>('four_pipe_g36.json'))
}
async function upgradeG36BackupFailover(saved:Project):Promise<Project>{
  if(!needsG36BackupFailoverMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-backup-failover-backup',JSON.stringify(saved))
  return migrateG36BackupFailover(saved,await fetchJson<Block>('four_pipe_g36.json'))
}
async function upgradeG36EquipmentAvailability(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='heat_pump_availability_gate'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-equipment-status-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36EquipmentAvailability(saved,reference)
}
async function upgradeG36ZoneGroups(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.some(block=>block.name==='zone_group_1'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-zone-groups-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36ZoneGroups(saved,reference)
}
async function upgradeG36OptimalStart(saved:Project):Promise<Project>{
  const needsStart=needsG36OptimalStartMigration(saved)
  const needsDailyRecovery=needsG36DailyRecoveryMigration(saved)
  const needsCo2=needsG36Co2Migration(saved)
  const needsWindowMinimum=needsG36WindowMinimumMigration(saved)
  const needsStandbyVentilation=needsG36VentilationStandbyMigration(saved)
  const needsAlarm=needsG36TerminalAirflowAlarmMigration(saved)
  const needsHighDischarge=needsG36HighDischargeAlarmMigration(saved)
  const needsAcknowledgement=needsG36AlarmAcknowledgementMigration(saved)
  const needsAhuAcknowledgement=needsG36AhuAlarmAcknowledgementMigration(saved)
  const needsPlantAcknowledgement=needsG36PlantAlarmAcknowledgementMigration(saved)
  const needsAirflowHysteresis=needsG36LowFlowHysteresisMigration(saved)
  const needsLowFlowInit=needsG36LowFlowInitMigration(saved)
  const needsFanAlarm=needsG36SupplyFanAlarmMigration(saved)
  const needsSensorAlarm=needsG36AirflowSensorAlarmMigration(saved)
  const needsDischargeAlarm=needsG36LowDischargeAlarmMigration(saved)
  const needsLeakAlarm=needsG36TerminalLeakAlarmMigration(saved)
  const needsLeakTiming=needsG36TerminalLeakTimingMigration(saved)
  const needsLeakFlowHysteresis=needsG36TerminalLeakFlowHysteresisMigration(saved)
  const needsFanSpeed=needsG36FanSpeedFeedbackMigration(saved)
  const needsAhuDamper=needsG36AhuDamperFeedbackMigration(saved)
  const needsEconHold=needsG36EconomizerHoldMigration(saved)
  const needsEconTransition=needsG36EconomizerDisableTransitionMigration(saved)
  const needsFixedDryBulb=needsG36FixedDryBulbMigration(saved)
  const needsCombinedDryBulb=needsG36CombinedDryBulbMigration(saved)
  const needsOptionalLowLimit=needsG36OptionalLowLimitMigration(saved)
  const needsOptionalDemandGate=needsG36OptionalDemandGateMigration(saved)
  const needsVentilationEffectiveness=needsG36VentilationEffectivenessMigration(saved)
  const needsMinimumOutdoorAirPi=needsG36MinimumOutdoorAirPiMigration(saved)
  if(!needsStart&&!needsDailyRecovery&&!needsCo2&&!needsWindowMinimum&&!needsStandbyVentilation&&!needsAlarm&&!needsHighDischarge&&!needsAcknowledgement&&!needsAhuAcknowledgement&&!needsPlantAcknowledgement&&!needsAirflowHysteresis&&!needsLowFlowInit&&!needsFanAlarm&&!needsSensorAlarm&&!needsDischargeAlarm&&!needsLeakAlarm&&!needsLeakTiming&&!needsLeakFlowHysteresis&&!needsFanSpeed&&!needsAhuDamper&&!needsEconHold&&!needsEconTransition&&!needsFixedDryBulb&&!needsCombinedDryBulb&&!needsOptionalLowLimit&&!needsOptionalDemandGate&&!needsVentilationEffectiveness&&!needsMinimumOutdoorAirPi)return saved
  if(needsStart)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-recovery-learning-backup',JSON.stringify(saved))
  if(needsDailyRecovery)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-start-qualified-recovery-backup',JSON.stringify(saved))
  if(needsCo2)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-co2-reset-backup',JSON.stringify(saved))
  if(needsWindowMinimum)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-window-minimum-backup',JSON.stringify(saved))
  if(needsStandbyVentilation)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-standby-ventilation-backup',JSON.stringify(saved))
  if(needsAlarm)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-airflow-alarm-backup',JSON.stringify(saved))
  if(needsHighDischarge)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-high-discharge-gate-backup',JSON.stringify(saved))
  if(needsAcknowledgement)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-vav-alarm-ack-backup',JSON.stringify(saved))
  if(needsAhuAcknowledgement)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-ahu-alarm-ack-backup',JSON.stringify(saved))
  if(needsPlantAcknowledgement)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-plant-alarm-ack-backup',JSON.stringify(saved))
  if(needsAirflowHysteresis)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-airflow-hysteresis-backup',JSON.stringify(saved))
  if(needsLowFlowInit)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-low-flow-init-backup',JSON.stringify(saved))
  if(needsFanAlarm)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-fan-alarm-backup',JSON.stringify(saved))
  if(needsSensorAlarm)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-sensor-alarm-backup',JSON.stringify(saved))
  if(needsDischargeAlarm)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-discharge-alarm-backup',JSON.stringify(saved))
  if(needsLeakAlarm)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-leak-alarm-backup',JSON.stringify(saved))
  if(needsLeakTiming)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-leak-timing-backup',JSON.stringify(saved))
  if(needsLeakFlowHysteresis)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-leak-flow-hysteresis-backup',JSON.stringify(saved))
  if(needsFanSpeed)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-fan-speed-feedback-backup',JSON.stringify(saved))
  if(needsAhuDamper)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-ahu-damper-feedback-backup',JSON.stringify(saved))
  if(needsEconHold)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-economizer-hold-backup',JSON.stringify(saved))
  if(needsEconTransition)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-economizer-transition-backup',JSON.stringify(saved))
  if(needsFixedDryBulb)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-fixed-dry-bulb-backup',JSON.stringify(saved))
  if(needsCombinedDryBulb)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-combined-dry-bulb-backup',JSON.stringify(saved))
  if(needsOptionalLowLimit)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-optional-low-limit-backup',JSON.stringify(saved))
  if(needsOptionalDemandGate)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-optional-demand-gate-backup',JSON.stringify(saved))
  if(needsVentilationEffectiveness)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-ventilation-effectiveness-backup',JSON.stringify(saved))
  if(needsMinimumOutdoorAirPi)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-minimum-oa-pi-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  const next=migrateG36DailyRecovery(needsStart?migrateG36OptimalStart(saved,reference):saved,reference)
  let upgraded=migrateG36TerminalAirflowAlarms(next,reference)
  upgraded=migrateG36SupplyFanAlarms(upgraded,reference)
  upgraded=migrateG36AirflowSensorAlarms(upgraded,reference)
  upgraded=migrateG36LowDischargeAlarms(upgraded,reference)
  upgraded=migrateG36TerminalLeakAlarms(upgraded,reference)
  upgraded=migrateG36TerminalLeakTiming(upgraded)
  upgraded=migrateG36TerminalLeakFlowHysteresis(upgraded)
  upgraded=migrateG36LowFlowInit(upgraded)
  upgraded=migrateG36FanSpeedFeedback(upgraded,reference)
  upgraded=migrateG36AhuDamperFeedback(upgraded,reference)
  upgraded=migrateG36EconomizerHold(upgraded,reference)
  upgraded=migrateG36EconomizerDisableTransition(upgraded,reference)
  upgraded=migrateG36FixedDryBulb(upgraded,reference)
  upgraded=migrateG36CombinedDryBulb(upgraded,reference)
  upgraded=migrateG36OptionalLowLimit(upgraded,reference)
  upgraded=migrateG36OptionalDemandGate(upgraded,reference)
  upgraded=migrateG36VentilationEffectiveness(upgraded)
  upgraded=migrateG36MinimumOutdoorAirPi(upgraded,reference)
  const withVavAcknowledgement=migrateG36AlarmAcknowledgement(migrateG36HighDischargeAlarm(migrateG36LowFlowHysteresis(migrateG36VentilationStandby(migrateG36WindowMinimum(migrateG36Co2(upgraded,reference),reference),reference),reference),reference),reference)
  return migrateG36PlantAlarmAcknowledgement(migrateG36AhuAlarmAcknowledgement(withVavAcknowledgement,reference),reference)
}
async function upgradeG36PressureRequests(saved:Project):Promise<Project>{
  const needsLevels=needsG36PressureRequestLevelsMigration(saved)
  const needsHysteresis=needsG36PressureRequestHysteresisMigration(saved)
  const needsLatch=needsG36PressureDamperLatchMigration(saved)
  if(!needsLevels&&!needsHysteresis&&!needsLatch)return saved
  storeMigrationBackup(localStorage,needsLevels?'ace-cdl-g36-pre-pressure-levels-backup':needsHysteresis?'ace-cdl-g36-pre-pressure-hysteresis-backup':'ace-cdl-g36-pre-pressure-latch-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36PressureDamperLatch(migrateG36PressureRequestHysteresis(migrateG36PressureRequestLevels(saved,reference),reference))
}
async function upgradeG36SatRequests(saved:Project):Promise<Project>{
  const needsLevels=needsG36SatRequestLevelsMigration(saved)
  const needsDynamic=needsG36SatDynamicSuppressionMigration(saved)
  const needsLoop=needsG36SatCoolingLoopMigration(saved)
  const needsSampler=needsG36SatSampledSetpointMigration(saved)
  const needsThresholds=needsG36SatThresholdMigration(saved)
  if(!needsLevels&&!needsDynamic&&!needsLoop&&!needsSampler&&!needsThresholds)return saved
  storeMigrationBackup(localStorage,needsLevels?'ace-cdl-g36-pre-sat-levels-backup':needsDynamic?'ace-cdl-g36-pre-sat-dynamic-backup':needsLoop?'ace-cdl-g36-pre-sat-cooling-loop-backup':needsSampler?'ace-cdl-g36-pre-sat-sampler-backup':'ace-cdl-g36-pre-sat-thresholds-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36SatThresholds(migrateG36SatSampledSetpoint(migrateG36SatCoolingLoop(migrateG36SatDynamicSuppression(migrateG36SatRequestLevels(saved,reference),reference),reference),reference),reference)
}
async function upgradeG36ZoneStatus(saved:Project):Promise<Project>{
  if(!needsG36ZoneStatusMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-zone-status-backup',JSON.stringify(saved))
  return migrateG36ZoneStatus(saved,await fetchJson<Block>('four_pipe_g36.json'))
}
async function upgradeG36OperationMode(saved:Project):Promise<Project>{
  if(!needsG36OperationModeMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-operation-mode-backup',JSON.stringify(saved))
  return migrateG36OperationMode(saved,await fetchJson<Block>('four_pipe_g36.json'))
}
async function upgradeG36GroupStatus(saved:Project):Promise<Project>{
  if(!needsG36GroupStatusMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-group-status-backup',JSON.stringify(saved))
  return migrateG36GroupStatus(saved,await fetchJson<Block>('four_pipe_g36.json'))
}
async function upgradeG36ObcOptimalStart(saved:Project):Promise<Project>{
  if(!needsG36ObcOptimalStartMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-obc-optimal-start-backup',JSON.stringify(saved))
  return migrateG36ObcOptimalStart(saved,await fetchJson<Block>('four_pipe_g36.json'))
}
async function upgradeG36GroupUnoccupied(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.some(block=>block.name==='zone_01_unoccupied_heat_previous'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-group-unoccupied-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36GroupUnoccupied(saved,reference)
}
async function upgradeG36Windows(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.inputs?.some(port=>port.name==='zone_01_window_closed'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-windows-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36Windows(saved,reference)
}
async function upgradeG36Overrides(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls')return saved
  const needsBase=!saved.graph.inputs?.some(port=>port.name==='zone_01_override')
  if(!needsBase&&!needsG36TimedOverrideMigration(saved))return saved
  storeMigrationBackup(localStorage,needsBase?'ace-cdl-g36-pre-overrides-backup':'ace-cdl-g36-pre-timed-overrides-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36TimedOverrides(needsBase?migrateG36Overrides(saved,reference):saved,reference)
}
async function upgradeG36ZoneModes(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='zone_01')?.blocks?.some(block=>block.name==='mode_select_schedule'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-zone-modes-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36ZoneModes(saved,reference)
}
async function upgradeG36BackupStartFault(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='boiler_start_fault_latch'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-backup-start-fault-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36BackupStartFault(saved,reference)
}
async function upgradeG36StartFault(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='heat_pump_start_fault_latch'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-start-fault-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36StartFault(saved,reference)
}
async function upgradeG36StageFeedback(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_stage_ready_observed'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-stage-feedback-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36StageFeedback(saved,reference)
}
async function upgradeG36HotWaterReset(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_reset_previous'))return saved
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36HotWaterReset(saved,reference)
}
async function upgradeG36TerminalHotRequests(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='terminal_hot_plant_request_active'))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-terminal-request-levels-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36TerminalHotRequestLevels(saved,reference)
}
async function upgradeG36SupplyTemperatureModes(saved:Project):Promise<Project>{
  if(!needsG36SupplyTemperatureModeMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-sat-modes-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36SupplyTemperatureModes(saved,reference)
}
async function upgradeG36ObcSupplyAirReset(saved:Project):Promise<Project>{
  if(!needsG36ObcSupplyAirResetMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-obc-sat-reset-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36ObcSupplyAirReset(saved,reference)
}
async function upgradeG36DamperPiReset(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls')return saved
  const needsPi=saved.graph.blocks?.filter(block=>block.block_type==='VAVBoxController').some(zone=>!zone.blocks?.find(block=>block.name==='damper_flow_pi')?.block_type.endsWith('PIDWithReset')||!(zone.connections??[]).some(edge=>{const [from,to]=edgeEndpoints(edge);return from==='supply_fan_proven'&&to==='damper_flow_pi.trigger'}))??false
  const needsMode=needsG36DamperModeGateMigration(saved)
  if(!needsPi&&!needsMode)return saved
  if(needsPi)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-damper-pi-reset-backup',JSON.stringify(saved))
  if(needsMode)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-damper-mode-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36DamperModeGate(needsPi?migrateG36DamperPiReset(saved,reference):saved,reference)
}
function upgradeHeatPumpModeThreshold(saved:Project):Project{
  if(!needsHeatPumpModeThresholdMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-pre-heat-pump-mode-threshold-backup',JSON.stringify(saved))
  return migrateHeatPumpModeThreshold(saved)
}
async function upgradeG36FlowOverride(saved:Project):Promise<Project>{
  if(!needsG36FlowOverrideMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-flow-override-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36FlowOverride(saved,reference)
}
async function upgradeG36DischargeMinimum(saved:Project):Promise<Project>{
  if(!needsG36DischargeMinimumMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-discharge-threshold-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36DischargeMinimum(saved,reference)
}
async function upgradeG36NormalFlow(saved:Project):Promise<Project>{
  if(!needsG36NormalFlowMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-normal-flow-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36NormalFlow(saved,reference)
}
async function upgradeG36DischargeSetpoint(saved:Project):Promise<Project>{
  if(!needsG36DischargeSetpointMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-discharge-setpoint-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36DischargeSetpoint(saved,reference)
}
async function upgradeG36ValvePiReset(saved:Project):Promise<Project>{
  if(!needsG36ValvePiResetMigration(saved))return saved
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-valve-pi-reset-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36ValvePiReset(saved,reference)
}
async function upgradeG36AhuPlantRequests(saved:Project):Promise<Project>{
  const withModes=await upgradeG36SupplyTemperatureModes(saved)
  const withReset=await upgradeG36ObcSupplyAirReset(withModes)
  const withClock=migrateG36FreezeHoldTiming(withReset)
  if(withClock!==withReset)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-freeze-hold-clock-backup',JSON.stringify(withReset))
  if(withClock.graph.name!=='FourPipeG36Controls'||withClock.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='ahu_hot_plant_request_count'))return withClock
  storeMigrationBackup(localStorage,'ace-cdl-g36-pre-ahu-plant-requests-backup',JSON.stringify(withClock))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36AhuPlantRequests(withClock,reference)
}
async function upgradeG36ControlUpdates(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls')return saved
  const hasSat=saved.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='sat_previous')
  const hasDischarge=saved.graph.blocks?.filter(block=>block.block_type==='VAVBoxController').every(block=>block.blocks?.some(child=>child.name==='discharge_valve_pi')&&block.blocks?.some(child=>child.name==='heating_warmup_airflow'))
  const hasDamper=saved.graph.blocks?.filter(block=>block.block_type==='VAVBoxController').every(block=>block.blocks?.some(child=>child.name==='damper_flow_pi'))
  const hasFreeze=saved.graph.blocks?.filter(block=>block.block_type==='AHUController').every(block=>block.blocks?.some(child=>child.name==='freeze_stage_one_latch')&&block.blocks?.some(child=>child.name==='freeze_stage_two_latch')&&block.blocks?.some(child=>child.name==='freeze_stage_three_latch'))&&saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='freeze_chilled_water_pump_enable')
  const hasEconomizerProof=saved.graph.blocks?.filter(block=>block.block_type==='AHUController').every(block=>block.blocks?.some(child=>child.name==='economizer_high_limit_hysteresis'))&&saved.graph.inputs?.some(port=>port.name==='ahu_1_supply_fan_proven')
  const hasOutdoorLimits=saved.graph.blocks?.filter(block=>block.block_type==='AHUController').every(block=>block.blocks?.some(child=>child.name==='outdoor_damper_physical_limit'))&&saved.graph.outputs?.some(port=>port.name==='ahu_1_minimum_outdoor_air_enabled')
  const hasReturnDamper=saved.graph.blocks?.filter(block=>block.block_type==='AHUController').every(block=>block.blocks?.some(child=>child.name==='return_damper_physical_limit'))&&saved.graph.outputs?.some(port=>port.name==='ahu_1_return_air_damper_position')
  const hasCommonDamperMapping=saved.graph.blocks?.filter(block=>block.block_type==='AHUController').every(block=>block.blocks?.some(child=>child.name==='minimum_oa_outdoor_stroke'))
  const hasVentilation=saved.graph.blocks?.filter(block=>block.block_type==='AHUController').every(block=>block.blocks?.some(child=>child.name==='vent_effective_limit'))&&saved.graph.blocks?.filter(block=>block.block_type==='VAVBoxController').every(block=>block.blocks?.some(child=>child.name==='ventilation_minimum_oa'))
  const hasWaterSourceProof=saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(child=>child.name==='hot_source_proven')&&saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(child=>child.name==='chilled_source_proven')
  const hasPumpStatus=['hot','chilled'].every(loop=>saved.graph.inputs?.some(port=>port.name===`${loop}_water_pump_running`)&&saved.graph.blocks?.find(block=>block.name==='plant')?.inputs?.some(port=>port.name===`${loop}_water_pump_running`))
  const hasPumpStartFault=['hot','chilled'].every(loop=>saved.graph.outputs?.some(port=>port.name===`plant_${loop}_water_pump_start_fault`))
  const hasWaterFlowFault=['hot','chilled'].every(loop=>saved.graph.outputs?.some(port=>port.name===`plant_${loop}_water_flow_fault`)&&saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name===`${loop}_flow_fault_delay`)&&saved.graph.blocks?.find(block=>block.name==='plant')?.connections?.some(edge=>edgeEndpoints(edge)[1]===`${loop}_water_flow_fault`)&&saved.graph.connections?.some(edge=>edgeEndpoints(edge)[1]===`plant_${loop}_water_flow_fault`))
  const hasWaterPressure=saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(child=>child.name==='hot_dp_pi')&&saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(child=>child.name==='chilled_dp_pi')
  const hasChilledWaterMapping=saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(child=>child.name==='chilled_reset_temperature_scale')
  const hasCoolingStartup=saved.graph.blocks?.filter(block=>block.block_type==='VAVBoxController').every(block=>!block.blocks?.some(child=>child.name==='cooling_enabled'))
  if(hasSat&&hasDischarge&&hasDamper&&hasFreeze&&hasEconomizerProof&&hasOutdoorLimits&&hasReturnDamper&&hasCommonDamperMapping&&hasVentilation&&hasWaterSourceProof&&hasPumpStatus&&hasPumpStartFault&&hasWaterFlowFault&&hasWaterPressure&&hasChilledWaterMapping&&hasCoolingStartup)return upgradeG36VentilationDesign(migrateG36VacancyDelay(saved))
  if(!hasDischarge)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-discharge-backup',JSON.stringify(saved))
  if(!hasDamper)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-damper-backup',JSON.stringify(saved))
  if(!hasFreeze)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-freeze-backup',JSON.stringify(saved))
  if(!hasEconomizerProof)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-economizer-proof-backup',JSON.stringify(saved))
  if(!hasOutdoorLimits)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-outdoor-limits-backup',JSON.stringify(saved))
  if(!hasReturnDamper)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-return-damper-backup',JSON.stringify(saved))
  if(!hasCommonDamperMapping)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-common-damper-mapping-backup',JSON.stringify(saved))
  if(!hasVentilation)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-ventilation-backup',JSON.stringify(saved))
  if(!hasWaterSourceProof)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-water-source-proof-backup',JSON.stringify(saved))
  if(!hasPumpStatus)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-water-pump-status-backup',JSON.stringify(saved))
  if(!hasPumpStartFault)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-water-pump-fault-backup',JSON.stringify(saved))
  if(!hasWaterFlowFault)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-water-flow-fault-backup',JSON.stringify(saved))
  if(!hasWaterPressure)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-water-pressure-backup',JSON.stringify(saved))
  if(!hasChilledWaterMapping)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-chilled-water-mapping-backup',JSON.stringify(saved))
  if(!hasCoolingStartup)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-cooling-startup-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  const withSat=hasSat?saved:migrateG36SupplyAirReset(saved,reference)
  const withDischarge=hasDischarge?withSat:migrateG36TerminalDischarge(withSat,reference)
  const withDamper=hasDamper?withDischarge:migrateG36DamperFeedback(withDischarge,reference)
  const withFreeze=hasFreeze?withDamper:migrateG36FreezeStageOne(withDamper,reference)
  const withEconomizer=hasEconomizerProof?withFreeze:migrateG36EconomizerEnable(withFreeze,reference)
  const withOutdoorLimits=hasOutdoorLimits?withEconomizer:migrateG36OutdoorAirLimits(withEconomizer,reference)
  const withReturnDamper=hasReturnDamper?withOutdoorLimits:migrateG36ReturnDamper(withOutdoorLimits,reference)
  const withCommonDamperMapping=hasCommonDamperMapping?withReturnDamper:migrateG36CommonDamperMapping(withReturnDamper,reference)
  const withVentilation=hasVentilation?withCommonDamperMapping:migrateG36Ventilation(withCommonDamperMapping,reference)
  const withWaterSourceProof=hasWaterSourceProof?withVentilation:migrateG36WaterSourceProof(withVentilation,reference)
  const withWaterPressure=hasWaterPressure?withWaterSourceProof:migrateG36WaterPressure(withWaterSourceProof,reference)
  const withPumpStatus=migrateG36WaterPumpStatus(withWaterPressure,reference)
  const withPumpFault=migrateG36WaterPumpStartFault(withPumpStatus,reference)
  const withFlowFault=migrateG36WaterFlowFault(withPumpFault,reference)
  const withChilledWaterMapping=hasChilledWaterMapping?withFlowFault:migrateG36ChilledWaterMapping(withFlowFault,reference)
  const withCoolingStartup=hasCoolingStartup?withChilledWaterMapping:migrateG36CoolingStartup(withChilledWaterMapping)
  return upgradeG36VentilationDesign(migrateG36VacancyDelay(withCoolingStartup))
}
async function upgradeG36PollutionMode(saved:Project):Promise<Project>{
  const needsPollution=needsG36PollutionModeMigration(saved)
  const needsEnthalpy=needsG36EnthalpyHighLimitMigration(saved)
  const needsDifferentialEnthalpy=needsG36DifferentialEnthalpyMigration(saved)
  if(!needsPollution&&!needsEnthalpy&&!needsDifferentialEnthalpy)return saved
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  let next=saved
  if(needsPollution){
    storeMigrationBackup(localStorage,'ace-cdl-g36-pre-pollution-mode-backup',JSON.stringify(next))
    next=migrateG36PollutionMode(next,reference)
  }
  if(needsEnthalpy){
    storeMigrationBackup(localStorage,'ace-cdl-g36-pre-enthalpy-high-limit-backup',JSON.stringify(next))
    next=migrateG36EnthalpyHighLimit(next,reference)
  }
  if(needsDifferentialEnthalpy){
    storeMigrationBackup(localStorage,'ace-cdl-g36-pre-differential-enthalpy-backup',JSON.stringify(next))
    next=migrateG36DifferentialEnthalpy(next,reference)
  }
  return next
}
async function upgradeG36VentilationDesign(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.filter(block=>block.block_type==='VAVBoxController').every(block=>block.blocks?.some(child=>child.name==='ventilation_floor_area')&&block.blocks?.some(child=>child.name==='ventilation_window_eligible')&&block.blocks?.some(child=>child.name==='co2_population_component')&&block.inputs?.some(port=>port.name==='occupancy_count')&&(!saved.graph.brick_model||saved.graph.brick_model.entities.some(entity=>entity.id===`${block.name}_occupancy_count`))))return saved
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36VentilationDesign(saved,reference)
}
function upgradeSupplyFeedback(saved:Project):Project{
  if(saved.graph.name!=='FourPipeBuildingControls')return saved
  const next=structuredClone(saved)
  let changed=false
  for(let ahu=1;ahu<=2;ahu++){
    let rewired=false
    const measured=`ahu_${ahu}_actual_supply_air_temp`
    for(let zone=(ahu-1)*5+1;zone<=ahu*5;zone++){
      const target=`zone_${String(zone).padStart(2,'0')}.supply_air_temp`
      const oldSource=`ahu_${ahu}.supply_air_temp`
      const edge=next.graph.connections?.find(item=>{const [from,to]=edgeEndpoints(item);return from===oldSource&&to===target})
      if(edge){edge.from=measured;delete edge.from_block;delete edge.from_output;changed=true;rewired=true}
    }
    if(rewired&&!next.graph.inputs?.some(port=>port.name===measured)){
      ;(next.graph.inputs??=[]).push({name:measured,type:'Real',unit:'°C',description:`Measured AHU ${ahu} supply air temperature from the previous step`})
    }
  }
  return changed?next:saved
}
function upgradeLayout(saved:Project):Project{
  const previous=[layoutFor(saved.graph,true),layoutFor(saved.graph,'previous'),layoutFor(saved.graph,'depth24')],improved=layoutFor(saved.graph)
  const next=structuredClone(saved)
  let changed=false
  for(const [name,fresh] of Object.entries(improved)){
    const position=next.layout[name]
    if(position&&fresh&&previous.some(layout=>layout[name]?.x===position.x&&layout[name]?.y===position.y)&&(position.x!==fresh.x||position.y!==fresh.y)){next.layout[name]=fresh;changed=true}
  }
  return changed?next:saved
}
function validateGraph(graph:Block){worker.postMessage({id:++jobId,type:'validate',graph:plainWorkerData(graph)})}
async function loadExample(){if(!exampleChoice.value)return;try{const graph=await expandLegacyVavs(await fetchJson<Block>(exampleChoice.value));const next:Project={version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};updateProject(next);Object.assign(scenario,structuredClone(defaultScenario),next.scenario);syncInputs(graph);results.value=new ArrowRows([]);resultSummary.value=null;navigate('editor')}catch(e){errors.value=[String(e)]}}
function loadCurrentExample(){exampleChoice.value=project.value?.graph.name==='FourPipeG36Controls'?'four_pipe_g36.json':'four_pipe_building.json';void loadExample()}
function triggerImport(){fileInput.value?.click()}
let coreCache:Promise<CxfDocument>|null=null
function cxfCore():Promise<CxfDocument>{return coreCache??=(async()=>{const response=await fetch(new URL('schemas/CXF-Core.jsonld',document.baseURI));if(!response.ok)throw new Error('Could not load the CXF-Core class library');return response.json() as Promise<CxfDocument>})()}
async function importFile(event:Event){const file=(event.target as HTMLInputElement).files?.[0];if(!file)return;try{const data=JSON.parse(await file.text());const isCxf=Array.isArray(data['@graph']);const importedGraph:Block=isCxf?importCxf(data,await cxfCore()):data.version===1?data.graph:data;if(!importedGraph?.name||!importedGraph.block_type)throw new Error('Expected a CDL block, ACE CDL project, or CXF JSON-LD document');const graph=await expandLegacyVavs(importedGraph);let next:Project=!isCxf&&data.version===1?{...data,graph,layout:{...layoutFor(graph),...data.layout},scenario:{...defaultScenario,...data.scenario}}:{version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};next=migrateConventionalPumpSizing(upgradeHeatPumpModeThreshold(next));next=await upgradeWaterDispatch(next);next=await upgradeG36ControlUpdates(await upgradeG36TerminalHotRequests(await upgradeG36HotWaterReset(await upgradeWaterEnergyControl(await upgradeG36WaterReset(await upgradeWaterLoopControl(await upgradeCoilControl(await upgradeOutdoorAirCommand(await upgradeG36Economizer(await upgradeG36PressureTrim(await upgradeG36AhuFeedback(await upgradeG36TerminalFeedback(await upgradeSavedG36Timing(next)))))))))))));next=await upgradeG36StageFeedback(await upgradeG36EquipmentAvailability(await upgradeG36HybridDispatch(await upgradeG36WaterStageHold(await upgradeWaterDispatch(next)))));next=await upgradeG36StartFault(next);next=await upgradeG36BackupStartFault(next);next=await upgradeG36ZoneModes(next);next=await upgradeG36ZoneGroups(next);next=await upgradeG36GroupUnoccupied(next);next=await upgradeG36Windows(next);next=await upgradeG36Overrides(next);next=await upgradeG36OptimalStart(next);next=await upgradeG36PressureRequests(next);next=await upgradeG36SatRequests(next);next=await upgradeG36ZoneStatus(next);next=await upgradeG36ObcOptimalStart(await upgradeG36GroupStatus(await upgradeG36OperationMode(next)));next=upgradeLayout(await fillExampleMetadata(upgradeSupplyFeedback(next)));next=await upgradeG36ValvePiReset(await upgradeG36DischargeSetpoint(await upgradeG36NormalFlow(await upgradeG36DischargeMinimum(await upgradeG36FlowOverride(await upgradeG36DamperPiReset(await upgradeG36AhuPlantRequests(next)))))));next=await upgradeG36BackupFailover(await upgradeG36BackupStage(await upgradeG36ForcedStop(await upgradeG36SourceMinRun(await upgradeSourceStartRequests(await upgradeG36WaterCapacity(next))))));next=await upgradeG36CapacityReallocation(await upgradeG36PollutionMode(next));updateProject(next);Object.assign(scenario,structuredClone(defaultScenario),next.scenario);syncInputs(next.graph);results.value=new ArrowRows([]);resultSummary.value=null;navigate('editor')}catch(e){errors.value=[String(e)]}finally{(event.target as HTMLInputElement).value=''}}
function download(name:string,contents:string,type='application/json'){const url=URL.createObjectURL(new Blob([contents],{type}));const a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000)}
function downloadSavedProject(){if(recoverableProject.value)download('ace-cdl-unreadable-saved-project.json',recoverableProject.value)}
function exportProject(){if(project.value)download(`${project.value.name}.ace-cdl.json`,JSON.stringify(project.value,null,2))}
function exportBrickFile(){const graph=project.value?.graph;if(!graph?.brick_model)return;const issues=validateBrickModel(graph);if(issues.length){errors.value=issues;return}download(`${graph.name}.brick.jsonld`,JSON.stringify(exportBrickModel(graph.brick_model),null,2),'application/ld+json')}
async function exportCxfFile(){if(!project.value)return;try{const cxf=exportCxf(project.value.graph,await cxfCore());download(`${project.value.name}.jsonld`,JSON.stringify(cxf,null,2),'application/ld+json')}catch(e){errors.value=[String(e)]}}
function exportResults(){
  const zoneColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_c`)
  const zoneCo2Columns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_co2_ppm`)
  const occupancyColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_occupied`)
  const rawPresenceColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_presence_raw`)
  const actualOccupantColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_occupant_count_actual`)
  const occupantSensorColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_occupant_count_sensor`)
  const occupantHeatColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_occupant_sensible_heat_kw`)
  const zoneAirflowColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_airflow`)
  const zoneDischargeColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_discharge_c`)
  const zoneDamperColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_actual_damper_position`)
  const zoneValveColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_actual_reheat_valve_position`)
  const zoneValveSensorColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_measured_reheat_valve_position`)
  const header=['index','schedule_occupied','preconditioning_mode',...occupancyColumns,...rawPresenceColumns,...actualOccupantColumns,...occupantSensorColumns,...occupantHeatColumns,...zoneAirflowColumns,...zoneDischargeColumns,...zoneDamperColumns,...zoneValveColumns,...zoneValveSensorColumns,'outdoor_c','avg_zone_c',...zoneColumns,...zoneCo2Columns,'requested_airflow','delivered_airflow','ahu_1_supply_setpoint_c','ahu_1_supply_actual_c','ahu_2_supply_setpoint_c','ahu_2_supply_actual_c','ahu_1_duct_pressure','ahu_2_duct_pressure','ahu_1_outdoor_air_fraction','ahu_2_outdoor_air_fraction','ahu_1_intake_opening','ahu_2_intake_opening','ahu_1_actual_fan_speed','ahu_2_actual_fan_speed','ahu_1_measured_fan_speed','ahu_2_measured_fan_speed','ahu_1_actual_outdoor_damper_position','ahu_2_actual_outdoor_damper_position','ahu_1_actual_return_damper_position','ahu_2_actual_return_damper_position','ahu_1_measured_outdoor_damper_position','ahu_2_measured_outdoor_damper_position','ahu_1_measured_return_damper_position','ahu_2_measured_return_damper_position','ahu_1_outdoor_airflow','ahu_2_outdoor_airflow','ahu_1_return_air_c','ahu_2_return_air_c','ahu_1_mixed_air_c','ahu_2_mixed_air_c','ahu_1_measured_mixed_air_c','ahu_2_measured_mixed_air_c','cool_request_kw','cool_kw','heat_request_kw','heat_kw','heat_pump_heating_used_kw','heat_pump_cooling_used_kw','heat_pump_electric_kw','heat_pump_ambient_exchange_kw','heat_pump_available_capacity_kw','heat_pump_heating_available_capacity_kw','heat_pump_cooling_available_capacity_kw','heat_pump_part_load_fraction','heat_pump_cop_multiplier','heat_pump_hot_water_cop_multiplier','heat_pump_chilled_water_cop_multiplier','heat_pump_heating_cop','heat_pump_cooling_cop','heat_pump_available','boiler_available','chiller_available','heat_pump_running','boiler_running','chiller_running','hot_water_plant_proven','boiler_used_kw','chiller_used_kw','electric_kw','gas_kw','hot_water_flow_kg_s','chilled_water_flow_kg_s','hot_water_actual_flow_kg_s','chilled_water_actual_flow_kg_s','hot_water_bypass_flow_kg_s','chilled_water_bypass_flow_kg_s','ahu_1_hot_water_flow_kg_s','ahu_2_hot_water_flow_kg_s','ahu_1_chilled_water_flow_kg_s','ahu_2_chilled_water_flow_kg_s','hot_water_reheat_flow_kg_s']
  const rows:string[]=[]
  for(let i=0;i<results.value.length;i++){const r=results.value.at(i)!;rows.push([r.index,r.schedule_occupied??'',r.preconditioning_mode??'',...(r.zone_occupied??Array(10).fill('')),...(r.zone_presence_raw??Array(10).fill('')),...(r.zone_occupant_count_actual??Array(10).fill('')),...(r.zone_occupant_count_sensor??Array(10).fill('')),...(r.zone_occupant_sensible_heat_kw??Array(10).fill('')),...(r.zone_airflow??Array(10).fill('')),...(r.zone_discharge_c??Array(10).fill('')),...(r.zone_actual_damper_position??Array(10).fill('')),...(r.zone_actual_reheat_valve_position??Array(10).fill('')),...(r.zone_measured_reheat_valve_position??Array(10).fill('')),r.outdoor_c,r.avg_zone_c,...r.zone_c,...(r.zone_co2_ppm??Array(10).fill('')),r.requested_airflow,r.delivered_airflow,r.ahu_1_supply_setpoint_c,r.ahu_1_supply_actual_c,r.ahu_2_supply_setpoint_c,r.ahu_2_supply_actual_c,r.ahu_1_duct_pressure,r.ahu_2_duct_pressure,r.ahu_1_outdoor_air_fraction,r.ahu_2_outdoor_air_fraction,r.ahu_1_intake_opening??'',r.ahu_2_intake_opening??'',r.ahu_1_actual_fan_speed??'',r.ahu_2_actual_fan_speed??'',r.ahu_1_measured_fan_speed??'',r.ahu_2_measured_fan_speed??'',r.ahu_1_actual_outdoor_damper_position??'',r.ahu_2_actual_outdoor_damper_position??'',r.ahu_1_actual_return_damper_position??'',r.ahu_2_actual_return_damper_position??'',r.ahu_1_measured_outdoor_damper_position??'',r.ahu_2_measured_outdoor_damper_position??'',r.ahu_1_measured_return_damper_position??'',r.ahu_2_measured_return_damper_position??'',r.ahu_1_outdoor_airflow,r.ahu_2_outdoor_airflow,r.ahu_1_return_air_c??'',r.ahu_2_return_air_c??'',r.ahu_1_mixed_air_c,r.ahu_2_mixed_air_c,r.ahu_1_measured_mixed_air_c??'',r.ahu_2_measured_mixed_air_c??'',r.cool_request_kw,r.cool_kw,r.heat_request_kw,r.heat_kw,r.heat_pump_heating_used_kw,r.heat_pump_cooling_used_kw,r.heat_pump_electric_kw??'',r.heat_pump_ambient_exchange_kw??'',r.heat_pump_available_capacity_kw??'',r.heat_pump_heating_available_capacity_kw??'',r.heat_pump_cooling_available_capacity_kw??'',r.heat_pump_part_load_fraction??'',r.heat_pump_cop_multiplier??'',r.heat_pump_hot_water_cop_multiplier??'',r.heat_pump_chilled_water_cop_multiplier??'',r.heat_pump_heating_cop??'',r.heat_pump_cooling_cop??'',r.heat_pump_available??'',r.boiler_available??'',r.chiller_available??'',r.heat_pump_running??'',r.boiler_running??'',r.chiller_running??'',r.hot_water_plant_proven??'',r.boiler_used_kw,r.chiller_used_kw,r.electric_kw,r.gas_kw,r.hot_water_flow_kg_s??'',r.chilled_water_flow_kg_s??'',r.hot_water_actual_flow_kg_s??'',r.chilled_water_actual_flow_kg_s??'',r.hot_water_bypass_flow_kg_s??'',r.chilled_water_bypass_flow_kg_s??'',r.ahu_1_hot_water_flow_kg_s??'',r.ahu_2_hot_water_flow_kg_s??'',r.ahu_1_chilled_water_flow_kg_s??'',r.ahu_2_chilled_water_flow_kg_s??'',r.hot_water_reheat_flow_kg_s??''].join(','))}
  download('ace-cdl-simulation-2025.csv',[header.join(','),...rows].join('\n'),'text/csv')
}
function runStep(){if(project.value){testResult.value='';worker.postMessage({id:++jobId,type:'execute',graph:plainWorkerData(project.value.graph),inputs:plainWorkerData(testInputs)})}}
function saveJointPerformanceMap(value:Scenario['heat_pump_joint_performance']){scenario.heat_pump_joint_performance=value;saveScenario()}
function addHeatPumpPoint(){const points=scenario.heat_pump_performance;const last=points.at(-1)!;points.push({...last,outdoor_c:last.outdoor_c+10});saveScenario()}
function removeHeatPumpPoint(index:number){scenario.heat_pump_performance.splice(index,1);saveScenario()}
function addPartLoadPoint(){
  const points=scenario.heat_pump_part_load_performance
  if(points.length===1){
    const endpoint=points[0].load_fraction<=0.5?1:0
    points.push({load_fraction:endpoint,cop_multiplier:points[0].cop_multiplier})
    points.sort((a,b)=>a.load_fraction-b.load_fraction)
    saveScenario()
    return
  }
  let gap=0,index=0
  for(let i=0;i<points.length-1;i++){
    const size=points[i+1].load_fraction-points[i].load_fraction
    if(size>gap){gap=size;index=i}
  }
  if(gap<=0.001)return
  points.splice(index+1,0,{load_fraction:(points[index].load_fraction+points[index+1].load_fraction)/2,cop_multiplier:(points[index].cop_multiplier+points[index+1].cop_multiplier)/2})
  saveScenario()
}
function removePartLoadPoint(index:number){scenario.heat_pump_part_load_performance.splice(index,1);saveScenario()}
function waterCopPoints(loop:'hot'|'chilled'){
  return loop==='hot'?scenario.heat_pump_hot_water_cop_performance:scenario.heat_pump_chilled_water_cop_performance
}
function addWaterCopPoint(loop:'hot'|'chilled'){
  const points=waterCopPoints(loop)
  if(points.length===1){points.push({water_c:points[0].water_c+1,cop_multiplier:points[0].cop_multiplier,capacity_multiplier:points[0].capacity_multiplier??1});saveScenario();return}
  let gap=0,index=0
  for(let i=0;i<points.length-1;i++){
    const size=points[i+1].water_c-points[i].water_c
    if(size>gap){gap=size;index=i}
  }
  if(gap<=0.001)return
  points.splice(index+1,0,{water_c:(points[index].water_c+points[index+1].water_c)/2,cop_multiplier:(points[index].cop_multiplier+points[index+1].cop_multiplier)/2,capacity_multiplier:((points[index].capacity_multiplier??1)+(points[index+1].capacity_multiplier??1))/2})
  saveScenario()
}
function removeWaterCopPoint(loop:'hot'|'chilled',index:number){waterCopPoints(loop).splice(index,1);saveScenario()}
function setWaterCapacityMultiplier(loop:'hot'|'chilled',index:number,event:Event){waterCopPoints(loop)[index].capacity_multiplier=Number((event.target as HTMLInputElement).value);saveScenario()}
function addEquipmentOutage(){scenario.equipment_outages.push({equipment:'heat_pump',start_hour:24,duration_hours:2});saveScenario()}
function removeEquipmentOutage(index:number){scenario.equipment_outages.splice(index,1);saveScenario()}
function addStartFailure(){scenario.equipment_start_failures.push({equipment:'heat_pump',start_hour:24,duration_hours:2});saveScenario()}
function removeStartFailure(index:number){scenario.equipment_start_failures.splice(index,1);saveScenario()}
function addFanSpeedSensorEvent(){scenario.fan_speed_sensor_events.push({ahu:1,bias:-0.3,start_hour:24,duration_hours:1});saveScenario()}
function removeFanSpeedSensorEvent(index:number){scenario.fan_speed_sensor_events.splice(index,1);saveScenario()}
function addFanFault(){scenario.fan_faults.push({equipment:'ahu_1_stuck_off',start_hour:24,duration_hours:2});saveScenario()}
function addWaterPumpFault(){scenario.water_pump_faults.push({loop_name:'hot',start_hour:24,duration_hours:2});saveScenario()}
function addWaterFlowSensorEvent(){scenario.water_flow_sensor_events.push({loop_name:'hot',bias_kg_s:-5,start_hour:24,duration_hours:1});saveScenario()}
function removeWaterFlowSensorEvent(index:number){scenario.water_flow_sensor_events.splice(index,1);saveScenario()}
function addTerminalFault(){scenario.terminal_faults.push({zone:1,kind:'damper_stuck_open',start_hour:24,duration_hours:2});saveScenario()}
function setTerminalLeakFraction(index:number,event:Event){const fault=scenario.terminal_faults[index];if(!fault)return;fault.leak_fraction=Number((event.target as HTMLInputElement).value);saveScenario()}
function addTerminalValveSensorEvent(){scenario.terminal_valve_sensor_events.push({zone:1,bias:0.8,start_hour:24,duration_hours:2});saveScenario()}
function removeTerminalValveSensorEvent(index:number){scenario.terminal_valve_sensor_events.splice(index,1);saveScenario()}
function addAhuDamperFault(){scenario.ahu_damper_faults.push({ahu:1,damper:'outdoor',stuck_position:1,start_hour:24,duration_hours:2});saveScenario()}
function addAhuDamperSensorEvent(){scenario.ahu_damper_sensor_events.push({ahu:1,damper:'outdoor',bias:0.8,start_hour:24,duration_hours:2});saveScenario()}
function addAhuMixedAirSensorEvent(){scenario.ahu_mixed_air_sensor_events.push({ahu:1,bias_c:5,start_hour:24,duration_hours:2});saveScenario()}
function removeAhuMixedAirSensorEvent(index:number){scenario.ahu_mixed_air_sensor_events.splice(index,1);saveScenario()}
function removeAhuDamperSensorEvent(index:number){scenario.ahu_damper_sensor_events.splice(index,1);saveScenario()}
function addOutdoorTemperatureEvent(){scenario.outdoor_temperature_events.push({delta_c:5,start_hour:24.05,duration_hours:0.05});saveScenario()}
function removeOutdoorTemperatureEvent(index:number){scenario.outdoor_temperature_events.splice(index,1);saveScenario()}
function removeAhuDamperFault(index:number){scenario.ahu_damper_faults.splice(index,1);saveScenario()}
function removeTerminalFault(index:number){scenario.terminal_faults.splice(index,1);saveScenario()}
function removeFanFault(index:number){scenario.fan_faults.splice(index,1);saveScenario()}
function removeWaterPumpFault(index:number){scenario.water_pump_faults.splice(index,1);saveScenario()}
function addWindowOpening(){scenario.window_openings.push({zone:1,start_hour:24,duration_hours:2});saveScenario()}
function removeWindowOpening(index:number){scenario.window_openings.splice(index,1);saveScenario()}
function addOccupantCountEvent(){scenario.occupant_count_events.push({zone:1,count:8,start_hour:24,duration_hours:1});saveScenario()}
function removeOccupantCountEvent(index:number){scenario.occupant_count_events.splice(index,1);saveScenario()}
function addOccupancyCountSensorEvent(){scenario.occupancy_count_sensor_events.push({zone:1,count:0,start_hour:24,duration_hours:1});saveScenario()}
function removeOccupancyCountSensorEvent(index:number){scenario.occupancy_count_sensor_events.splice(index,1);saveScenario()}
function addOccupancySensorEvent(){scenario.occupancy_sensor_events.push({zone:1,present:true,start_hour:24,duration_hours:1});saveScenario()}
function removeOccupancySensorEvent(index:number){scenario.occupancy_sensor_events.splice(index,1);saveScenario()}
function addOccupancyOverride(){scenario.occupancy_overrides.push({zone:1,start_hour:24,duration_hours:2});saveScenario()}
function removeOccupancyOverride(index:number){scenario.occupancy_overrides.splice(index,1);saveScenario()}
function addAlarmAcknowledgement(){scenario.alarm_acknowledgements.push({zone:1,start_hour:24,duration_hours:0.25});saveScenario()}
function removeAlarmAcknowledgement(index:number){scenario.alarm_acknowledgements.splice(index,1);saveScenario()}
function addAhuAlarmAcknowledgement(){scenario.ahu_alarm_acknowledgements.push({ahu:1,start_hour:24,duration_hours:0.25});saveScenario()}
function removeAhuAlarmAcknowledgement(index:number){scenario.ahu_alarm_acknowledgements.splice(index,1);saveScenario()}
function addPlantAlarmAcknowledgement(){scenario.plant_alarm_acknowledgements.push({start_hour:24,duration_hours:0.25});saveScenario()}
function removePlantAlarmAcknowledgement(index:number){scenario.plant_alarm_acknowledgements.splice(index,1);saveScenario()}
type ZoneAirProperty = 'zone_air_volume_m3_by_zone' | 'zone_infiltration_m3_s_by_zone' | 'open_window_additional_infiltration_m3_s_by_zone'
const zoneAirDefault:Record<ZoneAirProperty,'zone_air_volume_m3'|'zone_infiltration_m3_s'|'open_window_additional_infiltration_m3_s'>={zone_air_volume_m3_by_zone:'zone_air_volume_m3',zone_infiltration_m3_s_by_zone:'zone_infiltration_m3_s',open_window_additional_infiltration_m3_s_by_zone:'open_window_additional_infiltration_m3_s'}
function resetZoneAirProperty(key:ZoneAirProperty){delete scenario[key];saveScenario()}
function setZoneAirProperty(key:ZoneAirProperty,index:number,event:Event){const value=Number((event.target as HTMLInputElement).value);const min=key==='zone_air_volume_m3_by_zone'?1:0;const max=key==='zone_air_volume_m3_by_zone'?10000:5;if(!Number.isFinite(value)||value<min||value>max)return;scenario[key]??=Array(10).fill(scenario[zoneAirDefault[key]]);scenario[key]![index]=value;saveScenario()}
function resetZoneDesignOccupants(){delete scenario.zone_design_occupants;saveScenario()}
function setZoneDesignOccupants(index:number,event:Event){const count=Number((event.target as HTMLInputElement).value);if(!Number.isFinite(count)||count<0||count>1000)return;scenario.zone_design_occupants??=Array(10).fill(scenario.design_occupants_per_zone);scenario.zone_design_occupants[index]=count;saveScenario()}
function saveScenario(){if(!project.value)return;const next=structuredClone(project.value);next.scenario=scenarioForWorker(scenario);updateProject(next)}
function setHumiditySource(event:Event){scenario.outdoor_relative_humidity=(event.target as HTMLSelectElement).value==='weather'?null:0.5;saveScenario()}
function setReturnHumiditySource(event:Event){scenario.return_air_relative_humidity=(event.target as HTMLSelectElement).value==='modeled'?null:0.5;saveScenario()}
function syncRoute(){const next=routeFromHash();if(tab.value!==next){tab.value=next;void nextTick(()=>window.scrollTo({top:0,behavior:'instant'}))}}
function navigate(target:Tab){if(location.hash!==`#${target}`)location.hash=target;else syncRoute()}
const firstStep=Date.UTC(2025,0,1)
function timeValue(index:number){return new Date(firstStep+index*900000).toISOString().slice(0,16)}
function timeIndex(value:string){const stamp=Date.parse(`${value}Z`);return Number.isFinite(stamp)?Math.round((stamp-firstStep)/900000):0}
function setTimeRange(start:number,end:number){const low=Math.max(0,Math.min(35035,Math.round(start)));timeStart.value=low;timeEnd.value=Math.max(low+4,Math.min(35039,Math.round(end)))}
function resetTimeRange(){setTimeRange(0,35039);panMode.value=false;rowVersion.value++}
function zoomTime(scale:number){const center=(timeStart.value+timeEnd.value)/2;const width=Math.max(4,Math.min(35039,Math.round((timeEnd.value-timeStart.value)*scale)));let start=Math.round(center-width/2);start=Math.max(0,Math.min(35039-width,start));setTimeRange(start,start+width)}
function panTimeTo(start:number){const width=timeEnd.value-timeStart.value;const next=Math.max(0,Math.min(35039-width,Math.round(start)));timeStart.value=next;timeEnd.value=next+width}
function shiftTime(direction:number){panTimeTo(timeStart.value+direction*Math.max(1,Math.round((timeEnd.value-timeStart.value)*0.5)))}
function addSignal(){if(!candidateSignal.value||selectedSignals.value.includes(candidateSignal.value))return;selectedSignals.value=[...selectedSignals.value,candidateSignal.value];candidateSignal.value=''}
function removeSignal(id:string){selectedSignals.value=selectedSignals.value.filter(item=>item!==id)}
async function runYear(){if(!project.value||!isBuildingGraph.value||running.value)return;running.value=true;simError.value='';resultSummary.value=null;currentRunId.value=null;progress.value=0;lastResultRefresh=0;const graph=plainWorkerData(project.value.graph);const traces=selectedSignalDetails.value.filter(signal=>signal.kind!=='physical').map(signal=>signal.id);activeTraceIds.value=traces;results.value=new ArrowRows(traces);const id=++jobId;activeRunId=id;activeRunContext={id,program:project.value.graph.name==='FourPipeG36Controls'?'g36':project.value.graph.name==='FourPipeBuildingControls'?'conventional':'custom',label:project.value.graph.name==='FourPipeG36Controls'?'G36-style controls':project.value.graph.name==='FourPipeBuildingControls'?'Conventional controls':project.value.name,startedAt:performance.now(),requestedStepSeconds:scenario.step_seconds,effectiveStepSeconds:effectiveStepSeconds.value};try{const weather=await fetchJson<unknown[]>('weather_2025.json');if(activeRunId===id)worker.postMessage({id,type:'simulate',graph,scenario:scenarioForWorker(scenario),weather,traces})}catch(e){if(activeRunId===id){simError.value=String(e);running.value=false;activeRunId=null;activeRunContext=null}}}
function cancelRun(){activeRunId=null;activeRunContext=null;worker.postMessage({type:'cancel'});running.value=false;resultSummary.value=null}
function renameRun(id:string,label:string){
  const trimmed=label.trim().slice(0,80)
  if(!trimmed)return
  runHistory.value=runHistory.value.map(run=>run.id===id?{...run,label:trimmed}:run)
  if(!saveRunHistory(localStorage,runHistory.value))simError.value='Run label changed, but browser storage is full. The label will be lost after this tab closes.'
}
function scrollToComparison(metric:keyof Summary){document.getElementById(`comparison-${metric}`)?.scrollIntoView({behavior:'smooth',block:'center'})}
function recordCompletedRun(id:number,summary:Summary){
  const context=activeRunContext
  if(!context||context.id!==id)return
  const run:SimulationRun={id:crypto.randomUUID(),program:context.program,label:context.label,completedAt:new Date().toISOString(),durationMs:performance.now()-context.startedAt,requestedStepSeconds:context.requestedStepSeconds,effectiveStepSeconds:context.effectiveStepSeconds,summary:{...summary}}
  runHistory.value=[run,...runHistory.value].slice(0,12)
  currentRunId.value=run.id
  if(!saveRunHistory(localStorage,runHistory.value))simError.value='Run completed, but browser storage is full. The comparison will be lost after this tab closes.'
  activeRunContext=null
}
const wasmReloadKey='ace-cdl-wasm-init-reload'
function reloadLatestRuntime(){
  sessionStorage.setItem(wasmReloadKey,String(Date.now()))
  const url=new URL(location.href)
  url.searchParams.set('runtime_refresh',String(Date.now()))
  location.replace(url.href)
}
function recoverWasmInit(data:{stage?:string;message?:string}):boolean{
  // A stale Pages tab can report the table mismatch from either initialization
  // or its first quick-test call, depending on which worker message arrives first.
  if(!data.message?.includes('WebAssembly.Table.set'))return false
  // Keep recovery available after a later deployment even if an earlier
  // failed reload left the session marker behind.
  const now=Date.now()
  const lastReload=Number(sessionStorage.getItem(wasmReloadKey)??0)
  if(now-lastReload<60_000)return false
  reloadLatestRuntime()
  return true
}
worker.onmessage=({data})=>{if(data.type==='validation'){errors.value=[...data.errors,...(project.value?validateBrickModel(project.value.graph):[])]}else if(data.type==='result'){sessionStorage.removeItem(wasmReloadKey);runtimeError.value='';testResult.value=JSON.stringify(data.result,null,2)}else if(data.type==='batch'){if(data.id!==activeRunId)return;try{results.value.append(data.buffer)}catch(e){simError.value=String(e);cancelRun();return}sessionStorage.removeItem(wasmReloadKey);runtimeError.value='';progress.value=data.progress;resultSummary.value=data.summary;if(data.done||performance.now()-lastResultRefresh>500){triggerRef(results);rowVersion.value++;lastResultRefresh=performance.now()}if(data.done){recordCompletedRun(data.id,data.summary);running.value=false;activeRunId=null}}else if(data.type==='cancelled'){if(data.id===activeRunId){running.value=false;activeRunId=null;activeRunContext=null;resultSummary.value=null}}else if(data.type==='error'){if(recoverWasmInit(data))return;const tableError=data.message?.includes('WebAssembly.Table.set');if(data.stage==='wasm-init'||tableError){runtimeError.value=tableError?'The browser has an incompatible cached simulation runtime. Reload the latest runtime to run this graph.':`Runtime initialization failed: ${data.message}`;testResult.value=''}if(data.id===activeRunId){simError.value=data.message;running.value=false;activeRunId=null;activeRunContext=null;resultSummary.value=null}else if(!runtimeError.value)testResult.value=data.message}}
onMounted(async()=>{runHistory.value=readRunHistory(localStorage);window.addEventListener('hashchange',syncRoute);syncRoute();let stored:string|null=null;try{stored=localStorage.getItem('ace-cdl-project-v1');const safeMode=new URLSearchParams(location.search).get('safe')==='1';if(safeMode&&stored){recoverableProject.value=stored;storageWarning.value='Safe mode opened the bundled controls. Your saved project is still available to download.'}if(stored&&!safeMode){let parsed=JSON.parse(stored) as Project;if(!parsed?.graph?.name||!parsed.graph.block_type)throw new Error('Saved project is missing its control graph');const expandedGraph=await expandLegacyVavs(parsed.graph);const expandedLegacyVav=expandedGraph!==parsed.graph;if(expandedLegacyVav){storeMigrationBackup(localStorage,'ace-cdl-legacy-vav-backup',stored);parsed.graph=expandedGraph;parsed.layout={...layoutFor(expandedGraph),...parsed.layout}}parsed.layout??=layoutFor(parsed.graph);const modeThresholdMigrated=needsHeatPumpModeThresholdMigration(parsed);const pumpSizingMigrated=needsConventionalPumpSizingMigration(parsed);if(pumpSizingMigrated)storeMigrationBackup(localStorage,'ace-cdl-pre-branch-flow-pump-backup',stored);parsed=migrateConventionalPumpSizing(upgradeHeatPumpModeThreshold(parsed));parsed.scenario={...defaultScenario,...parsed.scenario};if(parsed.graph.name==='FourPipeBuildingControls'&&!(parsed.graph.blocks??[]).some(b=>b.block_type==='PlantDispatch')){storeMigrationBackup(localStorage,'ace-cdl-legacy-project-backup',stored);const graph=await fetchJson<Block>('four_pipe_building.json');for(const old of parsed.graph.blocks??[]){const next=graph.blocks?.find(b=>b.name===old.name);if(!next)continue;const oldParam=(name:string)=>old.parameters?.find(p=>p.name===name)?.value;for(const [oldName,childName] of [['cooling_setpoint','cool_setpoint'],['heating_setpoint','heat_setpoint'],['min_damper_position','minimum_damper'],['min_airflow','minimum_airflow']] as const){const value=oldParam(oldName);const p=next.blocks?.find(b=>b.name===childName)?.parameters?.find(p=>p.name==='k');if(p&&typeof value==='number')p.value=value}}const legacyShare=parsed.scenario.heat_pump_fraction;if(typeof legacyShare==='number'){const dispatch=graph.blocks?.find(b=>b.name==='plant')?.blocks?.find(b=>b.name==='heat_pump_dispatch_fraction')?.parameters?.find(p=>p.name==='k');if(dispatch)dispatch.value=Math.max(0,Math.min(1,legacyShare));delete parsed.scenario.heat_pump_fraction}parsed={version:1,name:graph.name,graph,scenario:parsed.scenario,layout:layoutFor(graph)};updateProject(parsed)}else{if(parsed.graph.name==='FourPipeG36Controls'&&parsed.graph.inputs?.some(port=>port.name==='preconditioning_mode'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-timing-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&(!parsed.graph.inputs?.some(port=>port.name==='zone_01_measured_airflow')||!parsed.graph.blocks?.find(block=>block.name==='zone_01')?.blocks?.some(block=>block.name==='heating_damper_select')))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-terminal-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&(!parsed.graph.inputs?.some(port=>port.name==='ahu_1_outdoor_airflow')||!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='fan_enable')))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-ahu-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='pressure_previous'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-pressure-trim-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='economizer_mixed_air_pi'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-economizer-backup',stored);if(['FourPipeBuildingControls','FourPipeG36Controls'].includes(parsed.graph.name)&&!parsed.graph.outputs?.some(port=>port.name==='ahu_1_heating_coil_valve_position'))storeMigrationBackup(localStorage,'ace-cdl-pre-coil-control-backup',stored);if(['FourPipeBuildingControls','FourPipeG36Controls'].includes(parsed.graph.name)&&!parsed.graph.outputs?.some(port=>port.name==='plant_hot_water_pump_speed'))storeMigrationBackup(localStorage,'ace-cdl-pre-water-loop-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='chilled_reset_previous'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-water-reset-backup',stored);if(['FourPipeBuildingControls','FourPipeG36Controls'].includes(parsed.graph.name)&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_water_source_gain'))storeMigrationBackup(localStorage,'ace-cdl-pre-water-energy-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_reset_previous'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-hot-water-reset-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_reset_all_request_count'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-terminal-hot-requests-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='sat_previous'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-sat-reset-backup',stored);parsed=await upgradeWaterDispatch(parsed);const migrated=await upgradeG36ControlUpdates(await upgradeG36TerminalHotRequests(await upgradeG36HotWaterReset(await upgradeWaterEnergyControl(await upgradeG36WaterReset(await upgradeWaterLoopControl(await upgradeCoilControl(await upgradeOutdoorAirCommand(await upgradeG36Economizer(await upgradeG36PressureTrim(await upgradeG36AhuFeedback(await upgradeG36TerminalFeedback(await upgradeSavedG36Timing(parsed)))))))))))));const dispatched=await upgradeG36HybridDispatch(await upgradeG36WaterStageHold(await upgradeWaterDispatch(migrated)));const enriched=await upgradeG36ZoneStatus(await fillExampleMetadata(upgradeSupplyFeedback(await upgradeG36OptimalStart(await upgradeG36Overrides(await upgradeG36Windows(await upgradeG36GroupUnoccupied(await upgradeG36ZoneGroups(await upgradeG36ZoneModes(await upgradeG36BackupStartFault(await upgradeG36StartFault(await upgradeG36StageFeedback(await upgradeG36EquipmentAvailability(dispatched)))))))))))));const modeEnriched=await upgradeG36SatRequests(await upgradeG36PressureRequests(await upgradeG36ObcOptimalStart(await upgradeG36GroupStatus(await upgradeG36OperationMode(enriched)))));const latest=await upgradeG36ValvePiReset(await upgradeG36DischargeSetpoint(await upgradeG36NormalFlow(await upgradeG36DischargeMinimum(await upgradeG36FlowOverride(await upgradeG36DamperPiReset(await upgradeG36AhuPlantRequests(modeEnriched)))))));const capacityEnriched=await upgradeG36WaterCapacity(latest);const sourceEnriched=await upgradeG36BackupFailover(await upgradeG36BackupStage(await upgradeG36ForcedStop(await upgradeG36SourceMinRun(await upgradeSourceStartRequests(capacityEnriched)))));const pollutionEnriched=await upgradeG36CapacityReallocation(await upgradeG36PollutionMode(sourceEnriched));if(pollutionEnriched===parsed){if(expandedLegacyVav||modeThresholdMigrated||pumpSizingMigrated)updateProject(parsed);else{project.value=parsed;validateGraph(parsed.graph)}}else{parsed=pollutionEnriched;updateProject(parsed)}}Object.assign(scenario,structuredClone(defaultScenario),parsed.scenario);syncInputs(parsed.graph)}else{const graph=await fetchJson<Block>('four_pipe_building.json');const next:Project={version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};if(safeMode&&stored){project.value=next;validateGraph(graph)}else updateProject(next);syncInputs(graph)}}catch(e){
  console.error('Could not open saved controls',e)
  recoverableProject.value=stored
  storageWarning.value=stored?`Saved controls need an update (${String(e)}). You can inspect or download your saved graph, then load the current example.`:'Could not open the default controls.'
  try{
    const graph=await fetchJson<Block>('four_pipe_building.json')
    project.value={version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)}
    syncInputs(graph)
    validateGraph(graph)
  }catch(loadError){errors.value=[String(e),String(loadError)]}
}})
onUnmounted(()=>{window.removeEventListener('hashchange',syncRoute);worker.terminate()})
</script>
