<template>
  <div class="app-shell">
    <header class="site-header"><div class="brand" @click="navigate('primer')"><span class="brand-mark">ACE<span>◆</span></span><span class="brand-divider"/><span class="brand-name">CDL <b>EXPLORER</b></span></div><nav class="top-nav" aria-label="Main navigation"><button :class="{active:tab==='primer'}" @click="navigate('primer')">Primer</button><button :class="{active:tab==='editor'}" @click="navigate('editor')">Visual editor</button><button :class="{active:tab==='simulation'}" @click="navigate('simulation')">Simulation</button><button :class="{active:tab==='glossary'}" @click="navigate('glossary')">Glossary</button></nav><span class="header-tag">BROWSER-BASED CONTROLS LAB</span></header>

    <main v-if="tab==='primer'"><PrimerPage/><section class="cta-band"><div><span class="eyebrow">READY TO EXPERIMENT?</span><h2>Turn the sequence into a working model.</h2></div><button class="solid-button" @click="navigate('editor')">Open the visual editor ↗</button></section></main>

    <main v-else-if="tab==='glossary'"><GlossaryPage/></main>

    <main v-else-if="tab==='editor'" class="workspace-page"><div class="workspace-heading"><div><span class="eyebrow">WORKSPACE / CONTROL GRAPH</span><h1>{{ project?.name ?? 'Loading controls…' }}</h1><p>Build a sequence from blocks, tune parameters, and test its outputs.</p></div><div class="workspace-actions"><select v-model="exampleChoice" aria-label="Load example"><option value="">Load an example…</option><option v-for="example in examples" :key="example.file" :value="example.file">{{ example.label }}</option></select><button class="outline-button" @click="loadExample">Load</button><button class="outline-button" @click="triggerImport">Import JSON / CXF</button><button class="outline-button" :disabled="!project" @click="exportProject">Export project</button><button class="outline-button" :disabled="!project" @click="exportCxfFile">Export CXF</button><input ref="fileInput" class="sr-only" type="file" accept="application/json,application/ld+json,.json,.jsonld" @change="importFile"/></div></div>
      <div v-if="project" class="workspace-body"><div class="validation-strip" :class="{invalid:errors.length}"><span>{{ errors.length ? `● ${errors.length} graph issue${errors.length===1?'':'s'}` : '● Graph ready' }}</span><span>{{ project.graph.blocks?.length ?? 0 }} top-level blocks · Local autosave</span></div><div v-if="errors.length" class="validation-list"><p v-for="(error,i) in errors.slice(0,8)" :key="i">{{ error }}</p></div><GraphEditor :key="project.graph.name" :project="project" @change="updateProject"/><section v-if="!isBuildingGraph" class="try-panel"><div><span class="eyebrow">QUICK TEST</span><h2>Run one control step</h2><p>Supply values for the imported sequence and inspect its output.</p></div><div class="try-inputs"><label v-for="input in project.graph.inputs ?? []" :key="input.name" class="field"><span>{{ input.name }} <small>{{ input.unit }}</small></span><select v-if="input.type==='Boolean'" v-model="testInputs[input.name]"><option :value="true">True</option><option :value="false">False</option></select><input v-else type="number" step="any" v-model.number="testInputs[input.name]"/></label></div><button class="solid-button" :disabled="errors.length>0" @click="runStep">Run step</button><pre v-if="testResult">{{ testResult }}</pre></section></div>
    </main>

    <main v-else class="simulation-page"><section class="simulation-hero"><div><span class="eyebrow">THE FOUR-PIPE BUILDING</span><h1>One year. Ten zones. Your control logic.</h1><p>A documented synthetic mixed-climate year drives two AHUs, ten VAVs, and shared hot and chilled water loops. Load the conventional or G36-style sample in the editor, then run it against the same building model.</p><div class="hero-actions"><AceButton :disabled="!project || !isBuildingGraph || errors.length>0 || running" @click="runYear">{{ running ? 'Running…' : 'Run 2025 simulation' }}</AceButton><button v-if="running" class="outline-button" @click="cancelRun">Cancel</button><button class="outline-button" @click="navigate('editor')">Edit controls ↗</button></div><p v-if="!isBuildingGraph" class="simulation-notice">Load a Four-pipe building example in the editor to run the building simulation.</p></div><div class="hero-stat"><span>2025 SIMULATION</span><strong>{{ number(totalSteps) }}</strong><small>{{ stepLabel }} steps · 35,040 weather samples</small></div></section>
      <div class="sim-layout"><aside class="sim-side"><div class="panel-title"><span class="eyebrow">SCENARIO</span><h2>Building assumptions</h2></div><p class="muted small">These simplified thermal assumptions apply to all ten zones.</p><label class="field"><span>Maximum simulation timestep</span><select v-model.number="scenario.step_seconds" :disabled="running" @change="saveScenario"><option :value="900">15 minutes</option><option :value="300">5 minutes</option><option :value="60">1 minute</option><option :value="30">30 seconds</option><option :value="15">15 seconds</option></select></label><p class="muted small">Weather updates every 15 minutes. The controls and building physics run at the selected maximum step or a smaller step required by sampled CDL blocks. Charts record one value per weather interval.</p><p v-if="effectiveStepSeconds<scenario.step_seconds" class="muted small">This graph contains sampled CDL blocks. The runtime uses {{ effectiveStepSeconds }}-second internal steps to resolve their timing.</p><label class="field"><span>Zone envelope · kW/K</span><input v-model.number="scenario.zone_ua_kw_per_k" type="number" min="0.01" max="2" step="0.01" @change="saveScenario"/></label><label class="field"><span>Thermal capacity · kWh/K</span><input v-model.number="scenario.zone_capacity_kwh_per_k" type="number" min="0.1" max="50" step="0.1" @change="saveScenario"/></label><label class="field"><span>Heat pump share · %</span><input :value="Math.round(scenario.heat_pump_fraction*100)" type="range" min="0" max="100" step="5" @input="scenario.heat_pump_fraction=Number(($event.target as HTMLInputElement).value)/100;saveScenario()"/><b>{{ Math.round(scenario.heat_pump_fraction*100) }}%</b></label><label class="field"><span>Heat pump capacity · kW</span><input v-model.number="scenario.heat_pump_capacity_kw" type="number" min="1" step="5" @change="saveScenario"/></label><label class="field"><span>Boiler capacity · kW</span><input v-model.number="scenario.boiler_capacity_kw" type="number" min="1" step="5" @change="saveScenario"/></label><label class="field"><span>Chiller capacity · kW</span><input v-model.number="scenario.chiller_capacity_kw" type="number" min="1" step="5" @change="saveScenario"/></label><details class="field"><summary>Freeze-stat test inputs</summary><p class="muted small">Inject a raw AHU freeze-stat trip and an operator reset pulse. The editable CDL graph decides the shutdown and recovery.</p><label class="field"><span>Test AHU</span><select v-model.number="scenario.freeze_test_ahu" :disabled="running" @change="saveScenario"><option :value="0">Off</option><option :value="1">AHU 1</option><option :value="2">AHU 2</option></select></label><label class="field"><span>Trip starts · elapsed hours</span><input v-model.number="scenario.freeze_test_start_hour" type="number" min="-1" step="0.25" @change="saveScenario"/></label><label class="field"><span>Trip duration · hours</span><input v-model.number="scenario.freeze_test_duration_hours" type="number" min="0" step="0.25" @change="saveScenario"/></label><label class="field"><span>Operator reset · elapsed hours</span><input v-model.number="scenario.freeze_test_reset_hour" type="number" min="-1" step="0.25" @change="saveScenario"/></label></details><div class="assumption-note">Each AHU has a 20% minimum outdoor-air share; an editable G36-style sequence can increase it for free cooling. Fan capacity determines delivered airflow and normalized duct pressure. Graph-commanded water pumps and source output charge mixed water volumes; flow and measured supply temperature limit coil and reheat delivery.</div></aside>
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
      <div v-if="isG36Sample && results.length && comparisonBaselineSummary" class="chart-card comparison-card"><div class="card-heading"><div><span class="eyebrow">CONTROL COMPARISON</span><h2>G36-style vs conventional</h2></div><span class="muted small">SAME 2025 WEATHER · TEN ZONES</span></div><p v-if="!defaultComparisonScenario" class="muted small">The bundled conventional reference uses default building assumptions. Use the default assumptions and a 60-second effective timestep to compare like for like.</p><div v-else class="comparison-grid"><div v-for="metric in comparisonMetrics" :key="metric.label"><span>{{ metric.label }}</span><strong>{{ deltaPercent(metric.current,metric.baseline) }}</strong><small>{{ number(metric.current) }} current · {{ number(metric.baseline) }} conventional {{ metric.unit }}</small></div></div><p class="muted small">This sample includes occupancy setpoint resets and temperature-enabled, mixed-air-feedback outdoor-air modulation. It includes editable AHU coil valve loops, sampled duct-pressure and supply-air temperature resets, teaching alarms, and sampled hot/chilled-water supply resets with VAV reheat requests, but still omits full G36 economizer limits, plant pressure reset and staging, and the full G36 alarm set. Compare comfort as well as energy.</p></div>
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
import { graphSignals, physicalSignals, type GraphSignal } from './lib/signals'
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
import { migrateG36CoolingStartup } from './lib/g36CoolingStartupMigration'
import { migrateWaterEnergyControl } from './lib/waterEnergyMigration'
import { ArrayRows, ArrowRows } from './lib/frames'
import RuntimeWorker from './workers/runtime.worker?worker'

type Tab = 'primer' | 'editor' | 'simulation' | 'glossary'
function routeFromHash():Tab {const route=location.hash.slice(1);return route==='editor'||route==='simulation'||route==='glossary'?route:'primer'}
const tab=ref<Tab>(routeFromHash())
const project=shallowRef<Project|null>(null)
const errors=ref<string[]>([])
const worker=new RuntimeWorker()
const exampleChoice=ref('')
const fileInput=ref<HTMLInputElement|null>(null)
const examples=[{file:'four_pipe_building.json',label:'Four-pipe building · conventional controls'},{file:'four_pipe_g36.json',label:'Four-pipe building · G36-style controls'},{file:'vav_composite_template.json',label:'VAV composite · editable blocks'},{file:'p_controller_limiter.json',label:'P controller with limiter'},{file:'room_control_system.json',label:'Dual-mode room controller'},{file:'simple_temp_controller.json',label:'Basic temperature controller'},...['east','north','south','west','corridor'].map(x=>({file:`${x}_controller.json`,label:`Legacy VAV controller · ${x}`}))]
const testInputs=reactive<Record<string,number|boolean>>({})
const testResult=ref('')
const scenario=reactive<Scenario>({...defaultScenario})
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
const defaultComparisonScenario=computed(()=>effectiveStepSeconds.value===60&&Object.entries(defaultScenario).every(([key,value])=>key==='step_seconds'||scenario[key as keyof Scenario]===value))
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
const signalOptions=computed(()=>project.value?[...physicalSignals,...graphSignals(project.value.graph)]:[])
const filteredSignals=computed(()=>{const query=signalSearch.value.trim().toLowerCase();return signalOptions.value.filter(signal=>!selectedSignals.value.includes(signal.id)&&(!query||`${signal.label} ${signal.unit} ${signal.brickClass??''}`.toLowerCase().includes(query)))})
const selectedSignalDetails=computed(()=>selectedSignals.value.flatMap(id=>{const signal=signalOptions.value.find(item=>item.id===id);return signal?[signal]:[]}))
const needsSignalRun=computed(()=>selectedSignals.value.length>0&&(!results.value.length||selectedSignalDetails.value.some(signal=>signal.kind!=='physical'&&!activeTraceIds.value.includes(signal.id))))
const plotGroups=computed(()=>{if(!results.value.length)return [];const groups=new Map<string,GraphSignal[]>();for(const signal of selectedSignalDetails.value){if(signal.kind!=='physical'&&!activeTraceIds.value.includes(signal.id))continue;const series=groups.get(signal.unit)??[];series.push(signal);groups.set(signal.unit,series)}return [...groups].map(([unit,signals])=>({unit,signals}))})
const number=(n:number)=>Math.round(n).toLocaleString()
const asset=(name:string)=>new URL(`examples/${name}`,document.baseURI).toString()
async function fetchJson<T>(name:string):Promise<T>{const response=await fetch(asset(name));if(!response.ok)throw new Error(`Could not load ${name}`);return response.json() as Promise<T>}
function syncInputs(graph:Block){for(const key of Object.keys(testInputs))delete testInputs[key];for(const input of graph.inputs??[])testInputs[input.name]=input.type==='Boolean'?false:input.name.toLowerCase().includes('max')?20:input.name.toLowerCase().includes('temp')?22:1;testResult.value=''}
function updateProject(next:Project){project.value=next;results.value=new ArrowRows([]);resultSummary.value=null;activeTraceIds.value=[];localStorage.setItem('ace-cdl-project-v1',JSON.stringify(next));validateGraph(next.graph)}
async function fillExampleMetadata(saved:Project):Promise<Project>{
  if(saved.graph.name!=='FourPipeBuildingControls')return saved
  try{
    const reference=await fetchJson<Block>('four_pipe_building.json')
    const next=structuredClone(saved)
    let changed=false
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
  if(!['FourPipeBuildingControls','FourPipeG36Controls'].includes(saved.graph.name)||saved.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_water_source_gain'))return saved
  const reference=await fetchJson<Block>(saved.graph.name==='FourPipeG36Controls'?'four_pipe_g36.json':'four_pipe_building.json')
  return migrateWaterEnergyControl(saved,reference)
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
  const hasCoolingStartup=saved.graph.blocks?.filter(block=>block.block_type==='VAVBoxController').every(block=>!block.blocks?.some(child=>child.name==='cooling_enabled'))
  if(hasSat&&hasDischarge&&hasDamper&&hasFreeze&&hasEconomizerProof&&hasOutdoorLimits&&hasReturnDamper&&hasCommonDamperMapping&&hasVentilation&&hasWaterSourceProof&&hasWaterPressure&&hasCoolingStartup)return migrateG36VacancyDelay(saved)
  if(!hasDischarge)localStorage.setItem('ace-cdl-g36-pre-discharge-backup',JSON.stringify(saved))
  if(!hasDamper)localStorage.setItem('ace-cdl-g36-pre-damper-backup',JSON.stringify(saved))
  if(!hasFreeze)localStorage.setItem('ace-cdl-g36-pre-freeze-backup',JSON.stringify(saved))
  if(!hasEconomizerProof)localStorage.setItem('ace-cdl-g36-pre-economizer-proof-backup',JSON.stringify(saved))
  if(!hasOutdoorLimits)localStorage.setItem('ace-cdl-g36-pre-outdoor-limits-backup',JSON.stringify(saved))
  if(!hasReturnDamper)localStorage.setItem('ace-cdl-g36-pre-return-damper-backup',JSON.stringify(saved))
  if(!hasCommonDamperMapping)localStorage.setItem('ace-cdl-g36-pre-common-damper-mapping-backup',JSON.stringify(saved))
  if(!hasVentilation)localStorage.setItem('ace-cdl-g36-pre-ventilation-backup',JSON.stringify(saved))
  if(!hasWaterSourceProof)localStorage.setItem('ace-cdl-g36-pre-water-source-proof-backup',JSON.stringify(saved))
  if(!hasWaterPressure)localStorage.setItem('ace-cdl-g36-pre-water-pressure-backup',JSON.stringify(saved))
  if(!hasCoolingStartup)localStorage.setItem('ace-cdl-g36-pre-cooling-startup-backup',JSON.stringify(saved))
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
  const withCoolingStartup=hasCoolingStartup?withWaterPressure:migrateG36CoolingStartup(withWaterPressure)
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
function validateGraph(graph:Block){worker.postMessage({id:++jobId,type:'validate',graph})}
async function loadExample(){if(!exampleChoice.value)return;try{const graph=await fetchJson<Block>(exampleChoice.value);const next:Project={version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};updateProject(next);Object.assign(scenario,next.scenario);syncInputs(graph);results.value=new ArrowRows([]);resultSummary.value=null;navigate('editor')}catch(e){errors.value=[String(e)]}}
function triggerImport(){fileInput.value?.click()}
let coreCache:Promise<CxfDocument>|null=null
function cxfCore():Promise<CxfDocument>{return coreCache??=(async()=>{const response=await fetch(new URL('schemas/CXF-Core.jsonld',document.baseURI));if(!response.ok)throw new Error('Could not load the CXF-Core class library');return response.json() as Promise<CxfDocument>})()}
async function importFile(event:Event){const file=(event.target as HTMLInputElement).files?.[0];if(!file)return;try{const data=JSON.parse(await file.text());const isCxf=Array.isArray(data['@graph']);const graph:Block=isCxf?importCxf(data,await cxfCore()):data.version===1?data.graph:data;if(!graph?.name||!graph.block_type)throw new Error('Expected a CDL block, ACE CDL project, or CXF JSON-LD document');let next:Project=!isCxf&&data.version===1?{...data,layout:data.layout??layoutFor(graph),scenario:{...defaultScenario,...data.scenario}}:{version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};next=await upgradeG36ControlUpdates(await upgradeG36TerminalHotRequests(await upgradeG36HotWaterReset(await upgradeWaterEnergyControl(await upgradeG36WaterReset(await upgradeWaterLoopControl(await upgradeCoilControl(await upgradeOutdoorAirCommand(await upgradeG36Economizer(await upgradeG36PressureTrim(await upgradeG36AhuFeedback(await upgradeG36TerminalFeedback(await upgradeSavedG36Timing(next)))))))))))));updateProject(next);Object.assign(scenario,next.scenario);syncInputs(next.graph);results.value=new ArrowRows([]);resultSummary.value=null;navigate('editor')}catch(e){errors.value=[String(e)]}finally{(event.target as HTMLInputElement).value=''}}
function download(name:string,contents:string,type='application/json'){const url=URL.createObjectURL(new Blob([contents],{type}));const a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000)}
function exportProject(){if(project.value)download(`${project.value.name}.ace-cdl.json`,JSON.stringify(project.value,null,2))}
async function exportCxfFile(){if(!project.value)return;try{const cxf=exportCxf(project.value.graph,await cxfCore());download(`${project.value.name}.jsonld`,JSON.stringify(cxf,null,2),'application/ld+json')}catch(e){errors.value=[String(e)]}}
function exportResults(){
  const zoneColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_c`)
  const occupancyColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_occupied`)
  const rawPresenceColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_presence_raw`)
  const zoneAirflowColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_airflow`)
  const zoneDischargeColumns=Array.from({length:10},(_,index)=>`zone_${String(index+1).padStart(2,'0')}_discharge_c`)
  const header=['index','schedule_occupied','preconditioning_mode',...occupancyColumns,...rawPresenceColumns,...zoneAirflowColumns,...zoneDischargeColumns,'outdoor_c','avg_zone_c',...zoneColumns,'requested_airflow','delivered_airflow','ahu_1_supply_setpoint_c','ahu_1_supply_actual_c','ahu_2_supply_setpoint_c','ahu_2_supply_actual_c','ahu_1_duct_pressure','ahu_2_duct_pressure','ahu_1_outdoor_air_fraction','ahu_2_outdoor_air_fraction','ahu_1_intake_opening','ahu_2_intake_opening','ahu_1_outdoor_airflow','ahu_2_outdoor_airflow','ahu_1_mixed_air_c','ahu_2_mixed_air_c','cool_request_kw','cool_kw','heat_request_kw','heat_kw','heat_pump_heating_used_kw','heat_pump_cooling_used_kw','boiler_used_kw','chiller_used_kw','electric_kw','gas_kw']
  const rows:string[]=[]
  for(let i=0;i<results.value.length;i++){const r=results.value.at(i)!;rows.push([r.index,r.schedule_occupied??'',r.preconditioning_mode??'',...(r.zone_occupied??Array(10).fill('')),...(r.zone_presence_raw??Array(10).fill('')),...(r.zone_airflow??Array(10).fill('')),...(r.zone_discharge_c??Array(10).fill('')),r.outdoor_c,r.avg_zone_c,...r.zone_c,r.requested_airflow,r.delivered_airflow,r.ahu_1_supply_setpoint_c,r.ahu_1_supply_actual_c,r.ahu_2_supply_setpoint_c,r.ahu_2_supply_actual_c,r.ahu_1_duct_pressure,r.ahu_2_duct_pressure,r.ahu_1_outdoor_air_fraction,r.ahu_2_outdoor_air_fraction,r.ahu_1_intake_opening??'',r.ahu_2_intake_opening??'',r.ahu_1_outdoor_airflow,r.ahu_2_outdoor_airflow,r.ahu_1_mixed_air_c,r.ahu_2_mixed_air_c,r.cool_request_kw,r.cool_kw,r.heat_request_kw,r.heat_kw,r.heat_pump_heating_used_kw,r.heat_pump_cooling_used_kw,r.boiler_used_kw,r.chiller_used_kw,r.electric_kw,r.gas_kw].join(','))}
  download('ace-cdl-simulation-2025.csv',[header.join(','),...rows].join('\n'),'text/csv')
}
function runStep(){if(project.value)worker.postMessage({id:++jobId,type:'execute',graph:project.value.graph,inputs:{...testInputs}})}
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
async function runYear(){if(!project.value||!isBuildingGraph.value||running.value)return;running.value=true;simError.value='';resultSummary.value=null;progress.value=0;lastResultRefresh=0;const graph=project.value.graph;const traces=selectedSignalDetails.value.filter(signal=>signal.kind!=='physical').map(signal=>signal.id);activeTraceIds.value=traces;results.value=new ArrowRows(traces);const id=++jobId;activeRunId=id;try{const weather=await fetchJson<unknown[]>('weather_2025.json');if(activeRunId===id)worker.postMessage({id,type:'simulate',graph,scenario:{...scenario},weather,traces})}catch(e){if(activeRunId===id){simError.value=String(e);running.value=false;activeRunId=null}}}
function cancelRun(){activeRunId=null;worker.postMessage({type:'cancel'});running.value=false}
worker.onmessage=({data})=>{if(data.type==='validation'){errors.value=data.errors}else if(data.type==='result'){testResult.value=JSON.stringify(data.result,null,2)}else if(data.type==='batch'){if(data.id!==activeRunId)return;try{results.value.append(data.buffer)}catch(e){simError.value=String(e);cancelRun();return}progress.value=data.progress;resultSummary.value=data.summary;if(data.done||performance.now()-lastResultRefresh>500){triggerRef(results);rowVersion.value++;lastResultRefresh=performance.now()}if(data.done){running.value=false;activeRunId=null}}else if(data.type==='cancelled'){if(data.id===activeRunId){running.value=false;activeRunId=null}}else if(data.type==='error'){if(data.id===activeRunId){simError.value=data.message;running.value=false;activeRunId=null}else testResult.value=data.message}}
onMounted(async()=>{window.addEventListener('hashchange',syncRoute);syncRoute();try{const stored=localStorage.getItem('ace-cdl-project-v1');if(stored){let parsed=JSON.parse(stored) as Project;parsed.scenario={...defaultScenario,...parsed.scenario};if(parsed.graph.name==='FourPipeBuildingControls'&&!(parsed.graph.blocks??[]).some(b=>b.block_type==='PlantDispatch')){localStorage.setItem('ace-cdl-legacy-project-backup',stored);const graph=await fetchJson<Block>('four_pipe_building.json');for(const old of parsed.graph.blocks??[]){const next=graph.blocks?.find(b=>b.name===old.name);if(!next)continue;const oldParam=(name:string)=>old.parameters?.find(p=>p.name===name)?.value;for(const [oldName,childName] of [['cooling_setpoint','cool_setpoint'],['heating_setpoint','heat_setpoint'],['min_damper_position','minimum_damper'],['min_airflow','minimum_airflow']] as const){const value=oldParam(oldName);const p=next.blocks?.find(b=>b.name===childName)?.parameters?.find(p=>p.name==='k');if(p&&typeof value==='number')p.value=value}}parsed={version:1,name:graph.name,graph,scenario:parsed.scenario,layout:layoutFor(graph)};updateProject(parsed)}else{if(parsed.graph.name==='FourPipeG36Controls'&&parsed.graph.inputs?.some(port=>port.name==='preconditioning_mode'))localStorage.setItem('ace-cdl-g36-pre-timing-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&(!parsed.graph.inputs?.some(port=>port.name==='zone_01_measured_airflow')||!parsed.graph.blocks?.find(block=>block.name==='zone_01')?.blocks?.some(block=>block.name==='heating_damper_select')))localStorage.setItem('ace-cdl-g36-pre-terminal-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&(!parsed.graph.inputs?.some(port=>port.name==='ahu_1_outdoor_airflow')||!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='fan_enable')))localStorage.setItem('ace-cdl-g36-pre-ahu-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='pressure_previous'))localStorage.setItem('ace-cdl-g36-pre-pressure-trim-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='economizer_mixed_air_pi'))localStorage.setItem('ace-cdl-g36-pre-economizer-backup',stored);if(['FourPipeBuildingControls','FourPipeG36Controls'].includes(parsed.graph.name)&&!parsed.graph.outputs?.some(port=>port.name==='ahu_1_heating_coil_valve_position'))localStorage.setItem('ace-cdl-pre-coil-control-backup',stored);if(['FourPipeBuildingControls','FourPipeG36Controls'].includes(parsed.graph.name)&&!parsed.graph.outputs?.some(port=>port.name==='plant_hot_water_pump_speed'))localStorage.setItem('ace-cdl-pre-water-loop-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='chilled_reset_previous'))localStorage.setItem('ace-cdl-g36-pre-water-reset-backup',stored);if(['FourPipeBuildingControls','FourPipeG36Controls'].includes(parsed.graph.name)&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_water_source_gain'))localStorage.setItem('ace-cdl-pre-water-energy-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_reset_previous'))localStorage.setItem('ace-cdl-g36-pre-hot-water-reset-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='plant')?.blocks?.some(block=>block.name==='hot_reset_all_request_count'))localStorage.setItem('ace-cdl-g36-pre-terminal-hot-requests-backup',stored);if(parsed.graph.name==='FourPipeG36Controls'&&!parsed.graph.blocks?.find(block=>block.name==='ahu_1')?.blocks?.some(block=>block.name==='sat_previous'))localStorage.setItem('ace-cdl-g36-pre-sat-reset-backup',stored);const migrated=await upgradeG36ControlUpdates(await upgradeG36TerminalHotRequests(await upgradeG36HotWaterReset(await upgradeWaterEnergyControl(await upgradeG36WaterReset(await upgradeWaterLoopControl(await upgradeCoilControl(await upgradeOutdoorAirCommand(await upgradeG36Economizer(await upgradeG36PressureTrim(await upgradeG36AhuFeedback(await upgradeG36TerminalFeedback(await upgradeSavedG36Timing(parsed)))))))))))));const enriched=upgradeLayout(await fillExampleMetadata(upgradeSupplyFeedback(migrated)));if(enriched===parsed){project.value=parsed;validateGraph(parsed.graph)}else{parsed=enriched;updateProject(parsed)}}Object.assign(scenario,parsed.scenario);syncInputs(parsed.graph)}else{const graph=await fetchJson<Block>('four_pipe_building.json');const next:Project={version:1,name:graph.name,graph,scenario:{...defaultScenario},layout:layoutFor(graph)};updateProject(next);syncInputs(graph)}}catch(e){errors.value=[String(e)]}})
onUnmounted(()=>{window.removeEventListener('hashchange',syncRoute);worker.terminate()})
</script>
