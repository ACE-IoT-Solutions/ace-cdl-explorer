<template>
  <div class="app-shell">
    <header class="site-header"><div class="brand" @click="navigate('primer')"><span class="brand-mark">ACE<span>◆</span></span><span class="brand-divider"/><span class="brand-name">CDL <b>EXPLORER</b></span></div><nav class="top-nav" aria-label="Main navigation"><button :class="{active:tab==='primer'}" @click="navigate('primer')">Primer</button><button :class="{active:tab==='editor'}" @click="navigate('editor')">Visual editor</button><button :class="{active:tab==='simulation'}" @click="navigate('simulation')">Simulation</button><button :class="{active:tab==='glossary'}" @click="navigate('glossary')">Glossary</button></nav><span class="header-tag">BROWSER-BASED CONTROLS LAB</span></header>

    <main v-if="tab==='primer'"><PrimerPage/><section class="cta-band"><div><span class="eyebrow">READY TO EXPERIMENT?</span><h2>Turn the sequence into a working model.</h2></div><button class="solid-button" @click="navigate('editor')">Open the visual editor ↗</button></section></main>

    <main v-else-if="tab==='glossary'"><GlossaryPage/></main>

    <main v-else-if="tab==='editor'" class="workspace-page"><div class="workspace-heading"><div><span class="eyebrow">WORKSPACE / CONTROL GRAPH</span><h1>{{ project?.name ?? 'Loading controls…' }}</h1><p>Build a sequence from blocks, tune parameters, and test its outputs.</p></div><div class="workspace-actions"><select v-model="exampleChoice" aria-label="Load example"><option value="">Load an example…</option><option v-for="example in examples" :key="example.file" :value="example.file">{{ example.label }}</option></select><button class="outline-button" @click="loadExample">Load</button><button class="outline-button" @click="triggerImport">Import JSON / CXF</button><button class="outline-button" :disabled="!project" @click="exportProject">Export project</button><button class="outline-button" :disabled="!project" @click="exportCxfFile">Export CXF</button><button v-if="project?.graph.brick_model" class="outline-button" @click="exportBrickFile">Export Brick model</button><input ref="fileInput" class="sr-only" type="file" accept="application/json,application/ld+json,.json,.jsonld" @change="importFile"/></div></div>
      <div v-if="storageWarning" class="error-banner">{{ storageWarning }} <button v-if="recoverableProject" class="outline-button" type="button" @click="downloadSavedProject">Download saved project</button></div><div v-if="!project && errors.length" class="error-banner">{{ errors.join("; ") }}</div>
      <div v-if="project" class="workspace-body"><div class="validation-strip" :class="{invalid:errors.length}"><span>{{ errors.length ? `● ${errors.length} graph issue${errors.length===1?'':'s'}` : '● Graph ready' }}</span><span>{{ project.graph.blocks?.length ?? 0 }} top-level blocks · Local autosave</span></div><div v-if="errors.length" class="validation-list"><p v-for="(error,i) in errors.slice(0,8)" :key="i">{{ error }}</p></div><BuildingSpaces v-if="project.graph.brick_model" :model="project.graph.brick_model"/><GraphEditor :key="project.graph.name" :project="project" @change="updateProject"/><section v-if="!isBuildingGraph" class="try-panel"><div><span class="eyebrow">QUICK TEST</span><h2>Run one control step</h2><p>Supply values for the imported sequence and inspect its output.</p></div><div class="try-inputs"><label v-for="input in project.graph.inputs ?? []" :key="input.name" class="field"><span>{{ input.name }} <small>{{ input.unit }}</small></span><select v-if="input.type==='Boolean'" v-model="testInputs[input.name]"><option :value="true">True</option><option :value="false">False</option></select><input v-else type="number" step="any" v-model.number="testInputs[input.name]"/></label></div><button class="solid-button" :disabled="errors.length>0" @click="runStep">Run step</button><pre v-if="testResult">{{ testResult }}</pre></section></div>
    </main>

    <main v-else class="simulation-page"><section class="simulation-hero"><div><span class="eyebrow">THE FOUR-PIPE BUILDING</span><h1>One year. Ten zones. Your control logic.</h1><p>A documented synthetic mixed-climate year drives two AHUs, ten VAVs, and shared hot and chilled water loops. Load the conventional or G36-style sample in the editor, then run it against the same building model.</p><div class="hero-actions"><AceButton :disabled="!project || !isBuildingGraph || errors.length>0 || running" @click="runYear">{{ running ? 'Running…' : 'Run 2025 simulation' }}</AceButton><button v-if="running" class="outline-button" @click="cancelRun">Cancel</button><button class="outline-button" @click="navigate('editor')">Edit controls ↗</button></div><p v-if="!isBuildingGraph" class="simulation-notice">Load a Four-pipe building example in the editor to run the building simulation.</p></div><div class="hero-stat"><span>2025 SIMULATION</span><strong>{{ number(totalSteps) }}</strong><small>{{ stepLabel }} steps · 35,040 weather samples</small></div></section>
      <div class="sim-layout"><aside class="sim-side"><div class="panel-title"><span class="eyebrow">SCENARIO</span><h2>Building assumptions</h2></div><p class="muted small">These simplified thermal assumptions apply to all ten zones.</p><label class="field"><span>Maximum simulation timestep</span><select v-model.number="scenario.step_seconds" :disabled="running" @change="saveScenario"><option :value="900">15 minutes</option><option :value="300">5 minutes</option><option :value="60">1 minute</option><option :value="30">30 seconds</option><option :value="15">15 seconds</option></select></label><p class="muted small">Weather updates every 15 minutes. The controls and building physics run at the selected maximum step or a smaller step required by sampled CDL blocks. Charts record one value per weather interval.</p><p v-if="effectiveStepSeconds<scenario.step_seconds" class="muted small">This graph contains sampled CDL blocks. The runtime uses {{ effectiveStepSeconds }}-second internal steps to resolve their timing.</p><label class="field"><span>Zone envelope · kW/K</span><input v-model.number="scenario.zone_ua_kw_per_k" type="number" min="0.01" max="2" step="0.01" @change="saveScenario"/></label><label class="field"><span>Thermal capacity · kWh/K</span><input v-model.number="scenario.zone_capacity_kwh_per_k" type="number" min="0.1" max="50" step="0.1" @change="saveScenario"/></label><details class="field"><summary>Zone CO₂ physical assumptions</summary><p class="muted small">The simulator mixes outdoor and return air, adds CO₂ from raw presence, and sends the measured concentration to CDL. Ventilation commands are calculated in the editable graph. One normalized airflow unit is treated as 1 m³/s.</p><label><span>Zone air volume · m³</span><input v-model.number="scenario.zone_air_volume_m3" type="number" min="1" max="10000" step="10" @change="saveScenario"/></label><label><span>Infiltration · m³/s per zone</span><input v-model.number="scenario.zone_infiltration_m3_s" type="number" min="0" max="5" step="0.005" @change="saveScenario"/></label><label><span>People per present zone</span><input v-model.number="scenario.design_occupants_per_zone" type="number" min="0" max="1000" step="1" @change="saveScenario"/></label><label><span>CO₂ generation · L/s per person</span><input v-model.number="scenario.co2_generation_lps_per_person" type="number" min="0" max="0.1" step="0.001" @change="saveScenario"/></label><label><span>Outdoor CO₂ · ppm</span><input v-model.number="scenario.outdoor_co2_ppm" type="number" min="250" max="2000" step="10" @change="saveScenario"/></label></details><p class="muted small">Tune heat pump dispatch in the plant composite’s <code>{{ isG36Sample ? "heat_pump_utilization_limit" : "heat_pump_dispatch_fraction" }}</code> CDL block in the visual editor. The performance points below describe physical equipment capacity and efficiency; they do not choose operating modes.</p><label class="field"><span>Heat pump capacity · kW</span><input v-model.number="scenario.heat_pump_capacity_kw" type="number" min="1" step="5" @change="saveScenario"/></label><details class="field"><summary>Heat pump performance vs outdoor temperature</summary><p class="muted small">Synthetic equipment data. Capacity is a fraction of nameplate; COP values set compressor electricity. Values interpolate between points and hold outside the listed range. Keep outdoor temperatures increasing.</p><div v-for="(point,index) in scenario.heat_pump_performance" :key="index" class="performance-point"><label><span>Outdoor · °C</span><input v-model.number="point.outdoor_c" type="number" step="1" :disabled="running" @change="saveScenario"/></label><label><span>Capacity fraction</span><input v-model.number="point.capacity_fraction" type="number" min="0.01" max="1" step="0.05" :disabled="running" @change="saveScenario"/></label><label><span>Heating COP</span><input v-model.number="point.heating_cop" type="number" min="0.1" step="0.1" :disabled="running" @change="saveScenario"/></label><label><span>Cooling COP</span><input v-model.number="point.cooling_cop" type="number" min="0.1" step="0.1" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running || scenario.heat_pump_performance.length<=1" @click="removeHeatPumpPoint(index)">Remove point</button></div><button class="outline-button" type="button" :disabled="running" @click="addHeatPumpPoint">Add point</button></details><label class="field"><span>Boiler capacity · kW</span><input v-model.number="scenario.boiler_capacity_kw" type="number" min="1" step="5" @change="saveScenario"/></label><label class="field"><span>Chiller capacity · kW</span><input v-model.number="scenario.chiller_capacity_kw" type="number" min="1" step="5" @change="saveScenario"/></label><details class="field"><summary>Equipment outage tests</summary><p class="muted small">Inject raw availability status for the heat pump, boiler, or chiller. The G36 plant CDL graph decides how to dispatch the sources that remain available.</p><p class="muted small">Physical startup delays hold source delivery and running proof after a CDL command. The plant graph decides when proven stages may release water-reset holds.</p><label class="field"><span>Heat pump start delay · seconds</span><input v-model.number="scenario.equipment_startup_seconds.heat_pump" type="number" min="0" step="15" :disabled="running" @change="saveScenario"/></label><label class="field"><span>Boiler start delay · seconds</span><input v-model.number="scenario.equipment_startup_seconds.boiler" type="number" min="0" step="15" :disabled="running" @change="saveScenario"/></label><label class="field"><span>Chiller start delay · seconds</span><input v-model.number="scenario.equipment_startup_seconds.chiller" type="number" min="0" step="15" :disabled="running" @change="saveScenario"/></label><div v-for="(outage,index) in scenario.equipment_outages" :key="index" class="outage-entry"><label><span>Equipment</span><select v-model="outage.equipment" :disabled="running" @change="saveScenario"><option value="heat_pump">Heat pump</option><option value="boiler">Boiler</option><option value="chiller">Chiller</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="outage.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="outage.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeEquipmentOutage(index)">Remove outage</button></div><button class="outline-button" type="button" :disabled="running" @click="addEquipmentOutage">Add outage</button></details><details class="field"><summary>Source failure to start tests</summary><p class="muted small">The equipment stays available but cannot prove running during the test interval. The G36 plant CDL blocks diagnose each source after an editable timeout. A heat-pump fault moves its load to backup sources; each latched source fault inhibits further commands until reset.</p><div v-for="(failure,index) in scenario.equipment_start_failures" :key="index" class="outage-entry"><label><span>Equipment</span><select v-model="failure.equipment" :disabled="running" @change="saveScenario"><option value="heat_pump">Heat pump</option><option value="boiler">Boiler</option><option value="chiller">Chiller</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="failure.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="failure.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeStartFailure(index)">Remove failure</button></div><button class="outline-button" type="button" :disabled="running" @click="addStartFailure">Add failure</button><label class="field"><span>Heat pump operator reset · elapsed hours</span><input v-model.number="scenario.heat_pump_fault_reset_hour" type="number" min="-1" step="0.25" :disabled="running" @change="saveScenario"/></label><label class="field"><span>Boiler operator reset · elapsed hours</span><input v-model.number="scenario.boiler_fault_reset_hour" type="number" min="-1" step="0.25" :disabled="running" @change="saveScenario"/></label><label class="field"><span>Chiller operator reset · elapsed hours</span><input v-model.number="scenario.chiller_fault_reset_hour" type="number" min="-1" step="0.25" :disabled="running" @change="saveScenario"/></label></details><details class="field" v-if="isG36Sample"><summary>Supply fan fault tests</summary><p class="muted small">Inject a fan motor failure, stuck-on motor, or drive speed cap. The physical model changes airflow and fan energy; CDL compares command with measured status and speed.</p><div v-for="(fault,index) in scenario.fan_faults" :key="index" class="outage-entry"><label><span>Fault</span><select v-model="fault.equipment" :disabled="running" @change="saveScenario"><option value="ahu_1_stuck_off">AHU 1 fan fails off</option><option value="ahu_1_stuck_on">AHU 1 fan stuck on</option><option value="ahu_2_stuck_off">AHU 2 fan fails off</option><option value="ahu_2_stuck_on">AHU 2 fan stuck on</option><option value="ahu_1_speed_capped">AHU 1 fan speed capped at 30%</option><option value="ahu_2_speed_capped">AHU 2 fan speed capped at 30%</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="fault.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="fault.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeFanFault(index)">Remove fault</button></div><button class="outline-button" type="button" :disabled="running" @click="addFanFault">Add fan fault</button></details><details class="field" v-if="isG36Sample"><summary>Outdoor temperature disturbances</summary><p class="muted small">Add a short physical temperature change between 15-minute weather samples. The same temperature feeds the outside-air sensor, envelope, mixed-air path, and equipment performance.</p><div v-for="(event,index) in scenario.outdoor_temperature_events" :key="index" class="outage-entry"><label><span>Change · °C</span><input v-model.number="event.delta_c" type="number" min="-25" max="25" step="0.5" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="event.start_hour" type="number" min="0" step="0.0166667" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="event.duration_hours" type="number" min="0.0041667" step="0.0166667" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeOutdoorTemperatureEvent(index)">Remove disturbance</button></div><button class="outline-button" type="button" :disabled="running" @click="addOutdoorTemperatureEvent">Add disturbance</button></details><details class="field" v-if="isG36Sample"><summary>AHU damper fault tests</summary><p class="muted small">Hold an outdoor or return damper at a physical position. Measured airflow and mixing respond to the fault; editable CDL checks position tracking.</p><div v-for="(fault,index) in scenario.ahu_damper_faults" :key="index" class="outage-entry"><label><span>AHU</span><select v-model.number="fault.ahu" :disabled="running" @change="saveScenario"><option :value="1">AHU 1</option><option :value="2">AHU 2</option></select></label><label><span>Damper</span><select v-model="fault.damper" :disabled="running" @change="saveScenario"><option value="outdoor">Outdoor air</option><option value="return">Return air</option></select></label><label><span>Stuck position · 0–1</span><input v-model.number="fault.stuck_position" type="number" min="0" max="1" step="0.1" :disabled="running" @change="saveScenario"/></label><label><span>Start · elapsed hours</span><input v-model.number="fault.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="fault.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeAhuDamperFault(index)">Remove fault</button></div><button class="outline-button" type="button" :disabled="running" @click="addAhuDamperFault">Add AHU damper fault</button></details><details class="field" v-if="isG36Sample"><summary>VAV actuator fault tests</summary><p class="muted small">Inject a physical damper stuck open or a reheat valve seat leak. The fault changes actual airflow or heat transfer; editable CDL blocks decide the alarm level from measured points.</p><div v-for="(fault,index) in scenario.terminal_faults" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="fault.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>Fault</span><select v-model="fault.kind" :disabled="running" @change="saveScenario"><option value="damper_stuck_open">Damper stuck open</option><option value="reheat_valve_leak">Reheat valve leaks while closed</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="fault.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="fault.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeTerminalFault(index)">Remove fault</button></div><button class="outline-button" type="button" :disabled="running" @click="addTerminalFault">Add VAV fault</button></details><details class="field"><summary>Freeze-stat test inputs</summary><p class="muted small">Inject a raw AHU freeze-stat trip and an operator reset pulse. The editable CDL graph decides the shutdown and recovery.</p><label class="field"><span>Test AHU</span><select v-model.number="scenario.freeze_test_ahu" :disabled="running" @change="saveScenario"><option :value="0">Off</option><option :value="1">AHU 1</option><option :value="2">AHU 2</option></select></label><label class="field"><span>Trip starts · elapsed hours</span><input v-model.number="scenario.freeze_test_start_hour" type="number" min="-1" step="0.25" @change="saveScenario"/></label><label class="field"><span>Trip duration · hours</span><input v-model.number="scenario.freeze_test_duration_hours" type="number" min="0" step="0.25" @change="saveScenario"/></label><label class="field"><span>Operator reset · elapsed hours</span><input v-model.number="scenario.freeze_test_reset_hour" type="number" min="-1" step="0.25" @change="saveScenario"/></label></details><details class="field" v-if="isG36Sample"><summary>Window contact tests</summary><p class="muted small">Inject raw window contacts. CDL excludes an open zone from ordinary group decisions and turns off terminal conditioning while retaining freeze protection.</p><div v-for="(opening,index) in scenario.window_openings" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="opening.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="opening.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="opening.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeWindowOpening(index)">Remove opening</button></div><button class="outline-button" type="button" :disabled="running" @click="addWindowOpening">Add opening</button></details><details class="field" v-if="isG36Sample"><summary>Local occupancy override tests</summary><p class="muted small">Inject raw local override button contacts. Editable CDL holds each press for two hours by default and makes the affected zone and AHU group occupied.</p><div v-for="(overrideEvent,index) in scenario.occupancy_overrides" :key="index" class="outage-entry"><label><span>Zone</span><select v-model.number="overrideEvent.zone" :disabled="running" @change="saveScenario"><option v-for="zone in 10" :key="zone" :value="zone">Zone {{ zone }}</option></select></label><label><span>Start · elapsed hours</span><input v-model.number="overrideEvent.start_hour" type="number" min="0" step="0.25" :disabled="running" @change="saveScenario"/></label><label><span>Duration · hours</span><input v-model.number="overrideEvent.duration_hours" type="number" min="0.25" step="0.25" :disabled="running" @change="saveScenario"/></label><button class="outline-button" type="button" :disabled="running" @click="removeOccupancyOverride(index)">Remove override</button></div><button class="outline-button" type="button" :disabled="running" @click="addOccupancyOverride">Add override</button></details><div class="assumption-note">Each AHU has a 20% minimum outdoor-air share; an editable G36-style sequence can increase it for free cooling. Fan capacity determines delivered airflow and normalized duct pressure. Graph-commanded water pumps and source output charge mixed water volumes; flow and measured supply temperature limit coil and reheat delivery.</div></aside>
      <div class="sim-content"><div class="sim-status"><div><span class="status-dot">● {{ running ? 'Running in your browser' : results.length ? 'Custom run complete' : 'Bundled baseline' }}</span><strong>{{ running ? `${Math.round(progress/totalSteps*100)}% complete` : '2025 · January → December' }}</strong></div><div v-if="running" class="progress-track"><i :style="{width:`${progress/totalSteps*100}%`} "/></div></div><div v-if="simError" class="error-banner">{{ simError }}</div>
      <div class="chart-card signal-card">
        <div class="card-heading"><div><span class="eyebrow">CONTROL SIGNAL EXPLORER</span><h2>Compare process variables</h2></div><span class="muted small">{{ signalOptions.length }} selectable signals</span></div>
        <div class="signal-picker"><label class="field"><span>Find a variable or connection</span><input v-model="signalSearch" type="search" placeholder="Search block, port, Brick class, or unit…" @input="candidateSignal=''"/></label><label class="field"><span>Signal</span><select v-model="candidateSignal" :disabled="running || !isBuildingGraph"><option value="">Choose a signal…</option><option v-for="signal in filteredSignals" :key="signal.id" :value="signal.id">{{ signal.kind==='physical' ? 'Physical · ' : signal.kind==='connection' ? 'Connection · ' : signal.kind==='input' ? 'Input · ' : 'Output · ' }}{{ signal.label }} ({{ signal.unit }}){{ signal.brickClass ? ` · brick:${signal.brickClass}` : '' }}</option></select></label><button class="outline-button signal-add" type="button" :disabled="!candidateSignal || running" @click="addSignal">Add signal</button></div>
        <div v-if="selectedSignalDetails.length" class="signal-selections"><div v-for="signal in selectedSignalDetails" :key="signal.id" class="signal-chip"><span><b>{{ signal.unit }}</b>{{ signal.label }}<small v-if="signal.brickClass"> · brick:{{ signal.brickClass }}</small></span><button type="button" :aria-label="`Remove ${signal.label}`" :disabled="running" @click="removeSignal(signal.id)">×</button></div></div>
        <p v-if="!selectedSignalDetails.length" class="muted small">Add several graph or physical signals. Each unit type gets its own plot with a labeled y-axis.</p>
        <div v-else-if="needsSignalRun" class="signal-run"><span>{{ results.length ? 'New graph signals need a fresh run.' : 'Run the current model to record your selected signals.' }}</span><button class="solid-button" type="button" :disabled="running || !isBuildingGraph || errors.length>0" @click="runYear">{{ running ? 'Running…' : 'Run selected signals' }}</button></div>
      </div>
      <div class="time-controls timeline-toolbar"><label class="field"><span>From · UTC</span><input type="datetime-local" min="2025-01-01T00:00" max="2025-12-31T23:45" step="900" :value="timeValue(timeStart)" @change="setTimeRange(timeIndex(($event.target as HTMLInputElement).value),timeEnd)"/></label><label class="field"><span>To · UTC</span><input type="datetime-local" min="2025-01-01T00:00" max="2025-12-31T23:45" step="900" :value="timeValue(timeEnd)" @change="setTimeRange(timeStart,timeIndex(($event.target as HTMLInputElement).value))"/></label><div class="time-zoom"><button class="outline-button" type="button" aria-label="Pan time earlier" :disabled="timeStart===0" @click="shiftTime(-1)">←</button><button class="outline-button" type="button" aria-label="Zoom time out" @click="zoomTime(2)">−</button><button class="outline-button" type="button" aria-label="Reset time range" @click="resetTimeRange">Full year</button><button class="outline-button" type="button" aria-label="Zoom time in" @click="zoomTime(0.5)">+</button><button class="outline-button" type="button" aria-label="Pan time later" :disabled="timeEnd===35039" @click="shiftTime(1)">→</button><button class="outline-button" :class="{active:panMode}" type="button" :aria-pressed="panMode" :disabled="timeStart===0&&timeEnd===35039" @click="panMode=!panMode">Pan charts</button></div></div>
      <SignalChart v-for="group in plotGroups" :key="group.unit" :rows="results" :version="rowVersion" :pan-mode="panMode" :signals="group.signals" :unit="group.unit" :start="timeStart" :end="timeEnd" @range="setTimeRange" @pan="panTimeTo"/>
      <div v-if="isG36Sample && results.length && comparisonBaselineSummary" class="chart-card comparison-card"><div class="card-heading"><div><span class="eyebrow">CONTROL COMPARISON</span><h2>G36-style vs conventional</h2></div><span class="muted small">SAME 2025 WEATHER · TEN ZONES</span></div><p v-if="!defaultComparisonScenario" class="muted small">The bundled conventional reference uses default building assumptions. Use the default assumptions and a 60-second effective timestep to compare like for like.</p><div v-else class="comparison-grid"><div v-for="metric in comparisonMetrics" :key="metric.label"><span>{{ metric.label }}</span><strong>{{ deltaPercent(metric.current,metric.baseline) }}</strong><small>{{ number(metric.current) }} current · {{ number(metric.baseline) }} conventional {{ metric.unit }}</small></div></div><p class="muted small">This sample includes occupancy setpoint resets and temperature-enabled, mixed-air-feedback outdoor-air modulation. It includes editable AHU coil valve loops, sampled duct-pressure and supply-air temperature resets, teaching alarms, and sampled hot/chilled-water supply resets with VAV reheat requests, but still omits full G36 economizer limits, plant staging, and the full G36 alarm set. Compare comfort as well as energy.</p></div>
      <div class="metric-grid"><article><span>ELECTRICITY</span><strong>{{ number(summary.electricity_kwh) }}</strong><small>kWh · heat pump, chiller, fans</small></article><article><span>BOILER FUEL</span><strong>{{ number(summary.gas_kwh) }}</strong><small>kWh input</small></article><article><span>COMFORT BAND EXCEEDANCE</span><strong>{{ number(summary.unmet_zone_hours) }}</strong><small>zone-hours · 20–25°C occupied / 17–29°C otherwise</small></article><article><span>AVG ZONE TEMP</span><strong>{{ latestRow?.avg_zone_c.toFixed(1) ?? '—' }}°C</strong><small>latest time step</small></article></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">TEMPERATURE RESPONSE</span><h2>Zone and outdoor temperature</h2></div><span class="muted small">{{ results.length ? 'CUSTOM RUN' : 'BASELINE' }}</span></div><TrendChart v-if="displayRows.length" :rows="displayRows" :version="rowVersion" :start="timeStart" :end="timeEnd" field-a="avg_zone_c" field-b="outdoor_c" unit="°C" label-a="Average zone" label-b="Outdoor air" @pan="panTimeTo"/></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">AIR DELIVERY</span><h2>VAV request and fan-delivered airflow</h2></div><span class="muted small">NORMALIZED FLOW UNITS</span></div><TrendChart v-if="displayRows.length" :rows="displayRows" :version="rowVersion" :start="timeStart" :end="timeEnd" field-a="requested_airflow" field-b="delivered_airflow" unit="normalized airflow" :zero-based="true" label-a="VAV request" label-b="Fan delivered" @pan="panTimeTo"/></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">COIL RESPONSE</span><h2>AHU 1 supply air</h2></div><span class="muted small">TARGET / ACTUAL</span></div><TrendChart v-if="displayRows.length" :rows="displayRows" :version="rowVersion" :start="timeStart" :end="timeEnd" field-a="ahu_1_supply_setpoint_c" field-b="ahu_1_supply_actual_c" unit="°C" label-a="Target · °C" label-b="Actual · °C" @pan="panTimeTo"/></div><div class="chart-card"><div class="card-heading"><div><span class="eyebrow">ENERGY PROFILE</span><h2>Electricity and boiler fuel</h2></div><button v-if="results.length" class="text-button" @click="exportResults">Download CSV ↓</button></div><TrendChart v-if="displayRows.length" :rows="displayRows" :version="rowVersion" :start="timeStart" :end="timeEnd" field-a="electric_kw" field-b="gas_kw" unit="kW" :zero-based="true" label-a="Electric · kW" label-b="Gas · kW" @pan="panTimeTo"/></div><div class="zone-card"><div class="card-heading"><div><span class="eyebrow">ZONE SNAPSHOT</span><h2>Latest temperatures</h2></div><span class="muted small">AHU 1 · Z1–5 / AHU 2 · Z6–10</span></div><div class="zone-grid"><div v-for="(temp,i) in latestRow?.zone_c ?? []" :key="i"><span>ZONE {{ String(i+1).padStart(2,'0') }}</span><strong>{{ temp.toFixed(1) }}°C</strong></div></div></div></div></div>
    </main>
    <footer class="site-footer"><span>ACE IoT · CDL Explorer</span><span>Simplified educational simulation · Runs locally in your browser</span><a href="https://github.com/ACE-IoT-Solutions/python-cdl" target="_blank" rel="noreferrer">Python CDL reference ↗</a></footer>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, shallowRef, triggerRef } from 'vue'
import PrimerPage from './components/PrimerPage.vue'
import GlossaryPage from './components/GlossaryPage.vue'
import GraphEditor from './components/GraphEditor.vue'
import TrendChart from './components/TrendChart.vue'
import SignalChart from './components/SignalChart.vue'
import AceButton from './vendor/ace/AceButton.vue'
import { defaultScenario, edgeEndpoints, effectiveStepForGraph, layoutFor, type Block, type Project, type Row, type Scenario, type Summary } from './lib/types'
import { exportCxf, importCxf, type CxfDocument } from './lib/cxf'
import { exportBrickModel, migrateBrickModel, validateBrickModel } from './lib/brickModel'
import { plainWorkerData, scenarioForWorker } from './lib/workerPayload'
import { storeMigrationBackup, storeProject } from './lib/projectStorage'
import BuildingSpaces from './components/BuildingSpaces.vue'
import { graphSignals, physicalSignalsForGraph, type GraphSignal } from './lib/signals'
import { migrateG36Timing, migrateG36TerminalFeedback, migrateG36AhuFeedback, migrateG36PressureTrim, migrateG36Economizer, migrateG36VacancyDelay } from './lib/g36Migration'
import { migrateOutdoorAirCommand, migrateCoilControl } from './lib/airCommandMigration'
import { migrateWaterLoopControl } from './lib/waterLoopMigration'
import { migrateG36WaterReset } from './lib/g36WaterResetMigration'
import { migrateG36HotWaterReset } from './lib/g36HotWaterResetMigration'
import { migrateG36TerminalHotRequests } from './lib/g36TerminalHotRequestMigration'
import { migrateG36SupplyAirReset } from './lib/g36SupplyAirResetMigration'
import { migrateG36TerminalDischarge } from './lib/g36TerminalDischargeMigration'
import { migrateG36DamperFeedback } from './lib/g36DamperFeedbackMigration'
import { migrateG36FreezeStageOne } from './lib/g36FreezeStageOneMigration'
import { migrateG36EconomizerEnable } from './lib/g36EconomizerEnableMigration'
import { migrateG36OutdoorAirLimits } from './lib/g36OutdoorAirLimitsMigration'
import { migrateG36ReturnDamper } from './lib/g36ReturnDamperMigration'
import { migrateG36CommonDamperMapping } from './lib/g36CommonDamperMappingMigration'
import { migrateG36Ventilation } from './lib/g36VentilationMigration'
import { migrateG36WaterSourceProof } from './lib/g36WaterSourceProofMigration'
import { migrateG36WaterPressure } from './lib/g36WaterPressureMigration'
import { migrateG36ChilledWaterMapping } from './lib/g36ChilledWaterMappingMigration'
import { migrateConventionalSourceEnable } from './lib/conventionalSourceEnableMigration'
import { migrateWaterDispatch } from './lib/waterDispatchMigration'
import { migrateG36WaterStageHold } from './lib/g36WaterStageHoldMigration'
import { migrateG36HybridDispatch } from './lib/g36HybridDispatchMigration'
import { migrateG36EquipmentAvailability } from './lib/g36EquipmentAvailabilityMigration'
import { migrateG36StageFeedback } from './lib/g36StageFeedbackMigration'
import { migrateG36StartFault } from './lib/g36StartFaultMigration'
import { migrateG36BackupStartFault } from './lib/g36BackupStartFaultMigration'
import { migrateG36ZoneModes } from './lib/g36ZoneModesMigration'
import { migrateG36ZoneGroups } from './lib/g36ZoneGroupMigration'
import { migrateG36OptimalStart, needsG36OptimalStartMigration } from './lib/g36OptimalStartMigration'
import { migrateG36DailyRecovery, needsG36DailyRecoveryMigration } from './lib/g36DailyRecoveryMigration'
import { migrateG36Co2, needsG36Co2Migration } from './lib/g36Co2Migration'
import { migrateG36GroupUnoccupied } from './lib/g36GroupUnoccupiedMigration'
import { migrateG36Windows } from './lib/g36WindowMigration'
import { migrateG36Overrides } from './lib/g36OverrideMigration'
import { migrateG36TimedOverrides, needsG36TimedOverrideMigration } from './lib/g36TimedOverrideMigration'
import { migrateG36TerminalAirflowAlarms, needsG36TerminalAirflowAlarmMigration } from './lib/g36TerminalAirflowAlarmMigration'
import { migrateG36SupplyFanAlarms, needsG36SupplyFanAlarmMigration } from './lib/g36SupplyFanAlarmMigration'
import { migrateG36AirflowSensorAlarms, needsG36AirflowSensorAlarmMigration } from './lib/g36AirflowSensorAlarmMigration'
import { migrateG36LowDischargeAlarms, needsG36LowDischargeAlarmMigration } from './lib/g36LowDischargeAlarmMigration'
import { migrateG36TerminalLeakAlarms, needsG36TerminalLeakAlarmMigration } from './lib/g36TerminalLeakAlarmMigration'
import { migrateG36FanSpeedFeedback, needsG36FanSpeedFeedbackMigration } from './lib/g36FanSpeedFeedbackMigration'
import { migrateG36AhuDamperFeedback, needsG36AhuDamperFeedbackMigration } from './lib/g36AhuDamperFeedbackMigration'
import { migrateG36EconomizerHold, needsG36EconomizerHoldMigration } from './lib/g36EconomizerHoldMigration'
import { migrateG36EconomizerDisableTransition, needsG36EconomizerDisableTransitionMigration } from './lib/g36EconomizerDisableTransitionMigration'
import { migrateG36FixedDryBulb, needsG36FixedDryBulbMigration } from './lib/g36FixedDryBulbMigration'
import { migrateG36CoolingStartup } from './lib/g36CoolingStartupMigration'
import { migrateWaterEnergyControl } from './lib/waterEnergyMigration'
import { ArrayRows, ArrowRows } from './lib/frames'
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
const examples=[{file:'four_pipe_building.json',label:'Four-pipe building · conventional controls'},{file:'four_pipe_g36.json',label:'Four-pipe building · G36-style controls'},{file:'vav_composite_template.json',label:'VAV composite · editable blocks'},{file:'p_controller_limiter.json',label:'P controller with limiter'},{file:'room_control_system.json',label:'Dual-mode room controller'},{file:'simple_temp_controller.json',label:'Basic temperature controller'},...['east','north','south','west','corridor'].map(x=>({file:`${x}_controller.json`,label:`Legacy VAV controller · ${x}`}))]
const testInputs=reactive<Record<string,number|boolean>>({})
const testResult=ref('')
const scenario=reactive<Scenario>(structuredClone(defaultScenario))
const effectiveStepSeconds=computed(()=>project.value?effectiveStepForGraph(project.value.graph,scenario.step_seconds):scenario.step_seconds)
const totalSteps=computed(()=>35_040*900/effectiveStepSeconds.value)
const stepLabel=computed(()=>effectiveStepSeconds.value>=60?`${effectiveStepSeconds.value/60}-minute`:`${effectiveStepSeconds.value}-second`)
const baseline=shallowRef<Row[]>([])
const baselineSummary=ref<Summary>({electricity_kwh:0,gas_kwh:0,unmet_zone_hours:0,occupied_unmet_zone_hours:0})
const comparisonBaselineSummary=ref<Summary|null>(null)
const results=shallowRef(new ArrowRows([]))
const rowVersion=ref(0)
const resultSummary=ref<Summary|null>(null)
const running=ref(false),progress=ref(0),simError=ref('')
const panMode=ref(false)
const signalSearch=ref(''),candidateSignal=ref(''),selectedSignals=ref<string[]>([]),activeTraceIds=ref<string[]>([]),timeStart=ref(0),timeEnd=ref(35039)
let lastResultRefresh=0
let jobId=0
let activeRunId:number|null=null
const isBuildingGraph=computed(()=>{const blocks=project.value?.graph.blocks??[];return blocks.filter(b=>b.block_type==='VAVBoxController'&&b.blocks?.length).length===10&&blocks.filter(b=>b.block_type==='AHUController').length===2&&blocks.some(b=>b.block_type==='PlantDispatch')})
const isG36Sample=computed(()=>project.value?.graph.name==='FourPipeG36Controls')
const defaultComparisonScenario=computed(()=>effectiveStepSeconds.value===60&&Object.entries(defaultScenario).every(([key,value])=>key==='step_seconds'||JSON.stringify(scenario[key as keyof Scenario])===JSON.stringify(value)))
const comparisonMetrics=computed(()=>resultSummary.value?[
  {label:'Electricity',current:resultSummary.value.electricity_kwh,baseline:comparisonBaselineSummary.value?.electricity_kwh??0,unit:'kWh'},
  {label:'Boiler fuel',current:resultSummary.value.gas_kwh,baseline:comparisonBaselineSummary.value?.gas_kwh??0,unit:'kWh'},
  {label:'Present-zone comfort exceedance',current:resultSummary.value.occupied_unmet_zone_hours,baseline:comparisonBaselineSummary.value?.occupied_unmet_zone_hours??0,unit:'zone-hours'},
]:[])
const deltaPercent=(current:number,reference:number)=>reference>0?`${current>=reference?'+':''}${((current/reference-1)*100).toFixed(1)}%`:'—'
const baselineRows=computed(()=>new ArrayRows(baseline.value))
const displayRows=computed(()=>results.value.length?results.value:baselineRows.value)
const summary=computed(()=>resultSummary.value??baselineSummary.value)
const latestRow=computed(()=>{rowVersion.value;return displayRows.value.at(-1)})
const signalOptions=computed(()=>project.value?[...physicalSignalsForGraph(project.value.graph),...graphSignals(project.value.graph)]:[])
const filteredSignals=computed(()=>{const query=signalSearch.value.trim().toLowerCase();return signalOptions.value.filter(signal=>!selectedSignals.value.includes(signal.id)&&(!query||`${signal.label} ${signal.unit} ${signal.brickClass??''}`.toLowerCase().includes(query)))})
const selectedSignalDetails=computed(()=>selectedSignals.value.flatMap(id=>{const signal=signalOptions.value.find(item=>item.id===id);return signal?[signal]:[]}))
const needsSignalRun=computed(()=>selectedSignals.value.length>0&&(!results.value.length||selectedSignalDetails.value.some(signal=>signal.kind!=='physical'&&!activeTraceIds.value.includes(signal.id))))
const plotGroups=computed(()=>{if(!results.value.length)return [];const groups=new Map<string,GraphSignal[]>();for(const signal of selectedSignalDetails.value){if(signal.kind!=='physical'&&!activeTraceIds.value.includes(signal.id))continue;const series=groups.get(signal.unit)??[];series.push(signal);groups.set(signal.unit,series)}return [...groups].map(([unit,signals])=>({unit,signals}))})
const number=(n:number)=>Math.round(n).toLocaleString()
const asset=(name:string)=>new URL(`examples/${name}`,document.baseURI).toString()
async function fetchJson<T>(name:string):Promise<T>{const response=await fetch(asset(name));if(!response.ok)throw new Error(`Could not load ${name}`);return response.json() as Promise<T>}
function syncInputs(graph:Block){for(const key of Object.keys(testInputs))delete testInputs[key];for(const input of graph.inputs??[])testInputs[input.name]=input.type==='Boolean'?false:input.name.toLowerCase().includes('max')?20:input.name.toLowerCase().includes('temp')?22:1;testResult.value=''}
function updateProject(next:Project){project.value=next;results.value=new ArrowRows([]);resultSummary.value=null;activeTraceIds.value=[];const saved=storeProject(localStorage,next);if(saved)recoverableProject.value=null;storageWarning.value=saved?'':'Browser storage is full. Export your project to preserve these edits.';validateGraph(next.graph)}
async function fillExampleMetadata(saved:Project):Promise<Project>{
  const example=saved.graph.name==='FourPipeBuildingControls'?'four_pipe_building.json':saved.graph.name==='FourPipeG36Controls'?'four_pipe_g36.json':null
  if(!example)return saved
  try{
    const reference=await fetchJson<Block>(example)
    const next=migrateBrickModel(structuredClone(saved),reference)
    let changed=!saved.graph.brick_model&&!!next.graph.brick_model
    function visit(block:Block,source:Block){
      if(block.notes===undefined&&source.notes){block.notes=source.notes;changed=true}
      for(const direction of ['inputs','outputs'] as const){
        for(const port of block[direction]??[]){
          const reference=source[direction]?.find(item=>item.name===port.name)
          if(!reference)continue
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
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='pressure_previous'))return saved
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36PressureTrim(saved,reference)
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
  if(hasEnergy&&!needsGate)return saved
  if(needsGate)storeMigrationBackup(localStorage,'ace-cdl-pre-source-enable-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>(saved.graph.name==='FourPipeG36Controls'?'four_pipe_g36.json':'four_pipe_building.json')
  const withEnergy=hasEnergy?saved:migrateWaterEnergyControl(saved,reference)
  return needsGate?migrateConventionalSourceEnable(withEnergy,reference):withEnergy
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
  const needsAlarm=needsG36TerminalAirflowAlarmMigration(saved)
  const needsFanAlarm=needsG36SupplyFanAlarmMigration(saved)
  const needsSensorAlarm=needsG36AirflowSensorAlarmMigration(saved)
  const needsDischargeAlarm=needsG36LowDischargeAlarmMigration(saved)
  const needsLeakAlarm=needsG36TerminalLeakAlarmMigration(saved)
  const needsFanSpeed=needsG36FanSpeedFeedbackMigration(saved)
  const needsAhuDamper=needsG36AhuDamperFeedbackMigration(saved)
  const needsEconHold=needsG36EconomizerHoldMigration(saved)
  const needsEconTransition=needsG36EconomizerDisableTransitionMigration(saved)
  const needsFixedDryBulb=needsG36FixedDryBulbMigration(saved)
  if(!needsStart&&!needsDailyRecovery&&!needsCo2&&!needsAlarm&&!needsFanAlarm&&!needsSensorAlarm&&!needsDischargeAlarm&&!needsLeakAlarm&&!needsFanSpeed&&!needsAhuDamper&&!needsEconHold&&!needsEconTransition&&!needsFixedDryBulb)return saved
  if(needsStart)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-recovery-learning-backup',JSON.stringify(saved))
  if(needsDailyRecovery)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-start-qualified-recovery-backup',JSON.stringify(saved))
  if(needsCo2)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-co2-reset-backup',JSON.stringify(saved))
  if(needsAlarm)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-airflow-alarm-backup',JSON.stringify(saved))
  if(needsFanAlarm)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-fan-alarm-backup',JSON.stringify(saved))
  if(needsSensorAlarm)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-sensor-alarm-backup',JSON.stringify(saved))
  if(needsDischargeAlarm)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-discharge-alarm-backup',JSON.stringify(saved))
  if(needsLeakAlarm)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-leak-alarm-backup',JSON.stringify(saved))
  if(needsFanSpeed)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-fan-speed-feedback-backup',JSON.stringify(saved))
  if(needsAhuDamper)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-ahu-damper-feedback-backup',JSON.stringify(saved))
  if(needsEconHold)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-economizer-hold-backup',JSON.stringify(saved))
  if(needsEconTransition)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-economizer-transition-backup',JSON.stringify(saved))
  if(needsFixedDryBulb)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-fixed-dry-bulb-backup',JSON.stringify(saved))
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  const next=migrateG36DailyRecovery(needsStart?migrateG36OptimalStart(saved,reference):saved,reference)
  const upgraded=migrateG36FixedDryBulb(migrateG36EconomizerDisableTransition(migrateG36EconomizerHold(migrateG36AhuDamperFeedback(migrateG36FanSpeedFeedback(migrateG36TerminalLeakAlarms(migrateG36LowDischargeAlarms(migrateG36AirflowSensorAlarms(migrateG36SupplyFanAlarms(migrateG36TerminalAirflowAlarms(next,reference),reference),reference),reference),reference),reference),reference),reference),reference),reference)
  return migrateG36Co2(upgraded,reference)
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
  if(saved.graph.name!=='FourPipeG36Controls'||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_reset_all_request_count'))return saved
  const reference=await fetchJson<Block>('four_pipe_g36.json')
  return migrateG36TerminalHotRequests(saved,reference)
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
  const hasWaterPressure=saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(child=>child.name==='hot_dp_pi')&&saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(child=>child.name==='chilled_dp_pi')
  const hasChilledWaterMapping=saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(child=>child.name==='chilled_reset_temperature_scale')
  const hasCoolingStartup=saved.graph.blocks?.filter(block=>block.block_type==='VAVBoxController').every(block=>!block.blocks?.some(child=>child.name==='cooling_enabled'))
  if(hasSat&&hasDischarge&&hasDamper&&hasFreeze&&hasEconomizerProof&&hasOutdoorLimits&&hasReturnDamper&&hasCommonDamperMapping&&hasVentilation&&hasWaterSourceProof&&hasWaterPressure&&hasChilledWaterMapping&&hasCoolingStartup)return migrateG36VacancyDelay(saved)
  if(!hasDischarge)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-discharge-backup',JSON.stringify(saved))
  if(!hasDamper)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-damper-backup',JSON.stringify(saved))
  if(!hasFreeze)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-freeze-backup',JSON.stringify(saved))
  if(!hasEconomizerProof)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-economizer-proof-backup',JSON.stringify(saved))
  if(!hasOutdoorLimits)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-outdoor-limits-backup',JSON.stringify(saved))
  if(!hasReturnDamper)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-return-damper-backup',JSON.stringify(saved))
  if(!hasCommonDamperMapping)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-common-damper-mapping-backup',JSON.stringify(saved))
  if(!hasVentilation)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-ventilation-backup',JSON.stringify(saved))
  if(!hasWaterSourceProof)storeMigrationBackup(localStorage,'ace-cdl-g36-pre-water-source-proof-backup',JSON.stringify(saved))
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
  const withChilledWaterMapping=hasChilledWaterMapping?withWaterPressure:migrateG36ChilledWaterMapping(withWaterPressure,reference)
  const withCoolingStartup=hasCoolingStartup?withChilledWaterMapping:migrateG36CoolingStartup(withChilledWaterMapping)
  return migrateG36VacancyDelay(withCoolingStartup)
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
async function loadExample(){if(!exampleChoice.value)return;try{const graph=await fetchJson<Block>(exampleChoice.value);const next:Project={version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};updateProject(next);Object.assign(scenario,structuredClone(defaultScenario),next.scenario);syncInputs(graph);results.value=new ArrowRows([]);resultSummary.value=null;navigate('editor')}catch(e){errors.value=[String(e)]}}
function triggerImport(){fileInput.value?.click()}
let coreCache:Promise<CxfDocument>|null=null
function cxfCore():Promise<CxfDocument>{return coreCache??=(async()=>{const response=await fetch(new URL('schemas/CXF-Core.jsonld',document.baseURI));if(!response.ok)throw new Error('Could not load the CXF-Core class library');return response.json() as Promise<CxfDocument>})()}
async function importFile(event:Event){const file=(event.target as HTMLInputElement).files?.[0];if(!file)return;try{const data=JSON.parse(await file.text());const isCxf=Array.isArray(data['@graph']);const graph:Block=isCxf?importCxf(data,await cxfCore()):data.version===1?data.graph:data;if(!graph?.name||!graph.block_type)throw new Error('Expected a CDL block, ACE CDL project, or CXF JSON-LD document');let next:Project=!isCxf&&data.version===1?{...data,layout:data.layout??layoutFor(graph),scenario:{...defaultScenario,...data.scenario}}:{version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};next=await upgradeWaterDispatch(next);next=await upgradeG36ControlUpdates(await upgradeG36TerminalHotRequests(await upgradeG36HotWaterReset(await upgradeWaterEnergyControl(await upgradeG36WaterReset(await upgradeWaterLoopControl(await upgradeCoilControl(await upgradeOutdoorAirCommand(await upgradeG36Economizer(await upgradeG36PressureTrim(await upgradeG36AhuFeedback(await upgradeG36TerminalFeedback(await upgradeSavedG36Timing(next)))))))))))));next=await upgradeG36StageFeedback(await upgradeG36EquipmentAvailability(await upgradeG36HybridDispatch(await upgradeG36WaterStageHold(await upgradeWaterDispatch(next)))));next=await upgradeG36StartFault(next);next=await upgradeG36BackupStartFault(next);next=await upgradeG36ZoneModes(next);next=await upgradeG36ZoneGroups(next);next=await upgradeG36GroupUnoccupied(next);next=await upgradeG36Windows(next);next=await upgradeG36Overrides(next);next=await upgradeG36OptimalStart(next);next=upgradeLayout(await fillExampleMetadata(upgradeSupplyFeedback(next)));updateProject(next);Object.assign(scenario,structuredClone(defaultScenario),next.scenario);syncInputs(next.graph);results.value=new ArrowRows([]);resultSummary.value=null;navigate('editor')}catch(e){errors.value=[String(e)]}finally{(event.target as HTMLInputElement).value=''}}
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
  const zoneAirflowColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_airflow`)
  const zoneDischargeColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_discharge_c`)
  const zoneDamperColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_actual_damper_position`)
  const zoneValveColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_actual_reheat_valve_position`)
  const header=['index','schedule_occupied','preconditioning_mode',...occupancyColumns,...rawPresenceColumns,...zoneAirflowColumns,...zoneDischargeColumns,...zoneDamperColumns,...zoneValveColumns,'outdoor_c','avg_zone_c',...zoneColumns,...zoneCo2Columns,'requested_airflow','delivered_airflow','ahu_1_supply_setpoint_c','ahu_1_supply_actual_c','ahu_2_supply_setpoint_c','ahu_2_supply_actual_c','ahu_1_duct_pressure','ahu_2_duct_pressure','ahu_1_outdoor_air_fraction','ahu_2_outdoor_air_fraction','ahu_1_intake_opening','ahu_2_intake_opening','ahu_1_actual_fan_speed','ahu_2_actual_fan_speed','ahu_1_actual_outdoor_damper_position','ahu_2_actual_outdoor_damper_position','ahu_1_actual_return_damper_position','ahu_2_actual_return_damper_position','ahu_1_outdoor_airflow','ahu_2_outdoor_airflow','ahu_1_mixed_air_c','ahu_2_mixed_air_c','cool_request_kw','cool_kw','heat_request_kw','heat_kw','heat_pump_heating_used_kw','heat_pump_cooling_used_kw','heat_pump_electric_kw','heat_pump_ambient_exchange_kw','heat_pump_available_capacity_kw','heat_pump_heating_cop','heat_pump_cooling_cop','heat_pump_available','boiler_available','chiller_available','heat_pump_running','boiler_running','chiller_running','hot_water_plant_proven','boiler_used_kw','chiller_used_kw','electric_kw','gas_kw']
  const rows:string[]=[]
  for(let i=0;i<results.value.length;i++){const r=results.value.at(i)!;rows.push([r.index,r.schedule_occupied??'',r.preconditioning_mode??'',...(r.zone_occupied??Array(10).fill('')),...(r.zone_presence_raw??Array(10).fill('')),...(r.zone_airflow??Array(10).fill('')),...(r.zone_discharge_c??Array(10).fill('')),...(r.zone_actual_damper_position??Array(10).fill('')),...(r.zone_actual_reheat_valve_position??Array(10).fill('')),r.outdoor_c,r.avg_zone_c,...r.zone_c,...(r.zone_co2_ppm??Array(10).fill('')),r.requested_airflow,r.delivered_airflow,r.ahu_1_supply_setpoint_c,r.ahu_1_supply_actual_c,r.ahu_2_supply_setpoint_c,r.ahu_2_supply_actual_c,r.ahu_1_duct_pressure,r.ahu_2_duct_pressure,r.ahu_1_outdoor_air_fraction,r.ahu_2_outdoor_air_fraction,r.ahu_1_intake_opening??'',r.ahu_2_intake_opening??'',r.ahu_1_actual_fan_speed??'',r.ahu_2_actual_fan_speed??'',r.ahu_1_actual_outdoor_damper_position??'',r.ahu_2_actual_outdoor_damper_position??'',r.ahu_1_actual_return_damper_position??'',r.ahu_2_actual_return_damper_position??'',r.ahu_1_outdoor_airflow,r.ahu_2_outdoor_airflow,r.ahu_1_mixed_air_c,r.ahu_2_mixed_air_c,r.cool_request_kw,r.cool_kw,r.heat_request_kw,r.heat_kw,r.heat_pump_heating_used_kw,r.heat_pump_cooling_used_kw,r.heat_pump_electric_kw??'',r.heat_pump_ambient_exchange_kw??'',r.heat_pump_available_capacity_kw??'',r.heat_pump_heating_cop??'',r.heat_pump_cooling_cop??'',r.heat_pump_available??'',r.boiler_available??'',r.chiller_available??'',r.heat_pump_running??'',r.boiler_running??'',r.chiller_running??'',r.hot_water_plant_proven??'',r.boiler_used_kw,r.chiller_used_kw,r.electric_kw,r.gas_kw].join(','))}
  download('ace-cdl-simulation-2025.csv',[header.join(','),...rows].join('\n'),'text/csv')
}
function runStep(){if(project.value)worker.postMessage({id:++jobId,type:'execute',graph:plainWorkerData(project.value.graph),inputs:plainWorkerData(testInputs)})}
function addHeatPumpPoint(){const points=scenario.heat_pump_performance;const last=points.at(-1)!;points.push({...last,outdoor_c:last.outdoor_c+10});saveScenario()}
function removeHeatPumpPoint(index:number){scenario.heat_pump_performance.splice(index,1);saveScenario()}
function addEquipmentOutage(){scenario.equipment_outages.push({equipment:'heat_pump',start_hour:24,duration_hours:2});saveScenario()}
function removeEquipmentOutage(index:number){scenario.equipment_outages.splice(index,1);saveScenario()}
function addStartFailure(){scenario.equipment_start_failures.push({equipment:'heat_pump',start_hour:24,duration_hours:2});saveScenario()}
function removeStartFailure(index:number){scenario.equipment_start_failures.splice(index,1);saveScenario()}
function addFanFault(){scenario.fan_faults.push({equipment:'ahu_1_stuck_off',start_hour:24,duration_hours:2});saveScenario()}
function addTerminalFault(){scenario.terminal_faults.push({zone:1,kind:'damper_stuck_open',start_hour:24,duration_hours:2});saveScenario()}
function addAhuDamperFault(){scenario.ahu_damper_faults.push({ahu:1,damper:'outdoor',stuck_position:1,start_hour:24,duration_hours:2});saveScenario()}
function addOutdoorTemperatureEvent(){scenario.outdoor_temperature_events.push({delta_c:5,start_hour:24.05,duration_hours:0.05});saveScenario()}
function removeOutdoorTemperatureEvent(index:number){scenario.outdoor_temperature_events.splice(index,1);saveScenario()}
function removeAhuDamperFault(index:number){scenario.ahu_damper_faults.splice(index,1);saveScenario()}
function removeTerminalFault(index:number){scenario.terminal_faults.splice(index,1);saveScenario()}
function removeFanFault(index:number){scenario.fan_faults.splice(index,1);saveScenario()}
function addWindowOpening(){scenario.window_openings.push({zone:1,start_hour:24,duration_hours:2});saveScenario()}
function removeWindowOpening(index:number){scenario.window_openings.splice(index,1);saveScenario()}
function addOccupancyOverride(){scenario.occupancy_overrides.push({zone:1,start_hour:24,duration_hours:2});saveScenario()}
function removeOccupancyOverride(index:number){scenario.occupancy_overrides.splice(index,1);saveScenario()}
function saveScenario(){if(!project.value)return;const next=structuredClone(project.value);next.scenario={...scenario};updateProject(next)}
let baselineLoading:Promise<void>|null=null
function ensureBaseline(){if(baseline.value.length||baselineLoading)return;baselineLoading=Promise.all([fetchJson<{rows:Row[];summary:Summary}>('baseline_2025.json'),fetchJson<{summary:Summary}>('baseline_summary_2025_60s.json')]).then(([data,comparison])=>{baseline.value=data.rows;baselineSummary.value=data.summary;comparisonBaselineSummary.value=comparison.summary;simError.value=''}).catch(e=>{simError.value=String(e)}).finally(()=>{baselineLoading=null})}
function syncRoute(){tab.value=routeFromHash();if(tab.value==='simulation')ensureBaseline()}
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
async function runYear(){if(!project.value||!isBuildingGraph.value||running.value)return;running.value=true;simError.value='';resultSummary.value=null;progress.value=0;lastResultRefresh=0;const graph=plainWorkerData(project.value.graph);const traces=selectedSignalDetails.value.filter(signal=>signal.kind!=='physical').map(signal=>signal.id);activeTraceIds.value=traces;results.value=new ArrowRows(traces);const id=++jobId;activeRunId=id;try{const weather=await fetchJson<unknown[]>('weather_2025.json');if(activeRunId===id)worker.postMessage({id,type:'simulate',graph,scenario:scenarioForWorker(scenario),weather,traces})}catch(e){if(activeRunId===id){simError.value=String(e);running.value=false;activeRunId=null}}}
function cancelRun(){activeRunId=null;worker.postMessage({type:'cancel'});running.value=false}
worker.onmessage=({data})=>{if(data.type==='validation'){errors.value=[...data.errors,...(project.value?validateBrickModel(project.value.graph):[])]}else if(data.type==='result'){testResult.value=JSON.stringify(data.result,null,2)}else if(data.type==='batch'){if(data.id!==activeRunId)return;try{results.value.append(data.buffer)}catch(e){simError.value=String(e);cancelRun();return}progress.value=data.progress;resultSummary.value=data.summary;if(data.done||performance.now()-lastResultRefresh>500){triggerRef(results);rowVersion.value++;lastResultRefresh=performance.now()}if(data.done){running.value=false;activeRunId=null}}else if(data.type==='cancelled'){if(data.id===activeRunId){running.value=false;activeRunId=null}}else if(data.type==='error'){if(data.id===activeRunId){simError.value=data.message;running.value=false;activeRunId=null}else testResult.value=data.message}}
onMounted(async()=>{window.addEventListener('hashchange',syncRoute);syncRoute();let stored:string|null=null;try{stored=localStorage.getItem('ace-cdl-project-v1');if(stored){let parsed=JSON.parse(stored) as Project;if(!parsed?.graph?.name||!parsed.graph.block_type)throw new Error('Saved project is missing its control graph');parsed.layout??=layoutFor(parsed.graph);parsed.scenario={...defaultScenario,...parsed.scenario};if(parsed.graph.name==='FourPipeBuildingControls'&&!(parsed.graph.blocks??[]).some(b=>b.block_type==='PlantDispatch')){storeMigrationBackup(localStorage,'ace-cdl-legacy-project-backup',stored);const graph=await fetchJson<Block>('four_pipe_building.json');for(const old of parsed.graph.blocks??[]){const next=graph.blocks?.find(b=>b.name===old.name);if(!next)continue;const oldParam=(name:string)=>old.parameters?.find(p=>p.name===name)?.value;for(const [oldName,childName] of [['cooling_setpoint','cool_setpoint'],['heating_setpoint','heat_setpoint'],['min_damper_position','minimum_damper'],['min_airflow','minimum_airflow']] as const){const value=oldParam(oldName);const p=next.blocks?.find(b=>b.name===childName)?.parameters?.find(p=>p.name==='k');if(p&&typeof value==='number')p.value=value}}const legacyShare=parsed.scenario.heat_pump_fraction;if(typeof legacyShare==='number'){const dispatch=graph.blocks?.find(b=>b.name==='plant')?.blocks?.find(b=>b.name==='heat_pump_dispatch_fraction')?.parameters?.find(p=>p.name==='k');if(dispatch)dispatch.value=Math.max(0,Math.min(1,legacyShare));delete parsed.scenario.heat_pump_fraction}parsed={version:1,name:graph.name,graph,scenario:parsed.scenario,layout:layoutFor(graph)};updateProject(parsed)}else{if(parsed.graph.name==='FourPipeG36Controls'&&parsed.graph.inputs?.some(port=>port.name==='preconditioning_mode'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-timing-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&(!parsed.graph.inputs?.some(port=>port.name==='zone_01_measured_airflow')||!parsed.graph.blocks?.find(block=>block.name==='zone_01')?.blocks?.some(block=>block.name==='heating_damper_select')))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-terminal-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&(!parsed.graph.inputs?.some(port=>port.name==='ahu_1_outdoor_airflow')||!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='fan_enable')))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-ahu-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='pressure_previous'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-pressure-trim-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='economizer_mixed_air_pi'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-economizer-backup',stored);if(['FourPipeBuildingControls','FourPipeG36Controls'].includes(parsed.graph.name)&&!parsed.graph.outputs?.some(port=>port.name==='ahu_1_heating_coil_valve_position'))storeMigrationBackup(localStorage,'ace-cdl-pre-coil-control-backup',stored);if(['FourPipeBuildingControls','FourPipeG36Controls'].includes(parsed.graph.name)&&!parsed.graph.outputs?.some(port=>port.name==='plant_hot_water_pump_speed'))storeMigrationBackup(localStorage,'ace-cdl-pre-water-loop-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='chilled_reset_previous'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-water-reset-backup',stored);if(['FourPipeBuildingControls','FourPipeG36Controls'].includes(parsed.graph.name)&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_water_source_gain'))storeMigrationBackup(localStorage,'ace-cdl-pre-water-energy-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_reset_previous'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-hot-water-reset-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_reset_all_request_count'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-terminal-hot-requests-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='sat_previous'))storeMigrationBackup(localStorage,'ace-cdl-g36-pre-sat-reset-backup',stored);parsed=await upgradeWaterDispatch(parsed);const migrated=await upgradeG36ControlUpdates(await upgradeG36TerminalHotRequests(await upgradeG36HotWaterReset(await upgradeWaterEnergyControl(await upgradeG36WaterReset(await upgradeWaterLoopControl(await upgradeCoilControl(await upgradeOutdoorAirCommand(await upgradeG36Economizer(await upgradeG36PressureTrim(await upgradeG36AhuFeedback(await upgradeG36TerminalFeedback(await upgradeSavedG36Timing(parsed)))))))))))));const dispatched=await upgradeG36HybridDispatch(await upgradeG36WaterStageHold(await upgradeWaterDispatch(migrated)));const enriched=upgradeLayout(await fillExampleMetadata(upgradeSupplyFeedback(await upgradeG36OptimalStart(await upgradeG36Overrides(await upgradeG36Windows(await upgradeG36GroupUnoccupied(await upgradeG36ZoneGroups(await upgradeG36ZoneModes(await upgradeG36BackupStartFault(await upgradeG36StartFault(await upgradeG36StageFeedback(await upgradeG36EquipmentAvailability(dispatched)))))))))))));if(enriched===parsed){project.value=parsed;validateGraph(parsed.graph)}else{parsed=enriched;updateProject(parsed)}}Object.assign(scenario,structuredClone(defaultScenario),parsed.scenario);syncInputs(parsed.graph)}else{const graph=await fetchJson<Block>('four_pipe_building.json');const next:Project={version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};updateProject(next);syncInputs(graph)}}catch(e){recoverableProject.value=stored;storageWarning.value=stored?'Saved controls could not be opened. A fresh example is available below; download your saved project before editing.':'Could not open the default controls.';try{const graph=await fetchJson<Block>('four_pipe_building.json');project.value={version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};syncInputs(graph);validateGraph(graph)}catch(loadError){errors.value=[String(e),String(loadError)]}}})
onUnmounted(()=>{window.removeEventListener('hashchange',syncRoute);worker.terminate()})
</script>
